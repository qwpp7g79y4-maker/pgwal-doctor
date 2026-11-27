# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0] - 2026-10-29

Changes made after 2026-10-29; the version number did not change.

### Fixed

- `--datadir` on a path that is not a data directory (no `PG_VERSION` or no
  `pg_wal`, such as `/nonexistent` or `/tmp`) is an `Error` finding and exits
  1, no longer three `NotPresent` lines and exit 0. `NotPresent` stays for a
  real data directory in which an optional part is missing.
- `psql` is run with `--csv` instead of tab-separated output. A tab or line
  break in a prepared-transaction id, a user name or an `archive_command` stays
  inside its cell: it can no longer forge a row or break the settings. Output
  that is not a readable table is its own `Error` that names the cause. This
  needs `psql` 12 or later.
- The test targets compile for Windows: the Unix-only tests (symlinks, file
  modes, `sh` stand-ins) are marked `cfg(unix)` and run on Unix only.
- A full or closed stdout no longer panics: the report is written to a stdout
  lock, and a write error gives exit code 2 with a message on stderr.
- A DSN with a percent-encoded `password` key in the URI query
  (`?pass%77ord=x`) is refused like a plain one.
- The archiver check no longer compares the displayed timestamps as text: the
  server compares `last_failed_time` and `last_archived_time`
  (`failed_after_archived`). Without that column, counted failures give a
  warning instead of `Ok`.

### Added

- Manual page `man/pgwal-doctor.1`, aligned with `pgwal-doctor --help`.
- `examples/quiet-datadir`, a fictional offline data directory, and
  `examples/README.md` with the command and its exit code.
- README (English and Dutch) names `--help`, `--version`, and the manual page.
- README (English and Dutch): sections Installation, Quick start, a
  five-minute walkthrough, Example, Exit codes and Maintenance.
- `CONTRIBUTING.md` (contributions start with an Issue), `QUALITY.md` and
  `QUALITY.nl.md` (test count, tested commit, what was not run), and
  `KEUR-NVT.md` (modules this tool does not have, with the reason).
- Test `tests/doc_headings.rs`: both READMEs keep the same sections.


### Added

- First version of `pgwal-doctor`, a read-only report of why PostgreSQL's
  `pg_wal` keeps growing. No tag.
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
