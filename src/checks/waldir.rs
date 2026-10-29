//! Online: size of `pg_wal` from `pg_ls_waldir()`.
//!
//! `pg_ls_waldir()` is limited to superusers and members of `pg_monitor`
//! unless `EXECUTE` was granted (PostgreSQL 17 functions-admin docs).

use crate::report::{Finding, Status};
use crate::source::{Rows, SourceError};
use crate::walname::is_segment_name;

use super::online::error_finding;

const CHECK: &str = "waldir";

#[derive(Debug, Default, Clone)]
pub(crate) struct WaldirFacts {
    /// Sum of the sizes of every file `pg_ls_waldir()` lists.
    pub total_bytes: Option<u64>,
}

pub(crate) fn check(waldir: &Result<Rows, SourceError>, facts: &mut WaldirFacts) -> Vec<Finding> {
    let rows = match waldir {
        Ok(rows) => rows,
        Err(err) => {
            return vec![error_finding(
                CHECK,
                err,
                "GRANT pg_monitor TO <login role>; (pg_ls_waldir() is limited to superusers and \
                 pg_monitor members unless EXECUTE is granted)",
            )]
        }
    };
    let mut segments = 0u64;
    let mut segment_bytes = 0u64;
    let mut other_files = 0u64;
    let mut other_bytes = 0u64;
    let mut unreadable = 0u64;
    for row in 0..rows.len() {
        let name = rows.get(row, "name").value().unwrap_or("");
        let Some(size) = rows
            .get(row, "size")
            .value()
            .and_then(|v| v.parse::<u64>().ok())
        else {
            unreadable += 1;
            continue;
        };
        if is_segment_name(name) {
            segments += 1;
            segment_bytes = segment_bytes.saturating_add(size);
        } else {
            other_files += 1;
            other_bytes = other_bytes.saturating_add(size);
        }
    }
    let total = segment_bytes.saturating_add(other_bytes);
    facts.total_bytes = Some(total);
    let mut evidence = format!(
        "pg_ls_waldir(): {segments} segment file(s) = {segment_bytes} bytes, {other_files} other \
         file(s) (history, partial, backup, ...) = {other_bytes} bytes, total {total} bytes"
    );
    if unreadable > 0 {
        evidence.push_str(&format!(
            "; {unreadable} row(s) with an unreadable size skipped"
        ));
    }
    let hint = if rows.is_empty() {
        evidence.push_str("; the listing is empty");
        "An empty listing is unusual: either pg_ls_waldir() hides files from this role or \
         pg_wal is empty. Compare with --datadir."
    } else {
        ""
    };
    vec![Finding::new(
        CHECK,
        Status::Ok,
        Some(total),
        Some(segments),
        evidence,
        hint,
    )]
}
