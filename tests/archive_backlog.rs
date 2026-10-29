//! Offline `archive_backlog` check.

mod common;

use std::fs;
use std::time::Instant;

use common::{all, fixture, only, TempDir};
use pgwal_doctor::{scan, Root, Status};

#[test]
fn normal_fixture_has_a_small_backlog() {
    let report = scan(Some(&Root::new(fixture("datadir/normal"))), None);
    let finding = only(&report, "archive_backlog");
    assert_eq!(finding.status, Status::Ok);
    assert_eq!(finding.count, Some(1));
    assert_eq!(finding.bytes, Some(3000));
    assert!(finding.evidence.contains("1 .ready files"));
    assert!(finding.evidence.contains("2 .done files"));
}

#[test]
fn thousands_of_ready_files_are_counted_and_evidence_is_truncated() {
    let tmp = TempDir::new("ready5000");
    let status = tmp.path().join("pg_wal/archive_status");
    fs::create_dir_all(&status).expect("mkdir");
    for i in 0..5000u32 {
        // Segments exist for every .ready file so none is an orphan.
        let name = format!("00000001{:08X}{:08X}", 0, i);
        fs::write(tmp.path().join("pg_wal").join(&name), [0u8; 10]).expect("segment");
        fs::write(status.join(format!("{name}.ready")), []).expect("ready");
    }
    let root = Root::new(tmp.path());
    let started = Instant::now();
    let report = scan(Some(&root), None);
    let elapsed = started.elapsed();
    assert!(elapsed.as_secs_f64() < 1.0, "scan took {elapsed:?}");
    let finding = only(&report, "archive_backlog");
    assert_eq!(finding.status, Status::Warn);
    assert_eq!(finding.count, Some(5000));
    assert_eq!(finding.bytes, Some(50_000));
    assert!(finding.evidence.contains("and 4995 more"));
    assert!(
        finding.evidence.len() < 400,
        "evidence: {}",
        finding.evidence
    );
}

#[test]
fn missing_archive_status_is_not_present() {
    let tmp = TempDir::new("noarchive");
    fs::create_dir(tmp.path().join("pg_wal")).expect("mkdir");
    let report = scan(Some(&Root::new(tmp.path())), None);
    let finding = only(&report, "archive_backlog");
    assert_eq!(finding.status, Status::NotPresent);
}

#[test]
fn ready_without_matching_segment_is_reported() {
    let tmp = TempDir::new("orphan");
    let status = tmp.path().join("pg_wal/archive_status");
    fs::create_dir_all(&status).expect("mkdir");
    fs::write(status.join("000000010000000000000009.ready"), []).expect("ready");
    let report = scan(Some(&Root::new(tmp.path())), None);
    let findings = all(&report, "archive_backlog");
    assert_eq!(findings.len(), 2);
    let orphan = findings
        .iter()
        .find(|f| f.evidence.contains("no matching segment"))
        .expect("orphan finding");
    assert_eq!(orphan.status, Status::Warn);
    assert_eq!(orphan.count, Some(1));
    assert!(orphan.evidence.contains("000000010000000000000009.ready"));
}
