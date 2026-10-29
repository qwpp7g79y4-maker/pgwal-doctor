//! Read-only report of why PostgreSQL's `pg_wal` keeps growing.
//!
//! The tool never writes to PostgreSQL or to the data directory. Offline
//! mode only reads files below a [`Root`]. Online mode (added later) only
//! runs `SELECT` queries through a `psql` subprocess.

#![forbid(unsafe_code)]

mod checks;
mod report;
mod root;
mod walname;

pub use report::{escape, Finding, Report, Status};
pub use root::Root;
pub use walname::is_segment_name;

/// Run every offline check against the data directory `datadir`.
pub fn scan(datadir: Option<&Root>, _source: Option<&()>) -> Report {
    let mut findings = Vec::new();
    let mut facts = checks::WalDirFacts::default();
    if let Some(root) = datadir {
        findings.extend(checks::wal_dir(root, &mut facts));
        findings.extend(checks::pg_replslot(root));
    }
    Report::from_findings(findings)
}
