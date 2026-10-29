//! Why does `pg_wal` grow? A ranking of what is known, not a verdict.
//!
//! The causes overlap (a slot, the archive backlog and `wal_keep_size` can
//! all keep the same segment), so the figures do not add up. The part of the
//! directory not explained by any measured cause is shown as `other or
//! unknown`; nothing is invented to fill the gap.

use crate::report::{Finding, Status};

const CHECK: &str = "summary";

#[derive(Debug, Default, Clone)]
pub(crate) struct Inputs {
    /// Directory size: `pg_ls_waldir()` if online, else the offline scan.
    pub total: Option<u64>,
    /// Largest distance any slot is behind (upper bound for slots).
    pub slot_retained: Option<u64>,
    /// Bytes of segments with a `.ready` file.
    pub archive_backlog: Option<u64>,
    /// Configured `wal_keep_size` (a minimum kept, not measured use).
    pub wal_keep: Option<u64>,
}

pub(crate) fn check(inputs: &Inputs) -> Vec<Finding> {
    let Some(total) = inputs.total else {
        return Vec::new();
    };
    let mut causes: Vec<(&str, u64)> = Vec::new();
    let mut unmeasured: Vec<&str> = Vec::new();
    for (label, value) in [
        ("slot-retained WAL (largest slot)", inputs.slot_retained),
        ("archive backlog (.ready)", inputs.archive_backlog),
        ("wal_keep_size", inputs.wal_keep),
    ] {
        match value {
            Some(bytes) => causes.push((label, bytes)),
            None => unmeasured.push(label),
        }
    }
    // Largest first; equal sizes keep a fixed order by label.
    causes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let explained = causes
        .iter()
        .map(|&(_, bytes)| bytes)
        .fold(0u64, u64::saturating_add);
    let other = total.saturating_sub(explained);

    let mut parts: Vec<String> = causes
        .iter()
        .map(|&(label, bytes)| format!("{label}: {bytes} bytes"))
        .collect();
    parts.push(format!("other or unknown: {other} bytes"));
    let mut evidence = format!("pg_wal holds {total} bytes. {}", parts.join("; "));
    if explained > total {
        evidence.push_str(&format!(
            ". The causes overlap and sum to {explained} bytes, more than the directory"
        ));
    }
    if !unmeasured.is_empty() {
        evidence.push_str(&format!(". Not measured: {}", unmeasured.join(", ")));
    }
    vec![Finding::new(
        CHECK,
        Status::Ok,
        Some(total),
        None,
        evidence,
        "Ranking by size, not proof. Causes overlap, so the figures need not add up. Slot lag \
         is the largest single slot (upper bound). wal_keep_size is a configured minimum, not \
         measured use. What .ready means for the backlog, and the thresholds of the other \
         checks, are assumptions (see README). A large pg_wal can also be harmless: \
         min_wal_size keeps recycled files.",
    )]
}
