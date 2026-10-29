//! Prefix that keeps every check inside one tree.
//!
//! Production uses the PostgreSQL data directory. Tests pass a fixture
//! directory. Logical paths such as `/pg_wal` are joined onto that prefix, so
//! a check never opens a host path by accident.

use std::path::{Component, Path, PathBuf};

/// Filesystem prefix for every path a check reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    prefix: PathBuf,
}

impl Root {
    pub fn new(prefix: impl Into<PathBuf>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }

    pub fn prefix(&self) -> &Path {
        &self.prefix
    }

    /// Map a logical absolute path onto this prefix.
    ///
    /// `/pg_wal` with prefix `fixtures` becomes `fixtures/pg_wal`. Parent
    /// components (`..`) are dropped so a logical path cannot climb out of
    /// the prefix. Symlinks on disk are not looked at here.
    pub fn join(&self, logical: impl AsRef<Path>) -> PathBuf {
        let logical = logical.as_ref();
        let relative = logical.strip_prefix("/").unwrap_or(logical);
        let mut out = self.prefix.clone();
        for component in relative.components() {
            match component {
                Component::Normal(part) => out.push(part),
                Component::CurDir
                | Component::ParentDir
                | Component::RootDir
                | Component::Prefix(_) => {}
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_keeps_logical_path_under_the_prefix() {
        let root = Root::new("/tmp/pgwal-fixture");
        let joined = root.join("/pg_wal/archive_status");
        assert_eq!(
            joined,
            PathBuf::from("/tmp/pgwal-fixture/pg_wal/archive_status")
        );
    }

    #[test]
    fn join_of_root_is_the_prefix() {
        let root = Root::new("/tmp/pgwal-fixture");
        assert_eq!(root.join("/"), PathBuf::from("/tmp/pgwal-fixture"));
    }

    #[test]
    fn join_drops_parent_components() {
        let root = Root::new("/tmp/pgwal-fixture");
        let joined = root.join("/pg_wal/../../etc/passwd");
        assert!(joined.starts_with("/tmp/pgwal-fixture"));
        assert_eq!(
            joined,
            PathBuf::from("/tmp/pgwal-fixture/pg_wal/etc/passwd")
        );
    }
}
