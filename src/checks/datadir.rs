//! Offline: is `--datadir` a PostgreSQL data directory at all?
//!
//! A typo such as `/nonexistent`, or `/tmp`, must not give "nothing to do".
//! A data directory has a `PG_VERSION` file and a `pg_wal` entry. The
//! `NotPresent` findings of the other checks stay for a real data directory
//! in which an optional piece is missing, such as `archive_status`.

use std::fs;
use std::io;

use crate::report::{Finding, Status};
use crate::root::Root;

const CHECK: &str = "datadir";

/// True for the errors that mean "this entry is not there". Permission and
/// other errors are left to the checks that read the entry.
fn is_absent(err: &io::Error) -> bool {
    // `NotADirectory` is not stable in every supported toolchain; a file
    // used as a directory reports it as `ENOTDIR`, raw os error 20 on Unix.
    err.kind() == io::ErrorKind::NotFound || err.raw_os_error() == Some(20)
}

pub(crate) fn check(root: &Root) -> Option<Finding> {
    let mut missing = Vec::new();
    match fs::metadata(root.join("/PG_VERSION")) {
        Ok(meta) if !meta.is_file() => missing.push("PG_VERSION"),
        Err(err) if is_absent(&err) => missing.push("PG_VERSION"),
        _ => {}
    }
    // `symlink_metadata`: a dangling `pg_wal` link is reported by `wal_dir`.
    if let Err(err) = fs::symlink_metadata(root.join("/pg_wal")) {
        if is_absent(&err) {
            missing.push("pg_wal");
        }
    }
    if missing.is_empty() {
        return None;
    }
    Some(Finding::new(
        CHECK,
        Status::Error,
        None,
        None,
        format!(
            "{} is not a PostgreSQL data directory: {} not found",
            root.prefix().display(),
            missing.join(" and ")
        ),
        "--datadir must be the data directory itself (the one with PG_VERSION and pg_wal); check the path",
    ))
}
