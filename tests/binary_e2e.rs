//! End-to-end runs of the built `pgwal-doctor` binary.
//!
//! Online cases talk to a `psql` stand-in under a temp directory. They never
//! open a live server, never set `PGPASSWORD`, and never read `~/.pgpass`.
//!
//! Unix only: the tests use symlinks, file modes and `sh` stand-ins.

#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;
use std::process::{Command, Output};
use std::time::UNIX_EPOCH;

use common::TempDir;

const SEGMENT: &str = "000000010000000000000001";

const SERVE_FIXTURES: &str = r#"#!/bin/sh
printf '%s\n' ran >> "$FAKE_PSQL_DIR/invocations"
if [ -n "$PGOPTIONS" ]; then
  printf '%s\n' "$PGOPTIONS" > "$FAKE_PSQL_DIR/pgoptions"
fi
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

fn doctor() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pgwal-doctor"))
}

fn text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn err_text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn assert_no_panic(out: &Output) {
    let stderr = err_text(out);
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert!(!text(out).contains("panicked"), "{}", text(out));
}

fn write_exe(path: &Path, body: &str) {
    fs::write(path, body).expect("write script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
}

/// A temp data directory with one segment, one history file, archive status, and one slot dir.
fn rebuilt_datadir(root: &Path) {
    fs::write(root.join("PG_VERSION"), "16\n").expect("PG_VERSION");
    let wal = root.join("pg_wal");
    fs::create_dir(&wal).expect("pg_wal");
    fs::write(wal.join(SEGMENT), vec![0u8; 100]).expect("segment");
    fs::write(wal.join(format!("{SEGMENT}.history")), vec![0u8; 8]).expect("history");
    let status = wal.join("archive_status");
    fs::create_dir(&status).expect("archive_status");
    fs::write(status.join(format!("{SEGMENT}.done")), "").expect("done");
    // Not a segment name, so the backlog counts the file and does not stat a segment.
    fs::write(status.join("notes.ready"), "").expect("ready");
    fs::create_dir_all(root.join("pg_replslot").join("standby1")).expect("slot");
}

struct Online {
    tmp: TempDir,
}

impl Online {
    fn new(tag: &str) -> Self {
        let tmp = TempDir::new(tag);
        write_exe(&tmp.path().join("psql"), SERVE_FIXTURES);
        let online = Self { tmp };
        online.set(
            "pg_replication_slots.tsv",
            "slot_name\tslot_type\tactive\trestart_lsn\tcurrent_lsn\twal_status\n",
        );
        online.set(
            "pg_stat_archiver.tsv",
            "archived_count\tlast_archived_wal\tlast_archived_time\tfailed_count\tlast_failed_wal\tlast_failed_time\n\
             1\t000000010000000000000001\t2026-01-01 00:00:00+00\t0\t\\N\t\\N\n",
        );
        online.set(
            "pg_stat_activity.tsv",
            "pid\tusename\tstate\txact_start\tbackend_xmin\txact_age_s\n",
        );
        online.set(
            "pg_prepared_xacts.tsv",
            "gid\tprepared\towner\tdatabase\tage_s\n",
        );
        online.set(
            "pg_settings.tsv",
            "name\tsetting\tunit\tsource\n\
             max_wal_size\t1\tMB\tdefault\n\
             min_wal_size\t1\tMB\tdefault\n",
        );
        online.set(
            "pg_ls_waldir.tsv",
            "name\tsize\tmodification\n000000010000000000000001\t16777216\t2026-01-01\n",
        );
        online
    }

    fn set(&self, name: &str, body: &str) {
        fs::write(self.tmp.path().join(name), body).unwrap_or_else(|_| panic!("write {name}"));
    }

    fn run(&self, args: &[&str]) -> Output {
        self.run_with(args, &[])
    }

    fn run_with(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        let mut command = doctor();
        command
            .args(args)
            .env("PGWAL_DOCTOR_PSQL", self.tmp.path().join("psql"))
            .env("FAKE_PSQL_DIR", self.tmp.path())
            .env_remove("PGPASSWORD")
            .env_remove("PGPASSFILE");
        for (key, value) in env {
            command.env(key, value);
        }
        command.output().expect("run pgwal-doctor")
    }
}

#[test]
fn help_exits_0() {
    let out = doctor().arg("--help").output().expect("run --help");
    assert_eq!(out.status.code(), Some(0));
    assert!(text(&out).contains("Usage: pgwal-doctor"));
    assert_no_panic(&out);
}

#[test]
fn no_datadir_dsn_or_online_exits_2_without_panic() {
    for args in [&[][..], &["--json"][..]] {
        let out = doctor().args(args).output().expect("run");
        assert_eq!(out.status.code(), Some(2), "{args:?} stdout {}", text(&out));
        let stderr = err_text(&out);
        assert!(
            stderr.contains("give --datadir, --dsn or --online"),
            "{stderr}"
        );
        assert_no_panic(&out);
    }
}

#[test]
fn rebuilt_datadir_renders_text_and_json() {
    let tmp = TempDir::new("rebuilt");
    let datadir = tmp.path().join("datadir");
    fs::create_dir(&datadir).expect("mkdir");
    rebuilt_datadir(&datadir);
    let path = datadir.to_str().expect("utf-8");

    let text_out = doctor().args(["--datadir", path]).output().expect("text");
    assert_eq!(text_out.status.code(), Some(0), "{}", text(&text_out));
    assert_no_panic(&text_out);
    let report = text(&text_out);
    assert!(report.contains("1 segments, 108 bytes"), "{report}");
    assert!(report.contains("notes.ready"), "{report}");
    assert!(report.contains("[Ok] pg_replslot"), "{report}");
    assert!(report.contains("count=1"), "{report}");
    assert!(err_text(&text_out).is_empty(), "{}", err_text(&text_out));

    let json_out = doctor()
        .args(["--json", "--datadir", path])
        .output()
        .expect("json");
    assert_eq!(json_out.status.code(), Some(0), "{}", text(&json_out));
    assert_no_panic(&json_out);
    let value: serde_json::Value = serde_json::from_slice(&json_out.stdout).expect("json");
    let findings = value["findings"].as_array().expect("findings");
    let wal = findings
        .iter()
        .find(|finding| finding["check"] == "wal_dir")
        .expect("wal_dir");
    assert_eq!(wal["status"], "Ok");
    assert_eq!(wal["count"], 1);
    assert_eq!(wal["bytes"], 108);
    assert!(value["needs_root"]
        .as_array()
        .expect("needs_root")
        .is_empty());
}

#[test]
fn datadir_that_is_a_file_is_an_error_without_panic() {
    let tmp = TempDir::new("not-a-dir");
    let path = tmp.path().join("not-a-datadir");
    fs::write(&path, b"not a directory").expect("write");
    let out = doctor()
        .args(["--datadir", path.to_str().expect("utf-8")])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert_no_panic(&out);
    let report = text(&out);
    assert!(report.contains("[Error] wal_dir"), "{report}");
    assert!(report.contains("cannot read"), "{report}");
    assert!(report.contains("archive_status"), "{report}");
    assert!(report.contains("pg_replslot"), "{report}");
}

#[test]
fn unreadable_datadir_needs_root() {
    let tmp = TempDir::new("unreadable-root");
    let datadir = tmp.path().join("datadir");
    fs::create_dir(&datadir).expect("mkdir");
    fs::create_dir(datadir.join("pg_wal")).expect("pg_wal");
    fs::set_permissions(&datadir, fs::Permissions::from_mode(0o000)).expect("chmod");
    if fs::symlink_metadata(datadir.join("pg_wal")).is_ok() {
        // Running as root: mode 000 does not deny the lookup.
        return;
    }
    let out = doctor()
        .args(["--datadir", datadir.to_str().expect("utf-8")])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert_no_panic(&out);
    assert!(text(&out).contains("[NeedsRoot] wal_dir"), "{}", text(&out));
}

#[test]
fn pg_wal_as_a_file_is_an_error() {
    let tmp = TempDir::new("wal-file");
    let datadir = tmp.path().join("datadir");
    fs::create_dir(&datadir).expect("mkdir");
    fs::write(datadir.join("pg_wal"), b"not a directory").expect("write");
    let out = doctor()
        .args(["--datadir", datadir.to_str().expect("utf-8")])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert_no_panic(&out);
    let report = text(&out);
    assert!(report.contains("[Error] wal_dir"), "{report}");
    assert!(report.contains("pg_wal"), "{report}");
}

#[test]
fn dangling_pg_wal_symlink_is_not_present() {
    let tmp = TempDir::new("dangling");
    let datadir = tmp.path().join("datadir");
    fs::create_dir(&datadir).expect("mkdir");
    symlink("missing-wal", datadir.join("pg_wal")).expect("symlink");
    let out = doctor()
        .args(["--datadir", datadir.to_str().expect("utf-8")])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert_no_panic(&out);
    assert!(text(&out).contains("dangling symlink"), "{}", text(&out));
}

#[test]
fn absolute_pg_wal_symlink_is_followed() {
    let tmp = TempDir::new("absolute-link");
    let store = tmp.path().join("wal-store");
    fs::create_dir(&store).expect("mkdir store");
    fs::write(store.join(SEGMENT), vec![0u8; 64]).expect("segment");
    let absolute = fs::canonicalize(&store).expect("canonicalize");
    let datadir = tmp.path().join("datadir");
    fs::create_dir(&datadir).expect("mkdir");
    symlink(&absolute, datadir.join("pg_wal")).expect("symlink");
    let out = doctor()
        .args(["--datadir", datadir.to_str().expect("utf-8")])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert_no_panic(&out);
    let report = text(&out);
    assert!(
        report.contains(&format!("pg_wal is a symlink to {}", absolute.display())),
        "{report}"
    );
    assert!(report.contains("1 segments, 64 bytes"), "{report}");
}

#[test]
fn archive_status_as_a_file_is_an_error() {
    let tmp = TempDir::new("status-file");
    let datadir = tmp.path().join("datadir");
    fs::create_dir(&datadir).expect("mkdir");
    let wal = datadir.join("pg_wal");
    fs::create_dir(&wal).expect("pg_wal");
    fs::write(wal.join(SEGMENT), vec![0u8; 16]).expect("segment");
    fs::write(wal.join("archive_status"), b"not a directory").expect("file");
    let out = doctor()
        .args(["--datadir", datadir.to_str().expect("utf-8")])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert_no_panic(&out);
    let report = text(&out);
    assert!(report.contains("[Error] archive_backlog"), "{report}");
    assert!(report.contains("archive_status"), "{report}");
}

#[test]
fn unreadable_pg_replslot_needs_root() {
    let tmp = TempDir::new("slot-mode");
    let datadir = tmp.path().join("datadir");
    fs::create_dir(&datadir).expect("mkdir");
    let wal = datadir.join("pg_wal");
    fs::create_dir(&wal).expect("pg_wal");
    fs::write(wal.join(SEGMENT), vec![0u8; 16]).expect("segment");
    let slots = datadir.join("pg_replslot");
    fs::create_dir(&slots).expect("slots");
    fs::set_permissions(&slots, fs::Permissions::from_mode(0o000)).expect("chmod");
    if fs::read_dir(&slots).is_ok() {
        return;
    }
    let out = doctor()
        .args(["--json", "--datadir", datadir.to_str().expect("utf-8")])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert_no_panic(&out);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert!(value["needs_root"]
        .as_array()
        .expect("needs_root")
        .iter()
        .any(|name| name == "pg_replslot"));
}

#[test]
fn segment_mtime_before_the_epoch_is_still_counted() {
    let tmp = TempDir::new("mtime");
    let datadir = tmp.path().join("datadir");
    fs::create_dir(&datadir).expect("mkdir");
    let wal = datadir.join("pg_wal");
    fs::create_dir(&wal).expect("pg_wal");
    let old = wal.join(SEGMENT);
    let recent = wal.join("000000010000000000000002");
    fs::write(&old, vec![0u8; 10]).expect("old");
    fs::write(&recent, vec![0u8; 20]).expect("recent");
    let touched = Command::new("touch")
        .args(["-d", "@-1"])
        .arg(&old)
        .status()
        .expect("touch");
    assert!(touched.success(), "touch could not set a pre-epoch mtime");
    let modified = fs::metadata(&old).expect("meta").modified().expect("mtime");
    assert!(
        modified.duration_since(UNIX_EPOCH).is_err(),
        "filesystem clamped the mtime to the epoch"
    );
    let out = doctor()
        .args(["--datadir", datadir.to_str().expect("utf-8")])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert_no_panic(&out);
    let report = text(&out);
    assert!(report.contains("2 segments, 30 bytes"), "{report}");
    assert!(report.contains("segment mtime unix"), "{report}");
}

#[test]
fn control_characters_in_the_datadir_path_are_escaped() {
    let tmp = TempDir::new("controls");
    let weird = tmp.path().join("w\\x\n\r\u{1}");
    fs::create_dir(&weird).expect("mkdir");
    let out = doctor()
        .args(["--datadir", weird.to_str().expect("utf-8")])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert_no_panic(&out);
    let report = text(&out);
    let evidence = report
        .lines()
        .find(|line| line.starts_with("evidence:"))
        .expect("evidence line");
    assert!(evidence.contains("w\\\\x\\n\\r\\u{1}"), "{evidence}");
    assert!(
        !evidence.contains('\r') && !evidence.contains('\u{1}'),
        "{evidence}"
    );
}

#[test]
fn dsn_with_a_password_is_refused_before_psql_starts() {
    let tmp = TempDir::new("dsn-secret");
    let marker = tmp.path().join("psql-was-started");
    let script = tmp.path().join("psql");
    write_exe(
        &script,
        &format!("#!/bin/sh\ntouch '{}'\nexit 0\n", marker.display()),
    );
    let secret = "not-a-real-secret";
    let dsns = [
        format!("host=127.0.0.1 port=9 user=nobody password={secret} dbname=postgres"),
        format!("postgresql://nobody:{secret}@127.0.0.1:9/postgres"),
    ];
    for dsn in dsns {
        let out = doctor()
            .args(["--dsn", &dsn])
            .env("PGWAL_DOCTOR_PSQL", &script)
            .env_remove("PGPASSWORD")
            .env_remove("PGPASSFILE")
            .output()
            .expect("run");
        assert_eq!(out.status.code(), Some(1), "{}", text(&out));
        assert_no_panic(&out);
        let all = format!("{}{}", text(&out), err_text(&out));
        assert!(!all.contains(secret), "{all}");
        assert!(all.contains("[Error] online"), "{all}");
        assert!(all.contains("DSN contains a password"), "{all}");
        assert!(
            !marker.exists(),
            "psql started for a DSN that carries a password"
        );
    }
}

#[test]
fn passwordfile_key_is_not_a_password_and_psql_stand_in_runs() {
    let online = Online::new("passwordfile");
    let dsn = "host=127.0.0.1 port=9 user=nobody passwordfile=/tmp/pgwal-doctor-no-such-pgpass dbname=postgres";
    let out = online.run(&["--dsn", dsn]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert_no_panic(&out);
    assert!(text(&out).contains("max_wal_size="), "{}", text(&out));
    assert!(
        online.tmp.path().join("invocations").is_file(),
        "stand-in psql did not run"
    );
    let all = format!("{}{}", text(&out), err_text(&out));
    assert!(!all.contains("pgpass"), "{all}");
}

#[test]
fn preset_pgoptions_are_kept_and_the_session_stays_read_only() {
    let online = Online::new("pgoptions");
    let out = online.run_with(&["--online"], &[("PGOPTIONS", "-c extra_float_digits=0")]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert_no_panic(&out);
    let recorded = fs::read_to_string(online.tmp.path().join("pgoptions")).expect("pgoptions");
    assert!(recorded.contains("extra_float_digits=0"), "{recorded}");
    assert!(
        recorded.contains("default_transaction_read_only=on"),
        "{recorded}"
    );
    assert!(recorded.contains("statement_timeout=15000"), "{recorded}");
}

#[test]
fn online_settings_edges_render_in_text_and_json() {
    let cases = [
        (
            "name\tsetting\tunit\tsource\n",
            "[NotPresent] settings",
            "none of the WAL settings",
        ),
        (
            "name\tsetting\tunit\tsource\nwal_keep_segments\t4\t\\N\tdefault\nmax_wal_size\t1\tMB\tdefault\n",
            "wal_keep_segments=4",
            "segment size assumed 16 MB",
        ),
        (
            "name\tsetting\tunit\tsource\nwal_keep_segments\tnope\t\\N\tdefault\nmax_wal_size\t1\tMB\tdefault\n",
            "max_wal_size=",
            "wal_keep_bytes",
        ),
        (
            "name\tsetting\tunit\tsource\nmax_slot_wal_keep_size\tnope\tMB\tdefault\nmax_wal_size\t1\tMB\tdefault\n",
            "max_wal_size=",
            "max_slot_wal_keep_size",
        ),
        (
            "name\tsetting\tunit\tsource\ncheckpoint_timeout\t2\th\tdefault\nmax_wal_size\t1\tMB\tdefault\n",
            "checkpoint_timeout=7200 s",
            "",
        ),
        (
            "name\tsetting\tunit\tsource\ncheckpoint_timeout\t1\td\tdefault\nmax_wal_size\t1\tMB\tdefault\n",
            "checkpoint_timeout=86400 s",
            "",
        ),
        (
            "name\tsetting\tunit\tsource\ncheckpoint_timeout\t1\tw\tdefault\nmax_wal_size\t1\tMB\tdefault\n",
            "max_wal_size=",
            "checkpoint_timeout",
        ),
    ];
    for (index, (settings, must, absent_or_extra)) in cases.iter().enumerate() {
        let online = Online::new(&format!("settings-{index}"));
        online.set("pg_settings.tsv", settings);
        let out = online.run(&["--online"]);
        assert_no_panic(&out);
        let report = text(&out);
        assert!(report.contains(must), "case {index}: {report}");
        if index == 2 || index == 3 || index == 6 {
            assert!(
                !report.contains(absent_or_extra),
                "case {index} should not contain {absent_or_extra}: {report}"
            );
        } else if !absent_or_extra.is_empty() {
            assert!(report.contains(absent_or_extra), "case {index}: {report}");
        }
    }

    let online = Online::new("settings-json");
    online.set(
        "pg_settings.tsv",
        "name\tsetting\tunit\tsource\ncheckpoint_timeout\t2\th\tdefault\n",
    );
    let out = online.run(&["--json", "--online"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let evidence = value["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .find(|finding| finding["check"] == "settings")
        .expect("settings")["evidence"]
        .as_str()
        .expect("evidence")
        .to_string();
    assert!(evidence.contains("checkpoint_timeout=7200 s"), "{evidence}");
}

#[test]
fn online_archiver_slots_waldir_and_transactions() {
    let online = Online::new("shapes");
    online.set(
        "pg_stat_archiver.tsv",
        "archived_count\tlast_archived_wal\tlast_archived_time\tfailed_count\tlast_failed_wal\tlast_failed_time\n",
    );
    online.set(
        "pg_settings.tsv",
        "name\tsetting\tunit\tsource\narchive_mode\ton\t\\N\tdefault\nmax_prepared_transactions\t10\t\\N\tdefault\n",
    );
    online.set(
        "pg_replication_slots.tsv",
        "slot_name\tslot_type\tactive\trestart_lsn\tcurrent_lsn\twal_status\n\
         nullstat\tphysical\tf\t0/100\t0/200\t\\N\n",
    );
    online.set(
        "pg_stat_activity.tsv",
        "pid\tusename\tstate\txact_start\tbackend_xmin\txact_age_s\n\
         9\tapp\tactive\t2026-01-01 00:00:00+00\t\\N\tnope\n",
    );
    online.set(
        "pg_prepared_xacts.tsv",
        "gid\tprepared\towner\tdatabase\tage_s\n\
         young\t2026-01-01 00:00:00+00\tapp\tappdb\t10\n",
    );
    online.set(
        "pg_ls_waldir.tsv",
        "name\tsize\tmodification\n\
         000000010000000000000001\tnope\t2026-01-01\n\
         000000010000000000000002\t100\t2026-01-01\n",
    );
    let out = online.run(&["--online"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert_no_panic(&out);
    let report = text(&out);
    assert!(
        report.contains("pg_stat_archiver returned no row"),
        "{report}"
    );
    assert!(report.contains("wal_status NULL"), "{report}");
    assert!(
        report.contains("evidence: 1 open transaction(s)\n"),
        "{report}"
    );
    assert!(report.contains("10 s"), "{report}");
    assert!(!report.contains("COMMIT PREPARED"), "{report}");
    assert!(
        report.contains("1 row(s) with an unreadable size skipped"),
        "{report}"
    );

    let empty = Online::new("empty-waldir");
    empty.set("pg_ls_waldir.tsv", "name\tsize\tmodification\n");
    empty.set(
        "pg_prepared_xacts.tsv",
        "gid\tprepared\towner\tdatabase\tage_s\n\
         ageless\t2026-01-01 00:00:00+00\tapp\tappdb\tnope\n",
    );
    let out = empty.run(&["--json", "--online"]);
    assert_no_panic(&out);
    let report = text(&out);
    assert!(report.contains("the listing is empty"), "{report}");
    assert!(report.contains("age unknown"), "{report}");
}

#[test]
fn prepared_transactions_disabled_and_long_psql_failure_is_truncated() {
    let online = Online::new("prepared-off");
    online.set(
        "pg_settings.tsv",
        "name\tsetting\tunit\tsource\nmax_prepared_transactions\t0\t\\N\tdefault\n",
    );
    let out = online.run(&["--online"]);
    assert_no_panic(&out);
    assert!(
        text(&out).contains("max_prepared_transactions=0"),
        "{}",
        text(&out)
    );

    let enabled = Online::new("prepared-empty");
    enabled.set(
        "pg_settings.tsv",
        "name\tsetting\tunit\tsource\nmax_prepared_transactions\t10\t\\N\tdefault\n",
    );
    let out = enabled.run(&["--online"]);
    assert_no_panic(&out);
    let report = text(&out);
    assert!(report.contains("[Ok] prepared_xacts"), "{report}");
    assert!(report.contains("no prepared transactions"), "{report}");

    let tmp = TempDir::new("long-err");
    let script = tmp.path().join("psql");
    let mut line = "x".repeat(299);
    line.push('é');
    line.push_str("TAILMARKER");
    write_exe(
        &script,
        &format!(
            "#!/bin/sh\nprintf '%s\\n' '{line}' >&2\nexit 2\n",
            line = line
        ),
    );
    let out = doctor()
        .args(["--dsn", "host=127.0.0.1 port=9 user=nobody dbname=postgres"])
        .env("PGWAL_DOCTOR_PSQL", &script)
        .env_remove("PGPASSWORD")
        .env_remove("PGPASSFILE")
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert_no_panic(&out);
    let report = text(&out);
    assert!(report.contains(&"x".repeat(299)), "{report}");
    assert!(!report.contains("TAILMARKER"), "{report}");
    assert!(!report.contains('é'), "{report}");

    let sqlstate = tmp.path().join("sqlstate-psql");
    write_exe(
        &sqlstate,
        "#!/bin/sh\necho 'ERROR:  08006 connection_failure' >&2\nexit 2\n",
    );
    let out = doctor()
        .args(["--online"])
        .env("PGWAL_DOCTOR_PSQL", &sqlstate)
        .env_remove("PGPASSWORD")
        .env_remove("PGPASSFILE")
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert_no_panic(&out);
    assert!(
        text(&out).contains("query failed: ERROR:  08006 connection_failure"),
        "{}",
        text(&out)
    );
}

#[test]
fn datadir_and_online_together_use_the_stand_in() {
    let tmp = TempDir::new("both");
    let datadir = tmp.path().join("datadir");
    fs::create_dir(&datadir).expect("mkdir");
    rebuilt_datadir(&datadir);
    let online = Online::new("both-psql");
    let out = doctor()
        .args([
            "--json",
            "--datadir",
            datadir.to_str().expect("utf-8"),
            "--online",
        ])
        .env("PGWAL_DOCTOR_PSQL", online.tmp.path().join("psql"))
        .env("FAKE_PSQL_DIR", online.tmp.path())
        .env_remove("PGPASSWORD")
        .env_remove("PGPASSFILE")
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert_no_panic(&out);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let checks: Vec<_> = value["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter_map(|finding| finding["check"].as_str())
        .collect();
    assert!(checks.contains(&"wal_dir"), "{checks:?}");
    assert!(checks.contains(&"settings"), "{checks:?}");
    assert!(checks.contains(&"waldir"), "{checks:?}");
}

#[test]
fn encoded_password_key_is_refused_before_psql_starts() {
    let tmp = TempDir::new("dsn-encoded-secret");
    let marker = tmp.path().join("psql-was-started");
    let script = tmp.path().join("psql");
    write_exe(
        &script,
        &format!("#!/bin/sh\ntouch '{}'\nexit 0\n", marker.display()),
    );
    for dsn in [
        "postgresql://h/db?pass%77ord=secretpw",
        "postgresql://h/db?%70assword=secretpw",
    ] {
        let out = doctor()
            .args(["--dsn", dsn])
            .env("PGWAL_DOCTOR_PSQL", &script)
            .env_remove("PGPASSWORD")
            .env_remove("PGPASSFILE")
            .output()
            .expect("run");
        assert_eq!(out.status.code(), Some(1), "{dsn}: {}", text(&out));
        assert_no_panic(&out);
        let all = format!("{}{}", text(&out), err_text(&out));
        assert!(!all.contains("secretpw"), "{all}");
        assert!(all.contains("DSN contains a password"), "{all}");
        // psql never ran, so the password never reached a process list.
        assert!(!marker.exists(), "psql started for {dsn}");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn full_stdout_exits_2_without_panic() {
    let full = fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .expect("open /dev/full");
    let tmp = TempDir::new("full-stdout");
    rebuilt_datadir(tmp.path());
    for json in [false, true] {
        let mut command = doctor();
        command.args(["--datadir", tmp.path().to_str().expect("utf-8")]);
        if json {
            command.arg("--json");
        }
        let out = command
            .stdout(full.try_clone().expect("clone"))
            .output()
            .expect("run");
        assert_eq!(
            out.status.code(),
            Some(2),
            "json={json}: {}",
            err_text(&out)
        );
        assert_no_panic(&out);
        assert!(
            err_text(&out).contains("cannot write the report"),
            "{}",
            err_text(&out)
        );
    }
}

#[test]
fn closed_stdout_pipe_exits_2_without_panic() {
    use std::process::Stdio;
    let tmp = TempDir::new("broken-pipe");
    rebuilt_datadir(tmp.path());
    let mut child = doctor()
        .args(["--datadir", tmp.path().to_str().expect("utf-8")])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    // Close the read end before the report is written.
    drop(child.stdout.take());
    let out = child.wait_with_output().expect("wait");
    assert_eq!(out.status.code(), Some(2), "{}", err_text(&out));
    assert_no_panic(&out);
    assert!(
        err_text(&out).contains("cannot write the report"),
        "{}",
        err_text(&out)
    );
}
