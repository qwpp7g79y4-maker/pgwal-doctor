use std::io;

/// `EACCES` and `EPERM` both surface as [`io::ErrorKind::PermissionDenied`].
pub(crate) fn is_permission(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::PermissionDenied
}

/// Collects at most `limit` names and counts the rest, so a report about
/// thousands of files stays short.
#[derive(Debug, Default)]
pub(crate) struct NameSample {
    names: Vec<String>,
    total: u64,
}

impl NameSample {
    pub(crate) fn push(&mut self, name: String, limit: usize) {
        self.total += 1;
        if self.names.len() < limit {
            self.names.push(name);
        }
    }

    pub(crate) fn total(&self) -> u64 {
        self.total
    }

    /// `a, b, c and 4995 more`.
    pub(crate) fn render(&self) -> String {
        let shown = self.names.join(", ");
        let left = self.total - self.names.len() as u64;
        if left > 0 {
            format!("{shown} and {left} more")
        } else {
            shown
        }
    }
}
