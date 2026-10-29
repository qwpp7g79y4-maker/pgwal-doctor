//! Online `settings` check.

mod common;

use common::{fixture, only};
use pgwal_doctor::{scan, FixtureSource, Report, Status};

fn run(scenario: &str) -> Report {
    let source = FixtureSource::new(fixture("pg").join(scenario));
    scan(None, Some(&source))
}

#[test]
fn pg12_reads_wal_keep_segments() {
    let report = run("pg12");
    let finding = only(&report, "settings");
    assert_eq!(finding.status, Status::Ok);
    // 64 segments of 16 MiB.
    assert!(finding
        .evidence
        .contains("wal_keep_segments=64, wal_keep_bytes=1073741824 at 16777216-byte segments"));
    assert!(finding
        .evidence
        .contains("max_slot_wal_keep_size not available on this server version"));
    assert!(!finding.evidence.contains("wal_keep_size="));
}

#[test]
fn pg13_plus_reads_wal_keep_size() {
    for (scenario, keep) in [
        ("pg13", 2048u64 << 20),
        ("pg15", 0),
        ("pg17", 1024u64 << 20),
    ] {
        let report = run(scenario);
        let finding = only(&report, "settings");
        assert!(
            finding.evidence.contains(&format!("wal_keep_bytes={keep}")),
            "{scenario}: {}",
            finding.evidence
        );
        assert!(!finding.evidence.contains("wal_keep_segments"));
        assert!(finding.evidence.contains("max_wal_size=1073741824 bytes"));
        assert!(finding.evidence.contains("min_wal_size=83886080 bytes"));
        assert!(finding.evidence.contains("checkpoint_timeout=300 s"));
    }
    let finite = only(&run("pg13"), "settings").evidence.clone();
    assert!(finite.contains("max_slot_wal_keep_size=4294967296 bytes"));
    let unlimited = only(&run("pg15"), "settings").evidence.clone();
    assert!(unlimited.contains("max_slot_wal_keep_size=unlimited"));
}
