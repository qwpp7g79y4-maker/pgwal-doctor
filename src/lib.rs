//! Read-only report of why PostgreSQL's `pg_wal` keeps growing.
//!
//! The tool never writes to PostgreSQL or to the data directory. Offline
//! mode only reads files below a [`Root`]. Online mode (added later) only
//! runs `SELECT` queries through a `psql` subprocess.

#![forbid(unsafe_code)]

mod report;
mod root;

pub use report::{escape, Finding, Report, Status};
pub use root::Root;

/// Run every offline check against the data directory `datadir`.
pub fn scan(datadir: Option<&Root>) -> Report {
    let findings = Vec::new();
    let _ = datadir;
    Report::from_findings(findings)
}
