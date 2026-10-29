//! Online `archiver` check.

mod common;

use std::fs;

use common::{fixture, only, TempDir};
use pgwal_doctor::{scan, FixtureSource, Report, Root, Status};

fn run(scenario: &str) -> Report {
    let source = FixtureSource::new(fixture("pg").join(scenario));
    scan(None, Some(&source))
}

#[test]
fn failing_command_warns_with_last_failed_wal() {
    let report = run("pg17-archiver-failing");
    let finding = only(&report, "archiver");
    assert_eq!(finding.status, Status::Warn);
    assert_eq!(finding.count, Some(37));
    assert!(finding
        .evidence
        .contains("last failed WAL 000000010000000000000065"));
    assert!(finding.hint.contains("/bin/true"));
    assert!(finding.hint.contains("does not prove"));
}

#[test]
fn healthy_archiver_is_ok() {
    let report = run("pg17");
    let finding = only(&report, "archiver");
    assert_eq!(finding.status, Status::Ok);
    assert!(finding.evidence.contains("archived_count=5000"));
    // Even a healthy result carries the caveat about clean statistics.
    assert!(finding.hint.contains("does not prove"));
}

#[test]
fn archive_mode_off_is_not_present() {
    let report = run("pg17-archiver-off");
    let finding = only(&report, "archiver");
    assert_eq!(finding.status, Status::NotPresent);
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn clean_stats_with_ready_backlog_still_warns() {
    let tmp = TempDir::new("archiver-backlog");
    let status = tmp.path().join("pg_wal/archive_status");
    fs::create_dir_all(&status).expect("mkdir");
    for i in 0..150u32 {
        let name = format!("000000010000000000{i:06X}");
        fs::write(tmp.path().join("pg_wal").join(&name), [0u8; 8]).expect("segment");
        fs::write(status.join(format!("{name}.ready")), []).expect("ready");
    }
    let source = FixtureSource::new(fixture("pg/pg17"));
    let report = scan(Some(&Root::new(tmp.path())), Some(&source));
    let finding = only(&report, "archiver");
    assert_eq!(finding.status, Status::Warn);
    assert!(finding
        .evidence
        .contains("no recent failure but 150 .ready files wait"));
}

#[test]
fn never_archived_is_ok_without_backlog_and_warns_with_one() {
    let report = run("pg17-archiver-never");
    let finding = only(&report, "archiver");
    assert_eq!(finding.status, Status::Ok);
    assert!(finding.evidence.contains("nothing archived yet"));

    let tmp = TempDir::new("archiver-never");
    let status = tmp.path().join("pg_wal/archive_status");
    fs::create_dir_all(&status).expect("mkdir");
    fs::write(status.join("000000010000000000000001.ready"), []).expect("ready");
    fs::write(tmp.path().join("pg_wal/000000010000000000000001"), [0u8; 8]).expect("segment");
    let source = FixtureSource::new(fixture("pg/pg17-archiver-never"));
    let report = scan(Some(&Root::new(tmp.path())), Some(&source));
    let finding = only(&report, "archiver");
    assert_eq!(finding.status, Status::Warn);
    assert!(finding
        .evidence
        .contains("nothing was ever archived, yet 1 .ready"));
}

#[test]
fn archive_mode_on_without_command_or_library_warns() {
    let report = run("pg17-archiver-nocmd");
    let finding = only(&report, "archiver");
    assert_eq!(finding.status, Status::Warn);
    assert!(finding
        .evidence
        .contains("neither archive_command nor archive_library"));
}

#[test]
fn pg12_without_archive_library_is_handled() {
    let report = run("pg12");
    let finding = only(&report, "archiver");
    assert_eq!(finding.status, Status::Ok);
    assert!(finding
        .evidence
        .contains("archive_library not available on this server version"));
}

#[test]
fn permission_denied_is_needs_root() {
    let report = run("pg17/denied");
    let finding = only(&report, "archiver");
    assert_eq!(finding.status, Status::NeedsRoot);
    assert!(finding.hint.contains("pg_monitor"));
}
