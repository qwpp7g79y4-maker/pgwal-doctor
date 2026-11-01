//! Answers from a live PostgreSQL, behind a trait so tests never need one.
//!
//! Every method is one read-only query. Results are plain tab-separated
//! tables: a header line, then one line per row, `\N` for SQL NULL. That is
//! what `psql -A -F <tab> -P null='\N'` prints and what the fixture files
//! under `tests/fixtures/pg` contain.

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
    /// Anything else: connection failure, unparsable output, ...
    Other(String),
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PermissionDenied => f.write_str("permission denied"),
            Self::Missing(what) => write!(f, "not available on this server: {what}"),
            Self::Other(message) => f.write_str(message),
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
    /// Parse tab-separated psql output. An empty text, or a row whose cell
    /// count differs from the header, is an error.
    pub fn parse_tsv(text: &str) -> Result<Self, SourceError> {
        let mut lines = text.lines();
        let header = match lines.next() {
            Some(line) if !line.is_empty() => line,
            _ => return Err(SourceError::Other("empty output: no header line".into())),
        };
        let columns: Vec<String> = header.split('\t').map(str::to_string).collect();
        let mut rows = Vec::new();
        for (index, line) in lines.enumerate() {
            let cells: Vec<Option<String>> = line
                .split('\t')
                .map(|cell| {
                    if cell == "\\N" {
                        None
                    } else {
                        Some(cell.to_string())
                    }
                })
                .collect();
            if cells.len() != columns.len() {
                return Err(SourceError::Other(format!(
                    "row {}: expected {} cells, got {}",
                    index + 1,
                    columns.len(),
                    cells.len()
                )));
            }
            rows.push(cells);
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
        assert_eq!(
            err,
            SourceError::Other("row 1: expected 2 cells, got 1".into())
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
}
