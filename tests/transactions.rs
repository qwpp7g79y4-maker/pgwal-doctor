//! Online `transactions` and `prepared_xacts` checks.

mod common;

use common::{all, fixture, only};
use pgwal_doctor::{scan, FixtureSource, Report, Status};

fn run(scenario: &str) -> Report {
    let source = FixtureSource::new(fixture("pg").join(scenario));
    scan(None, Some(&source))
}

#[test]
fn old_prepared_transaction_is_reported_with_age() {
    let report = run("pg17");
    let finding = only(&report, "prepared_xacts");
    assert_eq!(finding.status, Status::Warn);
    assert_eq!(finding.count, Some(1));
    assert!(finding.evidence.contains("gid migration-42"));
    assert!(finding.evidence.contains("172800 s (2 d 0 h)"));
    // Wording: hinders VACUUM, does not claim to hold WAL.
    assert!(finding.hint.contains("hinder VACUUM"));
    assert!(finding.hint.contains("assumption"));
    assert!(!finding.hint.contains("holds WAL"));
}

#[test]
fn old_idle_in_transaction_session_is_reported_with_age() {
    let report = run("pg17");
    let finding = only(&report, "transactions");
    assert_eq!(finding.status, Status::Warn);
    assert_eq!(finding.count, Some(2));
    assert!(finding.evidence.contains("pid 6002 (idle in transaction"));
    assert!(finding.evidence.contains("7200 s (2 h 0 min)"));
    assert!(finding.evidence.contains("backend_xmin 1500"));
}

#[test]
fn no_transactions_is_ok() {
    let report = run("pg17-no-transactions");
    let finding = only(&report, "transactions");
    assert_eq!(finding.status, Status::Ok);
    assert_eq!(finding.count, Some(0));
    // The fixture has max_prepared_transactions=0.
    assert_eq!(only(&report, "prepared_xacts").status, Status::NotPresent);
}

#[test]
fn prepared_with_max_prepared_transactions_zero_is_not_present() {
    let report = run("pg17-hidden-sessions");
    let finding = only(&report, "prepared_xacts");
    assert_eq!(finding.status, Status::NotPresent);
    assert!(finding.evidence.contains("max_prepared_transactions=0"));
}

#[test]
fn other_users_sessions_hidden_is_needs_root() {
    let report = run("pg17-hidden-sessions");
    let findings = all(&report, "transactions");
    let hidden = findings
        .iter()
        .find(|f| f.status == Status::NeedsRoot)
        .expect("hidden finding");
    assert_eq!(hidden.count, Some(2));
    assert!(hidden.hint.contains("pg_monitor"));
    assert!(report.needs_root.contains(&"transactions".to_string()));
    assert_eq!(report.exit_code(), 1);
}

#[test]
fn permission_denied_is_needs_root() {
    let report = run("pg17/denied");
    assert_eq!(only(&report, "transactions").status, Status::NeedsRoot);
    assert_eq!(only(&report, "prepared_xacts").status, Status::NeedsRoot);
}
