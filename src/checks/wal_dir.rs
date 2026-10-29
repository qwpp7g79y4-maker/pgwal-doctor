//! Offline: size and shape of `<datadir>/pg_wal`.

use std::fs;
use std::io;
use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::report::{Finding, Status};
use crate::root::Root;
use crate::walname::is_segment_name;

use super::util::is_permission;

const CHECK: &str = "wal_dir";

/// Numbers other checks reuse.
#[derive(Debug, Default, Clone)]
pub(crate) struct WalDirFacts {
    pub total_bytes: Option<u64>,
}

pub(crate) fn check(root: &Root, facts: &mut WalDirFacts) -> Vec<Finding> {
    let wal = root.join("/pg_wal");
    let meta = match fs::symlink_metadata(&wal) {
        Ok(meta) => meta,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return vec![Finding::new(
                CHECK,
                Status::NotPresent,
                None,
                None,
                format!("{} does not exist", wal.display()),
                "is --datadir the PostgreSQL data directory?",
            )];
        }
        Err(err) if is_permission(&err) => return vec![needs_access(&wal)],
        Err(err) => return vec![read_error(&wal, &err)],
    };

    let mut notes = Vec::new();
    if meta.file_type().is_symlink() {
        match symlink_note(root, &wal) {
            Ok(note) => notes.push(note),
            Err(finding) => return vec![finding],
        }
    }

    let entries = match fs::read_dir(&wal) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return vec![Finding::new(
                CHECK,
                Status::NotPresent,
                None,
                None,
                format!("{} is a dangling symlink", wal.display()),
                "the pg_wal symlink points to a directory that does not exist",
            )];
        }
        Err(err) if is_permission(&err) => return vec![needs_access(&wal)],
        Err(err) => return vec![read_error(&wal, &err)],
    };

    let mut segments = 0u64;
    let mut others = 0u64;
    let mut bytes = 0u64;
    let mut oldest: Option<u64> = None;
    let mut newest: Option<u64> = None;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) if is_permission(&err) => return vec![needs_access(&wal)],
            Err(err) => return vec![read_error(&wal, &err)],
        };
        let meta = match fs::symlink_metadata(entry.path()) {
            Ok(meta) => meta,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) if is_permission(&err) => return vec![needs_access(&wal)],
            Err(err) => return vec![read_error(&wal, &err)],
        };
        if !meta.is_file() {
            continue;
        }
        bytes = bytes.saturating_add(meta.len());
        let name = entry.file_name();
        if name.to_str().is_some_and(is_segment_name) {
            segments += 1;
            if let Ok(mtime) = meta.modified() {
                if let Ok(since) = mtime.duration_since(UNIX_EPOCH) {
                    let secs = since.as_secs();
                    oldest = Some(oldest.map_or(secs, |value| value.min(secs)));
                    newest = Some(newest.map_or(secs, |value| value.max(secs)));
                }
            }
        } else {
            others += 1;
        }
    }

    facts.total_bytes = Some(bytes);
    let mut evidence = format!("{segments} segments, {bytes} bytes");
    if others > 0 {
        evidence.push_str(&format!(
            ", {others} other files (history, partial, backup or unknown names)"
        ));
    }
    if let (Some(old), Some(new)) = (oldest, newest) {
        evidence.push_str(&format!(", segment mtime unix {old}..{new}"));
    }
    for note in &notes {
        evidence.push_str("; ");
        evidence.push_str(note);
    }
    let hint = if segments == 0 {
        "pg_wal without segments is unusual for a data directory that was ever started"
    } else {
        "size alone is not a problem: min_wal_size keeps recycled files even when idle; \
         see the other findings for what holds WAL back"
    };
    vec![Finding::new(
        CHECK,
        Status::Ok,
        Some(bytes),
        Some(segments),
        evidence,
        hint,
    )]
}

/// Describe a symlinked `pg_wal`. Relative targets must stay inside the
/// data directory; absolute targets are followed as written.
fn symlink_note(root: &Root, wal: &Path) -> Result<String, Finding> {
    let target = fs::read_link(wal).map_err(|err| read_error(wal, &err))?;
    if target.is_absolute() {
        return Ok(format!("pg_wal is a symlink to {}", target.display()));
    }
    let resolved = match fs::canonicalize(wal) {
        Ok(path) => path,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(format!("pg_wal is a symlink to {}", target.display()))
        }
        Err(err) => return Err(read_error(wal, &err)),
    };
    let prefix = fs::canonicalize(root.prefix()).map_err(|err| read_error(root.prefix(), &err))?;
    if resolved.starts_with(&prefix) {
        Ok(format!("pg_wal is a symlink to {}", target.display()))
    } else {
        Err(Finding::new(
            CHECK,
            Status::Error,
            None,
            None,
            format!(
                "pg_wal is a relative symlink to {} which leaves the data directory; not followed",
                target.display()
            ),
            "point --datadir at a directory whose pg_wal link stays inside it, or use an absolute link",
        ))
    }
}

/// Offline view of `pg_replslot`: only the number of slot directories.
pub(crate) fn pg_replslot(root: &Root) -> Vec<Finding> {
    let dir = root.join("/pg_replslot");
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return vec![Finding::new(
                "pg_replslot",
                Status::NotPresent,
                None,
                None,
                format!("{} does not exist", dir.display()),
                "",
            )];
        }
        Err(err) if is_permission(&err) => {
            return vec![Finding::new(
                "pg_replslot",
                Status::NeedsRoot,
                None,
                None,
                format!("cannot read {}", dir.display()),
                "run as the PostgreSQL OS user or as root",
            )];
        }
        Err(err) => return vec![read_error(&dir, &err)],
    };
    let mut count = 0u64;
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            count += 1;
        }
    }
    vec![Finding::new(
        "pg_replslot",
        Status::Ok,
        None,
        Some(count),
        format!(
            "pg_replslot has {count} slot directories (directory names are slot names: assumption)"
        ),
        "offline mode cannot say whether a slot holds WAL back: the state file is not parsed; \
         use --dsn for that",
    )]
}

fn needs_access(path: &Path) -> Finding {
    Finding::new(
        CHECK,
        Status::NeedsRoot,
        None,
        None,
        format!("cannot read {}", path.display()),
        "run as the PostgreSQL OS user or as root",
    )
}

fn read_error(path: &Path, err: &io::Error) -> Finding {
    Finding::new(
        CHECK,
        Status::Error,
        None,
        None,
        format!("cannot read {}: {err}", path.display()),
        "",
    )
}
