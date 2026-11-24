//! Online: is WAL archiving failing or stuck?
//!
//! Sources: PostgreSQL 17 docs for `pg_stat_archiver` and continuous
//! archiving. A clean `pg_stat_archiver` is not proof that archiving works:
//! when the archive command dies from a signal or exits above 125 (or the
//! library raises ERROR or FATAL), the archiver restarts and the failure is
//! not counted. So the `.ready` backlog from the data directory (when given)
//! is looked at as well.

use crate::report::{Finding, Status};
use crate::source::{Rows, SourceError};

use super::archive_backlog::READY_WARN_THRESHOLD;
use super::online::{error_finding, setting};

const CHECK: &str = "archiver";

const NOT_PROOF: &str = "A clean pg_stat_archiver does not prove that archiving works: if the \
    archive command is killed by a signal or exits above 125, the archiver restarts and the \
    failure is not reported (PostgreSQL docs).";

pub(crate) fn check(
    archiver: &Result<Rows, SourceError>,
    settings: Option<&Rows>,
    ready_backlog: Option<u64>,
) -> Vec<Finding> {
    let mode = settings.and_then(|rows| setting(rows, "archive_mode"));
    let rows = match archiver {
        Ok(rows) => rows,
        Err(err) => {
            return vec![error_finding(
                CHECK,
                err,
                "GRANT pg_monitor TO <login role>; (pg_read_all_stats). Whether pg_stat_archiver \
                 needs it is unknown: the docs page was not checked for privileges",
            )]
        }
    };
    if mode == Some("off") {
        return vec![Finding::new(
            CHECK,
            Status::NotPresent,
            None,
            None,
            "archive_mode=off: archiving is not a cause of WAL growth",
            "",
        )];
    }
    if rows.is_empty() {
        return vec![Finding::new(
            CHECK,
            Status::NotPresent,
            None,
            None,
            "pg_stat_archiver returned no row",
            "",
        )];
    }

    let failed: u64 = rows
        .get(0, "failed_count")
        .value()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let archived: u64 = rows
        .get(0, "archived_count")
        .value()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let last_failed_time = rows.get(0, "last_failed_time").value();
    let last_failed_wal = rows.get(0, "last_failed_wal").value();
    let last_archived_time = rows.get(0, "last_archived_time").value();
    let last_archived_wal = rows.get(0, "last_archived_wal").value();

    let mut status = Status::Ok;
    let mut reasons: Vec<String> = Vec::new();
    // The server compares the two timestamptz values (`failed_after_archived`,
    // see `ARCHIVER_SQL`). The displayed text depends on DateStyle and on the
    // time zone offset, so it is never compared here.
    let comparison = rows.get(0, "failed_after_archived").value();
    let failing = failed > 0 && comparison == Some("t");
    if failed > 0 && !matches!(comparison, Some("t" | "f")) {
        status = Status::Warn;
        reasons.push(format!(
            "{failed} failures are counted, but the server gave no comparison of \
             last_failed_time and last_archived_time, so it is not known whether the last \
             failure is newer than the last archive"
        ));
    }
    if failing {
        status = Status::Warn;
        reasons.push(format!(
            "archiving is failing: {failed} failures, last failed WAL {} at {}, last archived {} at {}",
            last_failed_wal.unwrap_or("?"),
            last_failed_time.unwrap_or("?"),
            last_archived_wal.unwrap_or("none"),
            last_archived_time.unwrap_or("never"),
        ));
    }
    if archived == 0 && last_archived_time.is_none() {
        match ready_backlog {
            Some(ready) if ready > 0 => {
                status = Status::Warn;
                reasons.push(format!(
                    "nothing was ever archived, yet {ready} .ready files wait"
                ));
            }
            _ => reasons.push("nothing archived yet".to_string()),
        }
    }
    if !failing {
        if let Some(ready) = ready_backlog.filter(|&ready| ready > READY_WARN_THRESHOLD) {
            status = Status::Warn;
            reasons.push(format!(
                "pg_stat_archiver shows no recent failure but {ready} .ready files wait"
            ));
        }
    }
    let command = settings.and_then(|rows| setting(rows, "archive_command"));
    let library = settings.and_then(|rows| setting(rows, "archive_library"));
    if mode.is_some_and(|mode| mode != "off")
        && command.is_some_and(str::is_empty)
        && library.is_none_or(str::is_empty)
    {
        status = Status::Warn;
        reasons.push(
            "archive_mode is on but neither archive_command nor archive_library is set: WAL \
             piles up (PostgreSQL docs)"
                .to_string(),
        );
    }

    let mut evidence = format!("archived_count={archived}, failed_count={failed}");
    if let Some(mode) = mode {
        evidence.push_str(&format!(", archive_mode={mode}"));
    }
    if library.is_none() && settings.is_some() {
        evidence.push_str(", archive_library not available on this server version");
    }
    for reason in &reasons {
        evidence.push_str("; ");
        evidence.push_str(reason);
    }
    let hint = if status == Status::Warn {
        format!(
            "Read the PostgreSQL server log for the archive command's error. Do not replace the \
             command with /bin/true: that hides the problem and breaks the archive chain. {NOT_PROOF}"
        )
    } else {
        NOT_PROOF.to_string()
    };
    vec![Finding::new(
        CHECK,
        status,
        None,
        Some(failed),
        evidence,
        hint,
    )]
}
