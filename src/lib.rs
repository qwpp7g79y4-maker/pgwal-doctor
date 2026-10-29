//! Read-only report of why PostgreSQL's `pg_wal` keeps growing.
//!
//! The tool never writes to PostgreSQL or to the data directory. Offline
//! mode only reads files below a [`Root`]. Online mode (added later) only
//! runs `SELECT` queries through a `psql` subprocess.

#![forbid(unsafe_code)]

mod checks;
mod psql;
mod report;
mod root;
mod source;
mod walname;

pub use psql::{dsn_has_password, PsqlSource};
pub use report::{escape, Finding, Report, Status};
pub use root::Root;
pub use source::{Cell, FixtureSource, PgSource, Rows, SourceError};
pub use walname::is_segment_name;

/// Run every offline check against the data directory `datadir`.
pub fn scan(datadir: Option<&Root>, _source: Option<&()>) -> Report {
    let mut findings = Vec::new();
    let mut wal_facts = checks::WalDirFacts::default();
    let mut backlog_facts = checks::BacklogFacts::default();
    if let Some(root) = datadir {
        findings.extend(checks::wal_dir(root, &mut wal_facts));
        findings.extend(checks::archive_backlog(root, &mut backlog_facts));
        findings.extend(checks::pg_replslot(root));
    }
    Report::from_findings(findings)
}
