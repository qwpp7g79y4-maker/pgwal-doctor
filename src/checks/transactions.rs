//! Online: open and prepared transactions, as context only.
//!
//! The PostgreSQL docs say prepared transactions hold locks and hinder
//! VACUUM; they do not say these transactions hold WAL back. That a long
//! transaction causes bloat and thereby more WAL is an **assumption**, so
//! the wording here is "can hinder VACUUM", never "holds WAL".

use crate::report::{Finding, Status};
use crate::source::{Rows, SourceError};

use super::online::{error_finding, setting};

const SESSIONS: &str = "transactions";
const PREPARED: &str = "prepared_xacts";

/// A transaction older than this is a warning. One hour is an **assumption**.
pub(crate) const TXN_AGE_WARN_SECS: u64 = 3600;

const CONTEXT: &str = "Context, not a proven cause of WAL growth: it is an assumption that old \
    transactions add to WAL; the PostgreSQL docs only say they can hinder VACUUM.";

fn human(secs: u64) -> String {
    let (d, h, m) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60);
    match (d, h) {
        (0, 0) => format!("{secs} s ({m} min)"),
        (0, _) => format!("{secs} s ({h} h {m} min)"),
        _ => format!("{secs} s ({d} d {h} h)"),
    }
}

fn age(rows: &Rows, row: usize, column: &str) -> Option<u64> {
    rows.get(row, column).value()?.parse().ok()
}

pub(crate) fn sessions(activity: &Result<Rows, SourceError>) -> Vec<Finding> {
    let rows = match activity {
        Ok(rows) => rows,
        Err(err) => {
            return vec![error_finding(
                SESSIONS,
                err,
                "GRANT pg_monitor TO <login role>; (pg_read_all_stats shows other users' sessions)",
            )]
        }
    };
    let mut findings = Vec::new();
    let mut oldest: Option<(usize, u64)> = None;
    let mut open = 0u64;
    let mut hidden = 0u64;
    for row in 0..rows.len() {
        // Without pg_read_all_stats the server blanks other users' rows:
        // an assumption, from the monitoring-stats docs summary.
        if rows.get(row, "state").value().is_none() {
            hidden += 1;
            continue;
        }
        if rows.get(row, "xact_start").value().is_none() {
            continue;
        }
        open += 1;
        if let Some(secs) = age(rows, row, "xact_age_s") {
            if oldest.is_none_or(|(_, best)| secs > best) {
                oldest = Some((row, secs));
            }
        }
    }

    let (status, evidence) = match oldest {
        Some((row, secs)) => {
            let text = format!(
                "{open} open transaction(s); oldest: pid {} ({}, user {}) for {}, backend_xmin {}",
                rows.get(row, "pid").value().unwrap_or("?"),
                rows.get(row, "state").value().unwrap_or("?"),
                rows.get(row, "usename").value().unwrap_or("?"),
                human(secs),
                rows.get(row, "backend_xmin").value().unwrap_or("none"),
            );
            let status = if secs >= TXN_AGE_WARN_SECS {
                Status::Warn
            } else {
                Status::Ok
            };
            (status, text)
        }
        None => (Status::Ok, format!("{open} open transaction(s)")),
    };
    let hint = if status == Status::Warn {
        format!(
            "Look at what this session is doing; an old transaction can hinder VACUUM and \
             cause bloat. This tool never ends sessions. {CONTEXT}"
        )
    } else {
        CONTEXT.to_string()
    };
    findings.push(Finding::new(
        SESSIONS,
        status,
        None,
        Some(open),
        evidence,
        hint,
    ));
    if hidden > 0 {
        findings.push(Finding::new(
            SESSIONS,
            Status::NeedsRoot,
            None,
            Some(hidden),
            format!("{hidden} session(s) of other users are hidden (no transaction data)"),
            "GRANT pg_monitor TO <login role>; (pg_read_all_stats) to see other users' \
             transactions. Whether this is the exact privilege needed is an assumption.",
        ));
    }
    findings
}

pub(crate) fn prepared(
    prepared: &Result<Rows, SourceError>,
    settings: Option<&Rows>,
) -> Vec<Finding> {
    let rows = match prepared {
        Ok(rows) => rows,
        Err(err) => {
            return vec![error_finding(
                PREPARED,
                err,
                "GRANT pg_monitor TO <login role>; whether pg_prepared_xacts is filtered by \
                 privilege is unknown (the docs page names no filter)",
            )]
        }
    };
    if rows.is_empty() {
        let disabled =
            settings.and_then(|rows| setting(rows, "max_prepared_transactions")) == Some("0");
        let evidence = if disabled {
            "max_prepared_transactions=0: prepared transactions cannot exist"
        } else {
            "no prepared transactions"
        };
        return vec![Finding::new(
            PREPARED,
            if disabled {
                Status::NotPresent
            } else {
                Status::Ok
            },
            None,
            Some(0),
            evidence,
            "",
        )];
    }
    let mut oldest: Option<(usize, u64)> = None;
    for row in 0..rows.len() {
        if let Some(secs) = age(rows, row, "age_s") {
            if oldest.is_none_or(|(_, best)| secs > best) {
                oldest = Some((row, secs));
            }
        }
    }
    let count = rows.len() as u64;
    let (status, evidence) = match oldest {
        Some((row, secs)) => (
            if secs >= TXN_AGE_WARN_SECS {
                Status::Warn
            } else {
                Status::Ok
            },
            format!(
                "{count} prepared transaction(s); oldest: gid {} (owner {}, database {}) prepared {} ago",
                rows.get(row, "gid").value().unwrap_or("?"),
                rows.get(row, "owner").value().unwrap_or("?"),
                rows.get(row, "database").value().unwrap_or("?"),
                human(secs),
            ),
        ),
        None => (
            Status::Ok,
            format!("{count} prepared transaction(s), age unknown"),
        ),
    };
    let hint = if status == Status::Warn {
        format!(
            "A forgotten prepared transaction keeps its locks and can hinder VACUUM (PostgreSQL \
             docs). Find the owner before COMMIT PREPARED or ROLLBACK PREPARED; this tool \
             never does either. {CONTEXT}"
        )
    } else {
        CONTEXT.to_string()
    };
    vec![Finding::new(
        PREPARED,
        status,
        None,
        Some(count),
        evidence,
        hint,
    )]
}
