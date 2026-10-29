//! Read-only report of why PostgreSQL's `pg_wal` keeps growing.
//!
//! The tool never writes to PostgreSQL or to the data directory. Offline
//! mode only reads files below a [`Root`]. Online mode only
//! runs `SELECT` queries through a `psql` subprocess.

#![forbid(unsafe_code)]

mod checks;
mod lsn;
mod psql;
mod report;
mod root;
mod source;
mod units;
mod walname;

pub use lsn::{lag_bytes, parse_lsn};
pub use psql::{dsn_has_password, PsqlSource};
pub use report::{escape, Finding, Report, Status};
pub use root::Root;
pub use source::{Cell, FixtureSource, PgSource, Rows, SourceError};
pub use units::{to_bytes, to_seconds};
pub use walname::is_segment_name;

/// Run the offline checks on `datadir` and the online checks on `source`.
/// Either may be absent.
pub fn scan(datadir: Option<&Root>, source: Option<&dyn PgSource>) -> Report {
    let mut findings = Vec::new();
    let mut wal_facts = checks::WalDirFacts::default();
    let mut backlog_facts = checks::BacklogFacts::default();
    let mut waldir_facts = checks::WaldirFacts::default();
    let mut inputs = checks::SummaryInputs::default();
    if let Some(root) = datadir {
        findings.extend(checks::wal_dir(root, &mut wal_facts));
        findings.extend(checks::archive_backlog(root, &mut backlog_facts));
        findings.extend(checks::pg_replslot(root));
    }
    if let Some(source) = source {
        let settings = source.settings();
        let mut settings_facts = checks::SettingsFacts::default();
        findings.extend(checks::settings(&settings, &mut settings_facts));
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
        findings.extend(checks::transactions(&source.activity()));
        findings.extend(checks::prepared(&source.prepared(), settings.as_ref().ok()));
        findings.extend(checks::waldir(&source.waldir(), &mut waldir_facts));
        inputs.slot_retained = slot_facts.retained_max;
        inputs.wal_keep = settings_facts.wal_keep_bytes;
    }
    inputs.total = waldir_facts.total_bytes.or(wal_facts.total_bytes);
    inputs.archive_backlog = backlog_facts.backlog_bytes;
    findings.extend(checks::summary(&inputs));
    Report::from_findings(findings)
}
