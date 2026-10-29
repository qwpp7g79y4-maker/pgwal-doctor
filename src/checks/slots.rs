//! Online: replication slots that hold WAL back.
//!
//! Statuses and columns follow the PostgreSQL 17 documentation of
//! `pg_replication_slots`. Older servers lack some columns; a missing column
//! is reported as "not available", never as an error. That `wal_status`
//! arrived in PG 13 is an assumption.

use crate::lsn::{lag_bytes, parse_lsn};
use crate::report::{Finding, Status};
use crate::source::{Cell, Rows, SourceError};

use super::online::{error_finding, setting};

const CHECK: &str = "slots";

/// An inactive slot retaining at least this much is a warning. 1 GiB equals
/// the default `max_wal_size`; the number is an **assumption**.
pub(crate) const SLOT_LAG_WARN_BYTES: u64 = 1 << 30;

#[derive(Debug, Default, Clone)]
pub(crate) struct SlotFacts {
    /// Largest retained distance over all slots. Slots overlap, so this is
    /// an upper bound for what slots keep, not a sum.
    pub retained_max: Option<u64>,
}

pub(crate) fn check(
    slots: &Result<Rows, SourceError>,
    settings: Option<&Rows>,
    facts: &mut SlotFacts,
) -> Vec<Finding> {
    let rows = match slots {
        Ok(rows) => rows,
        Err(err) => {
            return vec![error_finding(
                CHECK,
                err,
                "GRANT pg_monitor TO <login role>; (pg_replication_slots needs no superuser: \
                 whether it needs any privilege is unknown)",
            )]
        }
    };
    if rows.is_empty() {
        return vec![Finding::new(
            CHECK,
            Status::Ok,
            None,
            Some(0),
            "no replication slots",
            "",
        )];
    }

    let mut findings = Vec::new();
    let mut inactive = 0u64;
    let mut max_lag: Option<u64> = None;
    for row in 0..rows.len() {
        let name = rows.get(row, "slot_name").value().unwrap_or("?");
        let kind = rows.get(row, "slot_type").value().unwrap_or("?");
        let active = rows.get(row, "active").value() == Some("t");
        if !active {
            inactive += 1;
        }
        let restart = rows.get(row, "restart_lsn").value();
        let current = rows.get(row, "current_lsn").value();
        let lag = match (restart.and_then(parse_lsn), current.and_then(parse_lsn)) {
            (Some(restart), Some(current)) => Some(lag_bytes(current, restart)),
            _ => None,
        };
        if let Some(lag) = lag {
            max_lag = Some(max_lag.map_or(lag, |value| value.max(lag)));
        }

        let mut evidence = format!(
            "slot {name} ({kind}, {})",
            if active { "active" } else { "inactive" }
        );
        match lag {
            Some(lag) => evidence.push_str(&format!(": retains {lag} bytes")),
            None => evidence.push_str(": retained bytes unknown (restart_lsn is NULL)"),
        }
        let mut status = Status::Ok;
        let mut hint = String::new();

        let wal_status = rows.get(row, "wal_status");
        match wal_status {
            Cell::Missing => {
                evidence.push_str(", wal_status not available on this server version");
            }
            Cell::Null => evidence.push_str(", wal_status NULL"),
            Cell::Value(value) => {
                evidence.push_str(&format!(", wal_status={value}"));
                match value {
                    "lost" => {
                        status = Status::Warn;
                        hint.push_str(
                            "wal_status=lost: the slot is no longer usable and cannot be \
                             recovered; the standby or subscriber must be rebuilt. ",
                        );
                    }
                    "unreserved" => {
                        status = Status::Warn;
                        hint.push_str(
                            "wal_status=unreserved: the slot no longer holds WAL back; the files \
                             go at the next checkpoint unless the slot catches up. ",
                        );
                    }
                    "extended" => {
                        status = Status::Warn;
                        hint.push_str(
                            "wal_status=extended: max_wal_size is exceeded but the slot or \
                             wal_keep_size still holds WAL. ",
                        );
                    }
                    _ => {}
                }
            }
        }
        if let Cell::Value(size) = rows.get(row, "safe_wal_size") {
            evidence.push_str(&format!(", safe_wal_size={size}"));
        }
        if let Cell::Value(since) = rows.get(row, "inactive_since") {
            evidence.push_str(&format!(", inactive_since={since}"));
        }
        if let Cell::Value(reason) = rows.get(row, "invalidation_reason") {
            evidence.push_str(&format!(", invalidation_reason={reason}"));
            status = Status::Warn;
            hint.push_str("the slot was invalidated by the server. ");
        }
        if !active && lag.is_some_and(|lag| lag >= SLOT_LAG_WARN_BYTES) {
            status = Status::Warn;
            hint.push_str(&format!(
                "inactive slot retaining at least {SLOT_LAG_WARN_BYTES} bytes (assumed threshold). "
            ));
        }
        hint.push_str(
            "Check that the standby or subscriber is really gone before dropping the slot; \
             this tool never drops slots.",
        );
        findings.push(Finding::new(CHECK, status, lag, None, evidence, hint));
    }
    facts.retained_max = max_lag;

    if inactive > 0
        && settings.and_then(|rows| setting(rows, "max_slot_wal_keep_size")) == Some("-1")
    {
        findings.push(Finding::new(
            CHECK,
            Status::Warn,
            None,
            Some(inactive),
            format!(
                "max_slot_wal_keep_size=-1 (unlimited) with {inactive} inactive slot(s): WAL kept \
                 by an inactive slot is not capped"
            ),
            "set max_slot_wal_keep_size in postgresql.conf (PG 13 and later); slots that fall \
             behind the limit are then invalidated instead of filling pg_wal",
        ));
    }
    findings
}
