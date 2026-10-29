//! One module per cause of `pg_wal` growth.

mod util;
mod wal_dir;

pub(crate) use wal_dir::{check as wal_dir, pg_replslot, WalDirFacts};
