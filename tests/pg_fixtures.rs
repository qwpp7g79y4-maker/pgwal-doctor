//! Every fixture file parses, and the four version sets are all there.

mod common;

use std::fs;
use std::path::Path;

use common::fixture;
use pgwal_doctor::{Cell, FixtureSource, PgSource, Rows, SourceError};

fn tsv_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).expect("read_dir").flatten() {
        let path = entry.path();
        if path.is_dir() {
            tsv_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "tsv") {
            out.push(path);
        }
    }
}

#[test]
fn every_fixture_file_parses() {
    let mut files = Vec::new();
    tsv_files(&fixture("pg"), &mut files);
    assert!(files.len() >= 24, "found {} files", files.len());
    for path in files {
        let text = fs::read_to_string(&path).expect("read");
        Rows::parse_tsv(&text).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    }
}

#[test]
fn each_version_set_has_all_six_views() {
    for version in ["pg12", "pg13", "pg15", "pg17"] {
        let source = FixtureSource::new(fixture("pg").join(version));
        assert!(source.slots().is_ok(), "{version} slots");
        assert!(source.archiver().is_ok(), "{version} archiver");
        assert!(source.activity().is_ok(), "{version} activity");
        assert!(source.prepared().is_ok(), "{version} prepared");
        assert!(source.settings().is_ok(), "{version} settings");
        assert!(source.waldir().is_ok(), "{version} waldir");
    }
}

#[test]
fn version_column_sets_differ_as_documented() {
    let slots = |v: &str| {
        FixtureSource::new(fixture("pg").join(v))
            .slots()
            .expect("slots")
    };
    assert!(!slots("pg12").has_column("wal_status"));
    assert!(slots("pg13").has_column("wal_status"));
    assert!(!slots("pg13").has_column("inactive_since"));
    assert!(slots("pg17").has_column("inactive_since"));
    assert!(slots("pg17").has_column("invalidation_reason"));
}

#[test]
fn null_is_not_an_empty_string_in_fixtures() {
    let settings = FixtureSource::new(fixture("pg/pg13"))
        .settings()
        .expect("settings");
    let cell = |row: usize, col: &str| settings.get(row, col);
    // archive_command is '' (empty string); its unit is NULL.
    let index = (0..settings.len())
        .find(|&i| cell(i, "name") == Cell::Value("archive_command"))
        .expect("archive_command row");
    assert_eq!(cell(index, "setting"), Cell::Value("cp %p /arch/%f"));
    assert_eq!(cell(index, "unit"), Cell::Null);
    let denied = FixtureSource::new(fixture("pg/pg17/denied"));
    assert_eq!(denied.waldir(), Err(SourceError::PermissionDenied));
}
