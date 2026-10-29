//! Helpers shared by the checks that read a [`PgSource`](crate::PgSource).

use crate::report::{Finding, Status};
use crate::source::{Rows, SourceError};

/// Check name used for failures that are not about one view (connection
/// refused, psql missing). Identical findings collapse into one.
pub(crate) const ONLINE: &str = "online";

/// Turn a failed query into a finding.
///
/// `grant_hint` is shown for permission errors.
pub(crate) fn error_finding(check: &str, err: &SourceError, grant_hint: &str) -> Finding {
    match err {
        SourceError::PermissionDenied => Finding::new(
            check,
            Status::NeedsRoot,
            None,
            None,
            "permission denied (SQLSTATE 42501)",
            grant_hint,
        ),
        SourceError::Missing(what) => Finding::new(
            check,
            Status::NotPresent,
            None,
            None,
            format!("not available on this server version: {what}"),
            "",
        ),
        SourceError::Other(message) => Finding::new(
            ONLINE,
            Status::Error,
            None,
            None,
            message.clone(),
            "check the PG* environment variables or --dsn, and that psql is installed",
        ),
    }
}

/// `setting` of the `pg_settings` row called `name`.
pub(crate) fn setting<'a>(settings: &'a Rows, name: &str) -> Option<&'a str> {
    (0..settings.len())
        .find(|&row| settings.get(row, "name").value() == Some(name))
        .and_then(|row| settings.get(row, "setting").value())
}
