//! The binary renders a fixture data directory in text and JSON.

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/datadir")
        .join(name)
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pgwal-doctor"))
        .args(args)
        .output()
        .expect("run pgwal-doctor")
}

#[test]
fn unknown_flag_exits_2() {
    let out = run(&["--no-such-flag"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn clean_datadir_exits_0() {
    let dir = fixture("normal");
    let out = run(&["--datadir", dir.to_str().expect("utf-8 path")]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}
