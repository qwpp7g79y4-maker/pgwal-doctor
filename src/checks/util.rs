use std::io;

/// `EACCES` and `EPERM` both surface as [`io::ErrorKind::PermissionDenied`].
pub(crate) fn is_permission(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::PermissionDenied
}
