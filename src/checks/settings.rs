//! Online: WAL retention settings, converted to bytes.
//!
//! The settings are context, not a verdict, so the finding is `Ok`. Any
//! threshold on them would be an assumption.

use crate::report::{Finding, Status};
use crate::source::{Rows, SourceError};
use crate::units::{to_bytes, to_seconds};

use super::online::error_finding;

const CHECK: &str = "settings";

/// Default WAL segment size when the server does not report one. That 16 MB
/// is the default is an **assumption** (not read from the docs page).
const DEFAULT_SEGMENT_BYTES: u64 = 16 << 20;

#[derive(Debug, Default, Clone)]
pub(crate) struct SettingsFacts {
    /// WAL kept for standbys: `wal_keep_size`, or `wal_keep_segments` times
    /// the segment size on PG 12.
    pub wal_keep_bytes: Option<u64>,
}

struct Row<'a> {
    setting: &'a str,
    unit: Option<&'a str>,
}

fn find<'a>(rows: &'a Rows, name: &str) -> Option<Row<'a>> {
    let index = (0..rows.len()).find(|&i| rows.get(i, "name").value() == Some(name))?;
    Some(Row {
        setting: rows.get(index, "setting").value()?,
        unit: rows.get(index, "unit").value(),
    })
}

fn bytes(rows: &Rows, name: &str) -> Option<u64> {
    let row = find(rows, name)?;
    to_bytes(row.setting.parse().ok()?, row.unit)
}

pub(crate) fn check(
    settings: &Result<Rows, SourceError>,
    facts: &mut SettingsFacts,
) -> Vec<Finding> {
    let rows = match settings {
        Ok(rows) => rows,
        Err(err) => {
            return vec![error_finding(
                CHECK,
                err,
                "pg_settings is readable by every role",
            )]
        }
    };
    if rows.is_empty() {
        return vec![Finding::new(
            CHECK,
            Status::NotPresent,
            None,
            None,
            "pg_settings returned none of the WAL settings",
            "",
        )];
    }

    let mut parts: Vec<String> = Vec::new();
    for name in ["max_wal_size", "min_wal_size"] {
        if let Some(value) = bytes(rows, name) {
            parts.push(format!("{name}={value} bytes"));
        }
    }
    let segment = bytes(rows, "wal_segment_size");
    if let Some(value) = segment {
        parts.push(format!("wal_segment_size={value} bytes"));
    }

    if let Some(value) = bytes(rows, "wal_keep_size") {
        facts.wal_keep_bytes = Some(value);
        parts.push(format!(
            "wal_keep_size={value} bytes, wal_keep_bytes={value}"
        ));
    } else if let Some(row) = find(rows, "wal_keep_segments") {
        if let Ok(count) = row.setting.parse::<u64>() {
            let size = segment.unwrap_or(DEFAULT_SEGMENT_BYTES);
            let total = count.saturating_mul(size);
            facts.wal_keep_bytes = Some(total);
            let assumed = if segment.is_none() {
                " (segment size assumed 16 MB)"
            } else {
                ""
            };
            parts.push(format!(
                "wal_keep_segments={count}, wal_keep_bytes={total} at {size}-byte segments{assumed}"
            ));
        }
    }

    if let Some(row) = find(rows, "max_slot_wal_keep_size") {
        match row.setting.parse::<i64>() {
            Ok(value) if value < 0 => parts.push("max_slot_wal_keep_size=unlimited".to_string()),
            Ok(value) => {
                if let Some(total) = to_bytes(value, row.unit) {
                    parts.push(format!("max_slot_wal_keep_size={total} bytes"));
                }
            }
            Err(_) => {}
        }
    } else {
        parts.push("max_slot_wal_keep_size not available on this server version".to_string());
    }

    if let Some(row) = find(rows, "checkpoint_timeout") {
        if let Some(secs) = row
            .setting
            .parse()
            .ok()
            .and_then(|v| to_seconds(v, row.unit))
        {
            parts.push(format!("checkpoint_timeout={secs} s"));
        }
    }

    vec![Finding::new(
        CHECK,
        Status::Ok,
        None,
        None,
        parts.join(", "),
        "Context only. min_wal_size keeps recycled files even when idle, so a large pg_wal \
         is not by itself a problem; max_wal_size is a soft limit that failing archiving, \
         slots or wal_keep_size can push past. Thresholds on these values would be assumptions.",
    )]
}
