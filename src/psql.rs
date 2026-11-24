//! Online source: one `psql` subprocess per query.
//!
//! Choice for v0.1: no Postgres crate, so no TLS stack and no extra
//! dependencies. Cost: a `psql` binary (version 12 or later, for `--csv`) on
//! `PATH` and output parsing. CSV keeps a tab or line break in a value, such
//! as a prepared-transaction id, inside its cell.
//! Connection settings come from the libpq environment (`PGHOST`,
//! `PGUSER`, `PGPASSFILE`, ...) or from `--dsn`. A DSN that carries a
//! password is refused so the password never sits in a process list.

use std::ffi::OsString;
use std::process::Command;

use crate::source::{PgSource, Rows, SourceError};

const SLOTS_SQL: &str =
    "SELECT *, pg_current_wal_lsn() AS current_lsn FROM pg_replication_slots ORDER BY slot_name";
// The server compares the two timestamps: their text form depends on DateStyle
// and on the time zone offset, so it cannot be compared as text.
const ARCHIVER_SQL: &str = "SELECT *, CASE WHEN last_failed_time IS NULL THEN false \
     WHEN last_archived_time IS NULL THEN true \
     ELSE last_failed_time > last_archived_time END AS failed_after_archived \
     FROM pg_stat_archiver";
const ACTIVITY_SQL: &str = "SELECT pid, usename, state, xact_start, backend_xmin, \
     extract(epoch FROM (now() - xact_start))::bigint AS xact_age_s \
     FROM pg_stat_activity WHERE backend_type = 'client backend' AND pid <> pg_backend_pid() \
     ORDER BY pid";
const PREPARED_SQL: &str = "SELECT gid, prepared, owner, database, \
     extract(epoch FROM (now() - prepared))::bigint AS age_s FROM pg_prepared_xacts ORDER BY prepared";
const SETTINGS_SQL: &str = "SELECT name, setting, unit, source FROM pg_settings WHERE name IN \
     ('max_wal_size', 'min_wal_size', 'wal_keep_size', 'wal_keep_segments', \
      'max_slot_wal_keep_size', 'checkpoint_timeout', 'wal_segment_size', 'archive_mode', \
      'archive_command', 'archive_library', 'max_prepared_transactions', 'server_version_num') \
     ORDER BY name";
const WALDIR_SQL: &str = "SELECT name, size, modification FROM pg_ls_waldir() ORDER BY name";

/// Runs read-only queries through `psql`.
#[derive(Debug, Clone)]
pub struct PsqlSource {
    program: OsString,
    prefix_args: Vec<OsString>,
    dsn: Option<String>,
}

impl PsqlSource {
    /// `psql` from `PATH`, or the program named by `PGWAL_DOCTOR_PSQL`.
    /// Refuses a DSN that contains a password.
    pub fn new(dsn: Option<String>) -> Result<Self, SourceError> {
        let program = std::env::var_os("PGWAL_DOCTOR_PSQL").unwrap_or_else(|| "psql".into());
        Self::with_program(program, Vec::new(), dsn)
    }

    /// Like [`PsqlSource::new`] with an explicit program and leading
    /// arguments (tests run `sh script`).
    pub fn with_program(
        program: impl Into<OsString>,
        prefix_args: Vec<OsString>,
        dsn: Option<String>,
    ) -> Result<Self, SourceError> {
        if let Some(dsn) = &dsn {
            if dsn_has_password(dsn) {
                return Err(SourceError::Other(
                    "the DSN contains a password; put it in ~/.pgpass or PGPASSWORD instead".into(),
                ));
            }
        }
        Ok(Self {
            program: program.into(),
            prefix_args,
            dsn,
        })
    }

    fn run(&self, sql: &str) -> Result<Rows, SourceError> {
        let mut command = Command::new(&self.program);
        command.args(&self.prefix_args);
        command.args([
            "-X",
            "-q",
            "-w",
            "--csv",
            "-P",
            "null=\\N",
            "-v",
            "ON_ERROR_STOP=1",
            "-v",
            "VERBOSITY=sqlstate",
        ]);
        if let Some(dsn) = &self.dsn {
            command.arg("-d").arg(dsn);
        }
        command.arg("-c").arg(sql);
        // Read-only session and bounded waits. `default_transaction_read_only`
        // via PGOPTIONS is an assumption (not checked against the docs).
        let mut options = std::env::var("PGOPTIONS").unwrap_or_default();
        if !options.is_empty() {
            options.push(' ');
        }
        options.push_str("-c default_transaction_read_only=on -c statement_timeout=15000");
        command.env("PGOPTIONS", options);
        if std::env::var_os("PGCONNECT_TIMEOUT").is_none() {
            command.env("PGCONNECT_TIMEOUT", "10");
        }
        let output = command.output().map_err(|err| {
            SourceError::Other(format!(
                "cannot run {}: {err}",
                self.program.to_string_lossy()
            ))
        })?;
        if output.status.success() {
            return Rows::parse_csv(&String::from_utf8_lossy(&output.stdout));
        }
        Err(classify_failure(
            &String::from_utf8_lossy(&output.stderr),
            self.dsn.as_deref(),
        ))
    }
}

impl PgSource for PsqlSource {
    fn slots(&self) -> Result<Rows, SourceError> {
        self.run(SLOTS_SQL)
    }
    fn archiver(&self) -> Result<Rows, SourceError> {
        self.run(ARCHIVER_SQL)
    }
    fn activity(&self) -> Result<Rows, SourceError> {
        self.run(ACTIVITY_SQL)
    }
    fn prepared(&self) -> Result<Rows, SourceError> {
        self.run(PREPARED_SQL)
    }
    fn settings(&self) -> Result<Rows, SourceError> {
        self.run(SETTINGS_SQL)
    }
    fn waldir(&self) -> Result<Rows, SourceError> {
        self.run(WALDIR_SQL)
    }
}

/// Decode `%XX` escapes. A `%` without two hex digits stays as it is.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// True when a libpq key/value string or URI carries a password.
///
/// libpq decodes `%XX` in the keys of a URI query, so `pass%77ord=x` is a
/// password. The query keys are decoded before they are compared.
pub fn dsn_has_password(dsn: &str) -> bool {
    if let Some((_, tail)) = dsn.split_once("://") {
        if let Some((_, query)) = tail.split_once('?') {
            for pair in query.split('&') {
                let key = pair.split_once('=').map_or(pair, |(key, _)| key);
                if percent_decode(key).trim().eq_ignore_ascii_case("password") {
                    return true;
                }
            }
        }
    }
    let lower = dsn.to_ascii_lowercase();
    let mut rest = lower.as_str();
    while let Some(at) = rest.find("password") {
        let after = rest[at + "password".len()..].trim_start();
        if after.starts_with('=') {
            return true;
        }
        rest = &rest[at + "password".len()..];
    }
    if let Some((_, tail)) = dsn.split_once("://") {
        let authority = tail.split(['/', '?']).next().unwrap_or("");
        if let Some((userinfo, _)) = authority.rsplit_once('@') {
            return userinfo.contains(':');
        }
    }
    false
}

/// Map psql's stderr to a [`SourceError`]. Only the first meaningful line is
/// kept, and the DSN text is replaced, so nothing sensitive is echoed.
fn classify_failure(stderr: &str, dsn: Option<&str>) -> SourceError {
    for line in stderr.lines() {
        if let Some(rest) = line.strip_prefix("ERROR:") {
            let code = rest.split_whitespace().next().unwrap_or("");
            match code {
                "42501" => return SourceError::PermissionDenied,
                "42703" | "42883" | "42P01" => {
                    return SourceError::Missing(format!("SQLSTATE {code}"))
                }
                _ => return SourceError::Other(format!("query failed: {}", line.trim())),
            }
        }
    }
    let mut message = stderr
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("psql failed without a message")
        .trim()
        .to_string();
    if let Some(dsn) = dsn {
        if !dsn.is_empty() {
            message = message.replace(dsn, "<dsn>");
        }
    }
    if message.len() > 300 {
        let mut end = 300;
        while !message.is_char_boundary(end) {
            end -= 1;
        }
        message.truncate(end);
    }
    SourceError::Other(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn script_source(tag: &str, body: &str) -> (PsqlSource, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("pgwal-psql-{}-{tag}", std::process::id()));
        fs::create_dir_all(&dir).expect("mkdir");
        let script = dir.join("fake-psql.sh");
        fs::write(&script, body).expect("write script");
        let source =
            PsqlSource::with_program("sh", vec![script.into_os_string()], None).expect("source");
        (source, dir)
    }

    #[test]
    fn password_in_dsn_is_detected() {
        assert!(dsn_has_password("host=db user=u password=secret dbname=x"));
        assert!(dsn_has_password("host=db PASSWORD = secret"));
        assert!(dsn_has_password("postgresql://u:secret@db:5432/x"));
        assert!(dsn_has_password("postgresql://db/x?user=u&password=secret"));
        assert!(!dsn_has_password("postgresql://u@db:5432/x"));
        assert!(!dsn_has_password("host=db user=u dbname=x"));
        assert!(!dsn_has_password("postgresql://db/x?sslmode=require"));
    }

    #[test]
    fn percent_encoded_password_key_is_detected() {
        assert!(dsn_has_password("postgresql://h/db?pass%77ord=secretpw"));
        assert!(dsn_has_password("postgresql://h/db?%70assword=secretpw"));
        assert!(dsn_has_password("postgresql://h/db?%50ASS%57ORD=secretpw"));
        assert!(dsn_has_password(
            "postgresql://h/db?user=u&%70%61%73%73%77%6f%72%64=x"
        ));
        assert!(!dsn_has_password(
            "postgresql://h/db?sslmode=require&pass%77ordfile=/x"
        ));
        assert!(!dsn_has_password("postgresql://h/db?application_name=100%"));
    }

    #[test]
    fn percent_decode_keeps_bad_escapes() {
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("%zz%41"), "%zzA");
        assert_eq!(percent_decode("%"), "%");
    }

    #[test]
    fn archiver_query_lets_the_server_compare_the_times() {
        assert!(ARCHIVER_SQL.contains("last_failed_time > last_archived_time"));
        assert!(ARCHIVER_SQL.contains("failed_after_archived"));
    }

    #[test]
    fn new_refuses_a_dsn_with_a_password_without_echoing_it() {
        let err =
            PsqlSource::with_program("psql", Vec::new(), Some("host=h password=hunter2".into()))
                .expect_err("refused");
        assert!(!err.to_string().contains("hunter2"));
    }

    #[test]
    fn successful_output_is_parsed() {
        let (source, dir) = script_source("ok", "printf 'name,setting\\nmax_wal_size,\\\\N\\n'\n");
        let rows = source.settings().expect("rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows.get(0, "setting"), crate::source::Cell::Null);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn psql_is_asked_for_csv() {
        let (source, dir) = script_source(
            "args",
            "printf '%s\\n' \"$@\" > \"$0.args\"\nprintf 'name,setting\\n'\n",
        );
        source.settings().expect("rows");
        let args = fs::read_to_string(dir.join("fake-psql.sh.args")).expect("args");
        assert!(args.lines().any(|arg| arg == "--csv"), "{args}");
        assert!(!args.lines().any(|arg| arg == "-A"), "{args}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn quoted_tab_and_line_break_stay_in_their_cell() {
        let (source, dir) = script_source(
            "quoted",
            "printf 'gid,prepared,owner,database,age_s\\n\"x\\t\\t0\\nfake,1,mallory,db,9\",2026-01-01,app,db,10\\n'\n",
        );
        let rows = source.prepared().expect("rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows.get(0, "gid"),
            crate::source::Cell::Value("x\t\t0\nfake,1,mallory,db,9")
        );
        assert_eq!(rows.get(0, "owner"), crate::source::Cell::Value("app"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn sqlstate_42501_is_permission_denied() {
        let (source, dir) = script_source("denied", "echo 'ERROR:  42501' >&2\nexit 3\n");
        assert_eq!(source.waldir(), Err(SourceError::PermissionDenied));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn sqlstate_42703_is_missing() {
        let (source, dir) = script_source("missing", "echo 'ERROR:  42703' >&2\nexit 3\n");
        assert!(matches!(source.slots(), Err(SourceError::Missing(_))));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn connection_failure_is_other_and_hides_the_dsn() {
        let dsn = "postgresql://u@db.example:5432/x";
        let text = format!("psql: error: could not connect to {dsn}\n");
        let err = classify_failure(&text, Some(dsn));
        assert_eq!(
            err,
            SourceError::Other("psql: error: could not connect to <dsn>".into())
        );
    }

    #[test]
    fn missing_program_is_other() {
        let source = PsqlSource::with_program("/nonexistent/psql-for-test", Vec::new(), None)
            .expect("source");
        assert!(matches!(source.archiver(), Err(SourceError::Other(_))));
    }
}
