//! Offline `wal_dir` check against fixture data directories.
//!
//! Unix only: the tests use symlinks, file modes and `sh` stand-ins.

#![cfg(unix)]

mod common;

use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{symlink, PermissionsExt};

use common::{fixture, only, TempDir};
use pgwal_doctor::{scan, Root, Status};

fn wal_segment(dir: &std::path::Path, name: &str, len: usize) {
    fs::write(dir.join(name), vec![0u8; len]).expect("write segment");
}

#[test]
fn normal_fixture_counts_segments_and_bytes() {
    let root = Root::new(fixture("datadir/normal"));
    let report = scan(Some(&root), None);
    let finding = only(&report, "wal_dir");
    assert_eq!(finding.status, Status::Ok);
    assert_eq!(finding.count, Some(3));
    // 1000 + 2000 + 3000 segment bytes plus the 8-byte history file.
    assert_eq!(finding.bytes, Some(6008));
    assert!(finding.evidence.contains("3 segments"));
    assert!(finding.evidence.contains("1 other files"));
}

#[test]
fn empty_pg_wal_is_ok_with_zero_bytes() {
    let tmp = TempDir::new("empty");
    fs::create_dir(tmp.path().join("pg_wal")).expect("mkdir");
    let report = scan(Some(&Root::new(tmp.path())), None);
    let finding = only(&report, "wal_dir");
    assert_eq!(finding.status, Status::Ok);
    assert_eq!(finding.bytes, Some(0));
    assert_eq!(finding.count, Some(0));
    assert!(finding.hint.contains("unusual"));
}

#[test]
fn symlinked_pg_wal_is_followed_and_reported() {
    let tmp = TempDir::new("symlinked");
    let real = tmp.path().join("wal-real");
    fs::create_dir(&real).expect("mkdir");
    wal_segment(&real, "000000010000000000000001", 4096);
    symlink("wal-real", tmp.path().join("pg_wal")).expect("symlink");
    let report = scan(Some(&Root::new(tmp.path())), None);
    let finding = only(&report, "wal_dir");
    assert_eq!(finding.status, Status::Ok);
    assert_eq!(finding.bytes, Some(4096));
    assert_eq!(finding.count, Some(1));
    assert!(finding.evidence.contains("pg_wal is a symlink to wal-real"));
}

#[test]
fn symlink_does_not_escape_root_prefix() {
    let tmp = TempDir::new("escape");
    let outside = tmp.path().join("outside");
    fs::create_dir(&outside).expect("mkdir");
    wal_segment(&outside, "000000010000000000000001", 4096);
    let datadir = tmp.path().join("datadir");
    fs::create_dir(&datadir).expect("mkdir");
    symlink("../outside", datadir.join("pg_wal")).expect("symlink");
    let report = scan(Some(&Root::new(&datadir)), None);
    let finding = only(&report, "wal_dir");
    assert_eq!(finding.status, Status::Error);
    assert_eq!(finding.bytes, None);
    assert!(finding.evidence.contains("leaves the data directory"));
}

#[test]
fn unreadable_pg_wal_is_needs_root() {
    let tmp = TempDir::new("unreadable");
    let wal = tmp.path().join("pg_wal");
    fs::create_dir(&wal).expect("mkdir");
    wal_segment(&wal, "000000010000000000000001", 10);
    fs::set_permissions(&wal, fs::Permissions::from_mode(0o000)).expect("chmod");
    if fs::read_dir(&wal).is_ok() {
        // Running as root: chmod 000 does not deny anything.
        return;
    }
    let report = scan(Some(&Root::new(tmp.path())), None);
    let finding = only(&report, "wal_dir");
    assert_eq!(finding.status, Status::NeedsRoot);
    assert!(report.needs_root.contains(&"wal_dir".to_string()));
    assert_eq!(report.exit_code(), 1);
}

#[test]
fn non_utf8_file_name_does_not_panic() {
    let tmp = TempDir::new("nonutf8");
    let wal = tmp.path().join("pg_wal");
    fs::create_dir(&wal).expect("mkdir");
    wal_segment(&wal, "000000010000000000000001", 100);
    let odd = OsString::from_vec(b"000000010000000000\xff\xfe0002".to_vec());
    fs::write(wal.join(odd), vec![0u8; 50]).expect("write odd name");
    let report = scan(Some(&Root::new(tmp.path())), None);
    let finding = only(&report, "wal_dir");
    assert_eq!(finding.status, Status::Ok);
    assert_eq!(finding.count, Some(1));
    assert_eq!(finding.bytes, Some(150));
    let _ = report.render_text();
    let _ = report.render_json();
}

#[test]
fn missing_pg_wal_is_a_datadir_error_not_a_quiet_report() {
    let tmp = TempDir::new("missing");
    let report = scan(Some(&Root::new(tmp.path())), None);
    assert_eq!(only(&report, "datadir").status, Status::Error);
    assert_eq!(report.exit_code(), 1);
}
