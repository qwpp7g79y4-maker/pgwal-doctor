//! Answers from a live PostgreSQL, behind a trait so tests never need one.
//!
//! Every method is one read-only query. `psql --csv -P null='\N'` prints a
//! header record and one record per row, with quoting, so a tab, a comma or a
//! line break inside a value stays inside its cell. The fixture files under
//! `tests/fixtures/pg` are plain tab-separated tables with the same header,
//! rows and `\N` for SQL NULL.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Why a query gave no table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    /// SQLSTATE `42501` (insufficient_privilege) or its fixture stand-in.
    PermissionDenied,
    /// The view, function or column does not exist on this server version.
    Missing(String),
    /// `psql` ran but its output is not a readable table (broken CSV,
    /// a cut-off result).
    Malformed(String),
    /// Anything else: connection failure, missing program, ...
    Other(String),
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PermissionDenied => f.write_str("permission denied"),
            Self::Missing(what) => write!(f, "not available on this server: {what}"),
            Self::Malformed(message) | Self::Other(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for SourceError {}

/// One cell: absent column, SQL NULL, or a value (possibly empty).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell<'a> {
    Missing,
    Null,
    Value(&'a str),
}

impl<'a> Cell<'a> {
    /// The text of a non-NULL cell.
    pub fn value(self) -> Option<&'a str> {
        match self {
            Self::Value(text) => Some(text),
            Self::Missing | Self::Null => None,
        }
    }
}

/// A parsed result table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rows {
    columns: Vec<String>,
    rows: Vec<Vec<Option<String>>>,
}

impl Rows {
    /// Parse tab-separated text (the fixture files). An empty text, or a row
    /// whose cell count differs from the header, is an error.
    pub fn parse_tsv(text: &str) -> Result<Self, SourceError> {
        let mut lines = text.lines();
        let header = match lines.next() {
            Some(line) if !line.is_empty() => line,
            _ => return Err(SourceError::Other("empty output: no header line".into())),
        };
        let columns: Vec<String> = header.split('\t').map(str::to_string).collect();
        let records = lines
            .map(|line| {
                line.split('\t')
                    .map(|cell| (cell.to_string(), false))
                    .collect()
            })
            .collect();
        Self::from_records(columns, records)
    }

    /// Parse `psql --csv` output: RFC 4180 quoting, `\N` (unquoted) is SQL
    /// NULL. Quoted cells may hold commas, tabs, quotes and line breaks.
    pub fn parse_csv(text: &str) -> Result<Self, SourceError> {
        let mut records = csv_records(text)?;
        if records.is_empty() || (records[0].len() == 1 && records[0][0].0.is_empty()) {
            return Err(SourceError::Other("empty output: no header line".into()));
        }
        let columns = records
            .remove(0)
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        Self::from_records(columns, records)
    }

    /// Records are `(text, was_quoted)` cells; only an unquoted `\N` is NULL.
    fn from_records(
        columns: Vec<String>,
        records: Vec<Vec<(String, bool)>>,
    ) -> Result<Self, SourceError> {
        let mut rows = Vec::new();
        for (index, record) in records.into_iter().enumerate() {
            if record.len() != columns.len() {
                return Err(SourceError::Malformed(format!(
                    "row {}: expected {} cells, got {}; the psql output is not the table that was asked for",
                    index + 1,
                    columns.len(),
                    record.len()
                )));
            }
            rows.push(
                record
                    .into_iter()
                    .map(|(cell, quoted)| (quoted || cell != "\\N").then_some(cell))
                    .collect(),
            );
        }
        Ok(Self { columns, rows })
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    pub fn has_column(&self, name: &str) -> bool {
        self.columns.iter().any(|column| column == name)
    }

    pub fn get(&self, row: usize, name: &str) -> Cell<'_> {
        let Some(index) = self.columns.iter().position(|column| column == name) else {
            return Cell::Missing;
        };
        match self.rows.get(row).and_then(|cells| cells.get(index)) {
            Some(Some(text)) => Cell::Value(text),
            Some(None) => Cell::Null,
            None => Cell::Missing,
        }
    }
}

/// Read-only answers from one PostgreSQL server.
pub trait PgSource {
    /// `pg_replication_slots` (all columns the server has) plus `current_lsn`.
    fn slots(&self) -> Result<Rows, SourceError>;
    /// `pg_stat_archiver`.
    fn archiver(&self) -> Result<Rows, SourceError>;
    /// Client sessions from `pg_stat_activity`.
    fn activity(&self) -> Result<Rows, SourceError>;
    /// `pg_prepared_xacts`.
    fn prepared(&self) -> Result<Rows, SourceError>;
    /// `pg_settings` rows for the WAL-related settings.
    fn settings(&self) -> Result<Rows, SourceError>;
    /// `pg_ls_waldir()`.
    fn waldir(&self) -> Result<Rows, SourceError>;
}

/// Split CSV text into records of `(cell, was_quoted)`. A line break ends a
/// record only outside quotes. A final line break does not start a record.
fn csv_records(text: &str) -> Result<Vec<Vec<(String, bool)>>, SourceError> {
    let mut records = Vec::new();
    let mut record: Vec<(String, bool)> = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut in_quotes = false;
    let mut pending = false; // something of the current record was read
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        pending = true;
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    cell.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                cell.push(c);
            }
            continue;
        }
        match c {
            '"' if cell.is_empty() && !quoted => {
                in_quotes = true;
                quoted = true;
            }
            '"' => {
                return Err(SourceError::Malformed(
                    "the psql CSV output has a stray quote inside a value".into(),
                ))
            }
            ',' => record.push((std::mem::take(&mut cell), std::mem::take(&mut quoted))),
            '\n' => {
                record.push((std::mem::take(&mut cell), std::mem::take(&mut quoted)));
                records.push(std::mem::take(&mut record));
                pending = false;
            }
            _ if quoted => {
                return Err(SourceError::Malformed(
                    "the psql CSV output has text after a closing quote".into(),
                ))
            }
            _ => cell.push(c),
        }
    }
    if in_quotes {
        return Err(SourceError::Malformed(
            "the psql CSV output ends inside a quoted value (cut-off output)".into(),
        ));
    }
    if pending {
        record.push((cell, quoted));
        records.push(record);
    }
    Ok(records)
}

/// Reads `<dir>/<view>.tsv`. A `<view>.denied` file stands for SQLSTATE
/// 42501; a missing file stands for a view the server does not have.
#[derive(Debug, Clone)]
pub struct FixtureSource {
    dir: PathBuf,
}

impl FixtureSource {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn load(&self, view: &str) -> Result<Rows, SourceError> {
        if self.dir.join(format!("{view}.denied")).exists() {
            return Err(SourceError::PermissionDenied);
        }
        let path = self.dir.join(format!("{view}.tsv"));
        match read_regular_file(&path) {
            Ok(bytes) => Rows::parse_tsv(&String::from_utf8_lossy(&bytes)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                Err(SourceError::Missing(view.to_string()))
            }
            Err(err) => Err(SourceError::Other(format!("{}: {err}", path.display()))),
        }
    }
}

/// Read one named file.
///
/// The path is stat'd first. Anything that is not a regular file — a fifo, a
/// device, or a directory — is refused before it is opened, so a pipe with no
/// writer cannot block and a device is not read. On Unix the open that follows
/// is non-blocking and the descriptor is checked again with `fstat`, in case
/// the path changed after the first stat. Listing a directory stays on `read_dir`.
fn read_regular_file(path: &Path) -> io::Result<Vec<u8>> {
    let meta = fs::metadata(path)?;
    if !meta.is_file() {
        return Err(not_regular_file());
    }
    #[cfg(unix)]
    {
        read_regular_file_unix(path)
    }
    #[cfg(not(unix))]
    {
        fs::read(path)
    }
}

fn not_regular_file() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "not a regular file")
}

/// `O_NONBLOCK` from `<fcntl.h>`. Linux uses `04000`; Solaris uses `0x80`;
/// the other Unixes std supports use `0x4`.
#[cfg(unix)]
fn o_nonblock() -> i32 {
    #[cfg(any(target_os = "linux", target_os = "android", target_os = "redox"))]
    let flag = 0o4000;
    #[cfg(any(target_os = "solaris", target_os = "illumos"))]
    let flag = 0x80;
    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "redox",
        target_os = "solaris",
        target_os = "illumos",
    )))]
    let flag = 0x4;
    flag
}

#[cfg(unix)]
fn read_regular_file_unix(path: &Path) -> io::Result<Vec<u8>> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(o_nonblock())
        .open(path)?;
    // `File::metadata` is `fstat` on the opened descriptor.
    if !file.metadata()?.is_file() {
        return Err(not_regular_file());
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

impl PgSource for FixtureSource {
    fn slots(&self) -> Result<Rows, SourceError> {
        self.load("pg_replication_slots")
    }
    fn archiver(&self) -> Result<Rows, SourceError> {
        self.load("pg_stat_archiver")
    }
    fn activity(&self) -> Result<Rows, SourceError> {
        self.load("pg_stat_activity")
    }
    fn prepared(&self) -> Result<Rows, SourceError> {
        self.load("pg_prepared_xacts")
    }
    fn settings(&self) -> Result<Rows, SourceError> {
        self.load("pg_settings")
    }
    fn waldir(&self) -> Result<Rows, SourceError> {
        self.load("pg_ls_waldir")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_empty_string_and_value_are_distinct() {
        let rows = Rows::parse_tsv("a\tb\tc\n\\N\t\tx\n").expect("parse");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows.get(0, "a"), Cell::Null);
        assert_eq!(rows.get(0, "b"), Cell::Value(""));
        assert_eq!(rows.get(0, "c"), Cell::Value("x"));
        assert_eq!(rows.get(0, "d"), Cell::Missing);
        assert_eq!(rows.get(1, "a"), Cell::Missing);
    }

    #[test]
    fn invalid_row_and_empty_output_are_errors() {
        let err = Rows::parse_tsv("a\tb\n1\n").expect_err("short row");
        assert!(matches!(err, SourceError::Malformed(_)), "{err:?}");
        assert!(
            err.to_string()
                .starts_with("row 1: expected 2 cells, got 1"),
            "{err}"
        );
        assert!(Rows::parse_tsv("").is_err());
        assert!(Rows::parse_tsv("a\tb\n1\t2\t3\n").is_err());
    }

    #[test]
    fn header_only_output_is_an_empty_table() {
        let rows = Rows::parse_tsv("a\tb\n").expect("parse");
        assert!(rows.is_empty());
        assert!(rows.has_column("a"));
        assert_eq!(rows.columns(), ["a", "b"]);
    }

    #[test]
    fn fixture_source_maps_denied_and_missing_files() {
        let dir = std::env::temp_dir().join(format!("pgwal-source-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("mkdir");
        fs::write(dir.join("pg_stat_archiver.denied"), "").expect("denied");
        fs::write(dir.join("pg_settings.tsv"), "name\tsetting\nx\t1\n").expect("tsv");
        let source = FixtureSource::new(&dir);
        assert_eq!(source.archiver(), Err(SourceError::PermissionDenied));
        assert_eq!(
            source.slots(),
            Err(SourceError::Missing("pg_replication_slots".into()))
        );
        assert_eq!(source.settings().expect("settings").len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    /// A fifo (or a symlink to one), a device, or a directory is an error.
    /// The read returns within two seconds and does not panic.
    #[cfg(unix)]
    #[test]
    fn load_rejects_fifo_device_and_directory() {
        use std::os::unix::fs::symlink;
        use std::process::Command;
        use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "pgwal-source-special-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&base).expect("mkdir");

        let fifo_dir = base.join("fifo");
        fs::create_dir(&fifo_dir).expect("mkdir fifo");
        let pipe = fifo_dir.join("pipe");
        assert!(
            Command::new("mkfifo")
                .arg(&pipe)
                .status()
                .expect("mkfifo")
                .success(),
            "mkfifo"
        );
        symlink(&pipe, fifo_dir.join("pg_settings.tsv")).expect("symlink to fifo");
        let direct = fifo_dir.join("pg_replication_slots.tsv");
        assert!(
            Command::new("mkfifo")
                .arg(&direct)
                .status()
                .expect("mkfifo")
                .success(),
            "mkfifo direct"
        );

        let source = FixtureSource::new(&fifo_dir);
        let started = Instant::now();
        let via_symlink = source.settings().expect_err("symlink to fifo");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "symlink to a fifo hung"
        );
        assert!(
            matches!(via_symlink, SourceError::Other(ref message) if message.contains("not a regular file")),
            "{via_symlink}"
        );
        let started = Instant::now();
        let direct_err = source.slots().expect_err("fifo");
        assert!(started.elapsed() < Duration::from_secs(2), "fifo hung");
        assert!(
            matches!(direct_err, SourceError::Other(ref message) if message.contains("not a regular file")),
            "{direct_err}"
        );

        let report = crate::scan(None, Some(&source));
        assert_ne!(report.exit_code(), 0);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.evidence.contains("not a regular file")),
            "{report:?}"
        );

        let dev_dir = base.join("dev");
        fs::create_dir(&dev_dir).expect("mkdir dev");
        symlink("/dev/zero", dev_dir.join("pg_settings.tsv")).expect("symlink to device");
        let started = Instant::now();
        let device = FixtureSource::new(&dev_dir).settings().expect_err("device");
        assert!(started.elapsed() < Duration::from_secs(2), "device hung");
        assert!(
            matches!(device, SourceError::Other(ref message) if message.contains("not a regular file")),
            "{device}"
        );

        let as_dir = base.join("dir");
        fs::create_dir_all(as_dir.join("pg_settings.tsv")).expect("directory named as a file");
        let directory = FixtureSource::new(&as_dir)
            .settings()
            .expect_err("directory");
        assert!(
            matches!(directory, SourceError::Other(ref message) if message.contains("not a regular file")),
            "{directory}"
        );

        let ok_dir = base.join("ok");
        fs::create_dir(&ok_dir).expect("mkdir ok");
        let real = ok_dir.join("real.tsv");
        fs::write(&real, "name\tsetting\nx\t1\n").expect("write");
        symlink(&real, ok_dir.join("pg_settings.tsv")).expect("symlink to file");
        assert_eq!(
            FixtureSource::new(&ok_dir)
                .settings()
                .expect("symlink to a regular file")
                .len(),
            1
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn csv_keeps_tab_comma_quote_and_newline_in_a_cell() {
        let rows =
            Rows::parse_csv("a,b,c\n\"x\ty,\"\"q\"\"\nz\",2,\\N\n\"\\N\",,\"\"\n").expect("parse");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows.get(0, "a"), Cell::Value("x\ty,\"q\"\nz"));
        assert_eq!(rows.get(0, "b"), Cell::Value("2"));
        assert_eq!(rows.get(0, "c"), Cell::Null);
        // A quoted `\N` is the text `\N`; empty cells are empty strings.
        assert_eq!(rows.get(1, "a"), Cell::Value("\\N"));
        assert_eq!(rows.get(1, "b"), Cell::Value(""));
        assert_eq!(rows.get(1, "c"), Cell::Value(""));
    }

    #[test]
    fn csv_header_only_and_bad_input() {
        assert_eq!(Rows::parse_csv("a,b\n").expect("parse").len(), 0);
        assert!(Rows::parse_csv("").is_err());
        let cut = Rows::parse_csv("a\n\"open\n").expect_err("cut off");
        assert!(matches!(cut, SourceError::Malformed(_)), "{cut:?}");
        assert!(cut.to_string().contains("quoted value"), "{cut}");
        let short = Rows::parse_csv("a,b\n1\n").expect_err("short row");
        assert!(matches!(short, SourceError::Malformed(_)), "{short:?}");
        assert!(Rows::parse_csv("a\nx\"y\n").is_err());
        assert!(Rows::parse_csv("a\n\"x\"y\n").is_err());
    }
}
