//! Offline: `.ready` / `.done` files in `pg_wal/archive_status`.
//!
//! That a `.ready` file means "waiting for the archiver" and `.done` means
//! "archived" is an **assumption** (not verified against PostgreSQL source).

use std::fs;
use std::io;

use crate::report::{Finding, Status};
use crate::root::Root;
use crate::walname::is_segment_name;

use super::util::{is_permission, NameSample};

const CHECK: &str = "archive_backlog";

/// More `.ready` files than this is a warning. The number is an
/// **assumption**, not a PostgreSQL default.
pub(crate) const READY_WARN_THRESHOLD: u64 = 100;
/// Names listed in evidence before "and N more".
const EVIDENCE_NAMES: usize = 5;

#[derive(Debug, Default, Clone)]
pub(crate) struct BacklogFacts {
    pub ready: Option<u64>,
    pub backlog_bytes: Option<u64>,
}

pub(crate) fn check(root: &Root, facts: &mut BacklogFacts) -> Vec<Finding> {
    let dir = root.join("/pg_wal/archive_status");
    let wal = root.join("/pg_wal");
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return vec![Finding::new(
                CHECK,
                Status::NotPresent,
                None,
                None,
                format!("{} does not exist", dir.display()),
                "archive_status is created by PostgreSQL; none means no archiver state to read",
            )];
        }
        Err(err) if is_permission(&err) => {
            return vec![Finding::new(
                CHECK,
                Status::NeedsRoot,
                None,
                None,
                format!("cannot read {}", dir.display()),
                "run as the PostgreSQL OS user or as root",
            )];
        }
        Err(err) => {
            return vec![Finding::new(
                CHECK,
                Status::Error,
                None,
                None,
                format!("cannot read {}: {err}", dir.display()),
                "",
            )];
        }
    };

    let mut ready = NameSample::default();
    let mut orphans = NameSample::default();
    let mut done = 0u64;
    let mut backlog_bytes = 0u64;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => {
                return vec![Finding::new(
                    CHECK,
                    Status::Error,
                    None,
                    None,
                    format!("cannot read {}: {err}", dir.display()),
                    "",
                )];
            }
        };
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".done") {
            done += 1;
        } else if let Some(base) = name.strip_suffix(".ready") {
            ready.push(name.to_string(), EVIDENCE_NAMES);
            if is_segment_name(base) {
                match fs::symlink_metadata(wal.join(base)) {
                    Ok(meta) => backlog_bytes = backlog_bytes.saturating_add(meta.len()),
                    Err(err) if err.kind() == io::ErrorKind::NotFound => {
                        orphans.push(name.to_string(), EVIDENCE_NAMES);
                    }
                    Err(_) => {}
                }
            }
        }
    }

    let ready_count = ready.total();
    facts.ready = Some(ready_count);
    facts.backlog_bytes = Some(backlog_bytes);

    let mut findings = Vec::new();
    let status = if ready_count > READY_WARN_THRESHOLD {
        Status::Warn
    } else {
        Status::Ok
    };
    let evidence = if ready_count == 0 {
        format!("0 .ready files, {done} .done files")
    } else {
        format!(
            "{ready_count} .ready files ({}), {done} .done files",
            ready.render()
        )
    };
    findings.push(Finding::new(
        CHECK,
        status,
        Some(backlog_bytes),
        Some(ready_count),
        evidence,
        format!(
            "bytes = size of the segments that still have a .ready file. Warn above \
             {READY_WARN_THRESHOLD} .ready files (assumed threshold). \
             Meaning of .ready/.done is an assumption; check archive_command and the server log"
        ),
    ));
    if orphans.total() > 0 {
        findings.push(Finding::new(
            CHECK,
            Status::Warn,
            None,
            Some(orphans.total()),
            format!(
                "{} .ready files have no matching segment in pg_wal: {}",
                orphans.total(),
                orphans.render()
            ),
            "a .ready file without its segment is unexpected; do not delete it by hand, \
             look at the PostgreSQL server log first",
        ));
    }
    findings
}
