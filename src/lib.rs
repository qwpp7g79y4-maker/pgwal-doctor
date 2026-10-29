//! Read-only report of why PostgreSQL's `pg_wal` keeps growing.
//!
//! The tool never writes to PostgreSQL or to the data directory. Offline
//! mode only reads files below a [`Root`]. Online mode (added later) only
//! runs `SELECT` queries through a `psql` subprocess.

#![forbid(unsafe_code)]

mod checks;
mod lsn;
mod psql;
mod report;
mod root;
mod source;
mod walname;

pub use lsn::{lag_bytes, parse_lsn};
pub use psql::{dsn_has_password, PsqlSource};
pub use report::{escape, Finding, Report, Status};
pub use root::Root;
pub use source::{Cell, FixtureSource, PgSource, Rows, SourceError};
pub use walname::is_segment_name;

/// Run the offline checks on `datadir` and the online checks on `source`.
/// Either may be absent.
pub fn scan(datadir: Option<&Root>, source: Option<&dyn PgSource>) -> Report {
    let mut findings = Vec::new();
    let mut wal_facts = checks::WalDirFacts::default();
    let mut backlog_facts = checks::BacklogFacts::default();
    if let Some(root) = datadir {
        findings.extend(checks::wal_dir(root, &mut wal_facts));
        findings.extend(checks::archive_backlog(root, &mut backlog_facts));
        findings.extend(checks::pg_replslot(root));
    }
    if let Some(source) = source {
        let settings = source.settings();
        let mut slot_facts = checks::SlotFacts::default();
        findings.extend(checks::slots(
            &source.slots(),
            settings.as_ref().ok(),
            &mut slot_facts,
        ));
        findings.extend(checks::archiver(
            &source.archiver(),
            settings.as_ref().ok(),
            backlog_facts.ready,
        ));
    }
    Report::from_findings(findings)
}
