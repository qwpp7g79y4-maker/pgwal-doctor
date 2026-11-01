# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Manual page `man/pgwal-doctor.1`, aligned with `pgwal-doctor --help`.
- `examples/quiet-datadir`, a fictional offline data directory, and
  `examples/README.md` with the command and its exit code.
- README (English and Dutch) names `--help`, `--version`, and the manual page.

## [0.1.0] - unreleased

### Added

- First version of `pgwal-doctor`, a read-only report of why PostgreSQL's
  `pg_wal` keeps growing. Not released: `publish = false`, no tag.
- Offline checks (`--datadir`): `wal_dir` (segments, bytes, symlinked
  `pg_wal`), `archive_backlog` (`.ready` / `.done` files in
  `pg_wal/archive_status`), `pg_replslot` (slot directory count).
- Online checks (`--dsn` or `--online`, through a `psql` subprocess, SELECT
  only): `slots`, `archiver`, `settings`, `transactions`, `prepared_xacts`,
  `waldir`.
- `summary`: ranks slot-retained WAL, archive backlog and `wal_keep_size`
  against the directory size and shows the remainder as "other or unknown".
- Text and JSON output, exit codes 0 (nothing actionable), 1 (Warn,
  NeedsRoot or Error), 2 (usage error).
- Column sets per PostgreSQL version (12, 13, 15, 17 modelled): a missing
  column is reported as not available, not as an error.
- Tests use query-output fixtures and a fake data directory; no PostgreSQL
  server is needed. The fixtures are hand-written, not recorded.
