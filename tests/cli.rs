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

fn text(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn datadir_fixture_renders_text_and_json() {
    let dir = fixture("normal");
    let path = dir.to_str().expect("utf-8 path");
    let report = pgwal_doctor::scan(Some(&pgwal_doctor::Root::new(&dir)), None);
    let out = run(&["--datadir", path]);
    assert_eq!(text(&out), report.render_text());
    let out = run(&["--json", "--datadir", path]);
    assert_eq!(text(&out), report.render_json());
}

#[test]
fn json_output_is_valid_and_has_findings_array() {
    let dir = fixture("normal");
    let out = run(&["--json", "--datadir", dir.to_str().expect("utf-8 path")]);
    let value: serde_json::Value = serde_json::from_str(&text(&out)).expect("valid json");
    assert!(value["findings"].as_array().is_some_and(|a| !a.is_empty()));
    assert!(value["needs_root"].is_array());
}

#[test]
fn no_source_given_exits_2() {
    let out = run(&[]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("give --datadir, --dsn or --online"),
        "{stderr}"
    );
    assert!(!stderr.contains("panicked"), "{stderr}");
}

/// A `psql` stand-in that answers from `tests/fixtures/pg/<scenario>`.
fn fake_psql(tag: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("pgwal-cli-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let script = dir.join("psql");
    std::fs::write(&script, body).expect("write");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    script
}

const SERVE_FIXTURES: &str = r#"#!/bin/sh
for a; do sql="$a"; done
case "$sql" in
  *pg_replication_slots*) f=pg_replication_slots ;;
  *pg_stat_archiver*) f=pg_stat_archiver ;;
  *pg_stat_activity*) f=pg_stat_activity ;;
  *pg_prepared_xacts*) f=pg_prepared_xacts ;;
  *pg_settings*) f=pg_settings ;;
  *pg_ls_waldir*) f=pg_ls_waldir ;;
  *) echo "unexpected query: $sql" >&2; exit 9 ;;
esac
cat "$FAKE_PSQL_DIR/$f.tsv"
"#;

fn run_online(psql: &PathBuf, args: &[&str], fixture_dir: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pgwal-doctor"))
        .args(args)
        .env("PGWAL_DOCTOR_PSQL", psql)
        .env(
            "FAKE_PSQL_DIR",
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/pg")
                .join(fixture_dir),
        )
        .output()
        .expect("run pgwal-doctor")
}

#[test]
fn actionable_finding_exits_1() {
    let psql = fake_psql("actionable", SERVE_FIXTURES);
    let out = run_online(&psql, &["--online"], "pg17");
    assert_eq!(out.status.code(), Some(1), "stdout: {}", text(&out));
    assert!(text(&out).contains("[Warn] slots"));
    assert!(text(&out).contains("[Ok] summary"));
}

#[test]
fn dsn_error_message_never_contains_password() {
    // A DSN with a password is refused before psql runs.
    let psql = fake_psql("password", SERVE_FIXTURES);
    let out = run_online(&psql, &["--dsn", "host=db user=u password=hunter2"], "pg17");
    assert_eq!(out.status.code(), Some(1));
    let all = format!("{}{}", text(&out), String::from_utf8_lossy(&out.stderr));
    assert!(!all.contains("hunter2"), "{all}");
    assert!(all.contains("[Error] online"));

    // A connection failure that echoes the DSN is scrubbed.
    let echo = fake_psql(
        "echo",
        "#!/bin/sh\necho \"psql: error: could not connect with host=secret.example user=u\" >&2\nexit 2\n",
    );
    let out = run_online(&echo, &["--dsn", "host=secret.example user=u"], "pg17");
    assert_eq!(out.status.code(), Some(1));
    assert!(!text(&out).contains("secret.example"), "{}", text(&out));
    assert!(text(&out).contains("<dsn>"));
}
