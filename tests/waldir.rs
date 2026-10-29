//! Online `waldir` check, the `summary` ranking and the JSON schema.

mod common;

use std::fs;

use common::{fixture, only, TempDir};
use pgwal_doctor::{scan, FixtureSource, Report, Root, Status};

fn run(scenario: &str) -> Report {
    let source = FixtureSource::new(fixture("pg").join(scenario));
    scan(None, Some(&source))
}

#[test]
fn without_pg_monitor_is_needs_root_with_grant_hint() {
    let report = run("pg17/denied");
    let finding = only(&report, "waldir");
    assert_eq!(finding.status, Status::NeedsRoot);
    assert!(finding.hint.contains("GRANT pg_monitor"));
    assert!(report.needs_root.contains(&"waldir".to_string()));
    assert_eq!(report.exit_code(), 1);
    // No directory total, so no summary, and the other checks still ran.
    assert!(report.findings.iter().all(|f| f.check != "summary"));
    assert_eq!(only(&report, "settings").status, Status::Ok);
}

#[test]
fn sum_matches_fixture_sizes() {
    let report = run("pg17");
    let finding = only(&report, "waldir");
    assert_eq!(finding.status, Status::Ok);
    // 3 segments of 16 MiB plus a 42-byte history file.
    assert_eq!(finding.bytes, Some(3 * 16_777_216 + 42));
    assert_eq!(finding.count, Some(3));
    assert!(finding
        .evidence
        .contains("3 segment file(s) = 50331648 bytes"));
    assert!(finding.evidence.contains("1 other file(s)"));
}

#[test]
fn history_and_partial_files_are_not_counted_as_segments() {
    let report = run("pg17-waldir-odd");
    let finding = only(&report, "waldir");
    assert_eq!(finding.count, Some(2));
    assert_eq!(finding.bytes, Some(2 * 16_777_216 + 100 + 4096));
}

#[test]
fn summary_ranks_causes_by_bytes_and_shows_other_unknown() {
    let report = run("pg13");
    let finding = only(&report, "summary");
    let total = finding.bytes.expect("total");
    let text = &finding.evidence;
    let slot = text.find("slot-retained WAL").expect("slot");
    let keep = text.find("wal_keep_size:").expect("keep");
    assert!(text.contains(&format!("pg_wal holds {total} bytes")));
    assert!(text.contains("other or unknown:"));
    // The slot lag (two 4 GiB steps) is far above wal_keep_size.
    assert!(slot < keep, "{text}");
    assert!(finding.hint.contains("assumptions"));
}

#[test]
fn summary_says_when_causes_overlap_past_the_directory_total() {
    let report = run("pg17");
    let finding = only(&report, "summary");
    assert!(finding.evidence.contains("other or unknown: 0 bytes"));
    assert!(finding.evidence.contains("The causes overlap"));
}

#[test]
fn summary_is_deterministic() {
    let first = run("pg17");
    let second = run("pg17");
    assert_eq!(first.render_json(), second.render_json());
    assert_eq!(first.render_text(), second.render_text());
}

#[test]
fn offline_total_feeds_the_summary_and_archive_backlog_is_ranked() {
    let tmp = TempDir::new("summary-offline");
    let status = tmp.path().join("pg_wal/archive_status");
    fs::create_dir_all(&status).expect("mkdir");
    for i in 1..=3u32 {
        let name = format!("00000001000000000000000{i}");
        fs::write(tmp.path().join("pg_wal").join(&name), vec![0u8; 1000]).expect("segment");
        if i < 3 {
            fs::write(status.join(format!("{name}.ready")), []).expect("ready");
        }
    }
    let report = scan(Some(&Root::new(tmp.path())), None);
    let finding = only(&report, "summary");
    assert_eq!(finding.bytes, Some(3000));
    assert!(finding
        .evidence
        .contains("archive backlog (.ready): 2000 bytes"));
    assert!(finding.evidence.contains("other or unknown: 1000 bytes"));
    assert!(finding.evidence.contains("Not measured: slot-retained WAL"));
}

#[test]
fn json_schema_is_pinned() {
    let report = run("pg17");
    let value: serde_json::Value = serde_json::from_str(&report.render_json()).expect("json");
    let top = value.as_object().expect("object");
    let mut keys: Vec<_> = top.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["findings", "needs_root"]);
    for finding in value["findings"].as_array().expect("findings array") {
        let object = finding.as_object().expect("finding object");
        let mut keys: Vec<_> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["bytes", "check", "count", "evidence", "hint", "status"]
        );
        assert!(object["check"].is_string());
        assert!(object["status"].is_string());
        assert!(object["bytes"].is_null() || object["bytes"].is_u64());
        assert!(object["count"].is_null() || object["count"].is_u64());
    }
}
