//! One module per cause of `pg_wal` growth.

mod archive_backlog;
mod archiver;
mod online;
mod slots;
mod util;
mod wal_dir;

pub(crate) use archive_backlog::{check as archive_backlog, BacklogFacts};
pub(crate) use archiver::check as archiver;
pub(crate) use slots::{check as slots, SlotFacts};
pub(crate) use wal_dir::{check as wal_dir, pg_replslot, WalDirFacts};
