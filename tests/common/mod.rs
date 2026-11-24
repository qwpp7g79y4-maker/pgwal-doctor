//! Helpers shared by the integration tests.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use pgwal_doctor::{Finding, Report};

pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A scratch directory removed on drop. Permissions are restored first so
/// an unreadable directory does not block the cleanup.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(tag: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "pgwal-doctor-test-{}-{tag}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        restore_permissions(&self.path);
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(not(unix))]
fn restore_permissions(_path: &Path) {}

#[cfg(unix)]
fn restore_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    if meta.is_dir() {
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                restore_permissions(&entry.path());
            }
        }
    }
}

/// The finding of check `name`; panics when there is none or several.
pub fn only<'a>(report: &'a Report, name: &str) -> &'a Finding {
    let found: Vec<_> = report.findings.iter().filter(|f| f.check == name).collect();
    assert_eq!(found.len(), 1, "expected one `{name}` finding: {report:?}");
    found[0]
}

pub fn all<'a>(report: &'a Report, name: &str) -> Vec<&'a Finding> {
    report.findings.iter().filter(|f| f.check == name).collect()
}
