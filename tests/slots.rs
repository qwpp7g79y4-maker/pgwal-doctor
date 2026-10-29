//! Online `slots` check against recorded-style query output.

mod common;

use common::{all, fixture, only};
use pgwal_doctor::{scan, Finding, FixtureSource, Report, Status};

fn run(scenario: &str) -> Report {
    let source = FixtureSource::new(fixture("pg").join(scenario));
    scan(None, Some(&source))
}

fn slot<'a>(report: &'a Report, name: &str) -> &'a Finding {
    let needle = format!("slot {name} ");
    all(report, "slots")
        .into_iter()
        .find(|f| f.evidence.starts_with(&needle))
        .unwrap_or_else(|| panic!("no finding for slot {name}: {report:?}"))
}

#[test]
fn inactive_slot_with_large_lag_warns_with_bytes() {
    let report = run("pg13");
    let finding = slot(&report, "standby1");
    assert_eq!(finding.status, Status::Warn);
    // 5/00000000 - 3/00000000 = 2 * 4 GiB.
    assert_eq!(finding.bytes, Some(8_589_934_592));
    assert!(finding.evidence.contains("retains 8589934592 bytes"));
    let active = slot(&report, "standby2");
    assert_eq!(active.status, Status::Ok);
    assert_eq!(active.bytes, Some(65_536));
}

#[test]
fn wal_status_lost_warns_and_says_unrecoverable() {
    let report = run("pg17-wal-status");
    let finding = slot(&report, "s_lost");
    assert_eq!(finding.status, Status::Warn);
    assert!(finding.evidence.contains("wal_status=lost"));
    assert!(finding.hint.contains("cannot be recovered"));
    assert!(finding.hint.contains("never drops"));
}

#[test]
fn wal_status_extended_and_unreserved_warn() {
    let report = run("pg17-wal-status");
    let extended = slot(&report, "s_extended");
    assert_eq!(extended.status, Status::Warn);
    assert!(extended.evidence.contains("wal_status=extended"));
    let unreserved = slot(&report, "s_unreserved");
    assert_eq!(unreserved.status, Status::Warn);
    assert!(unreserved.hint.contains("no longer holds WAL"));
}

#[test]
fn pg12_without_wal_status_falls_back_to_lag() {
    let report = run("pg12");
    let finding = slot(&report, "standby1");
    assert_eq!(finding.status, Status::Warn);
    assert_eq!(finding.bytes, Some(17_179_869_184));
    assert!(finding.evidence.contains("wal_status not available"));
    assert!(!finding.evidence.contains("wal_status="));
}

#[test]
fn pg13_has_wal_status_but_no_inactive_since() {
    let report = run("pg13");
    let finding = slot(&report, "standby1");
    assert!(finding.evidence.contains("wal_status=extended"));
    assert!(!finding.evidence.contains("inactive_since"));
}

#[test]
fn pg17_uses_inactive_since_and_invalidation_reason() {
    let report = run("pg17");
    let old = slot(&report, "old_standby");
    assert!(old
        .evidence
        .contains("inactive_since=2026-10-20 08:00:00+00"));
    let dead = slot(&report, "dead_sub");
    assert_eq!(dead.status, Status::Warn);
    assert!(dead.evidence.contains("invalidation_reason=wal_removed"));
    assert_eq!(dead.bytes, None);
    let active = slot(&report, "sub1");
    assert_eq!(active.status, Status::Ok);
}

#[test]
fn unlimited_max_slot_wal_keep_size_with_inactive_slot_warns() {
    let report = run("pg15");
    let finding = all(&report, "slots")
        .into_iter()
        .find(|f| f.evidence.contains("max_slot_wal_keep_size=-1"))
        .expect("unlimited finding");
    assert_eq!(finding.status, Status::Warn);
    assert_eq!(finding.count, Some(1));
    // A finite limit does not trigger it.
    let finite = run("pg13");
    assert!(all(&finite, "slots")
        .iter()
        .all(|f| !f.evidence.contains("max_slot_wal_keep_size")));
}

#[test]
fn permission_denied_is_needs_root() {
    let report = run("pg17/denied");
    let finding = only(&report, "slots");
    assert_eq!(finding.status, Status::NeedsRoot);
    assert!(finding.hint.contains("GRANT pg_monitor"));
    assert!(report.needs_root.contains(&"slots".to_string()));
}

#[test]
fn small_inactive_slot_is_ok_and_no_slots_is_ok() {
    let report = run("pg17-slot-small");
    let finding = slot(&report, "quiet");
    assert_eq!(finding.status, Status::Ok);
    assert_eq!(finding.bytes, Some(256));
    let none = run("pg17-slot-none");
    let finding = only(&none, "slots");
    assert_eq!(finding.status, Status::Ok);
    assert_eq!(finding.count, Some(0));
}
