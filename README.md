# pgwal-doctor

English | [Nederlands](README.nl.md)

Read-only report of why PostgreSQL's `pg_wal` keeps growing. It looks at the
usual causes in one pass and prints one report, so you do not have to query
six views by hand.

Status: version 0.1.0, unreleased (`publish = false`).
Manual page: [man/pgwal-doctor.1](man/pgwal-doctor.1).

## What it checks

Offline, from the data directory (`--datadir`):

- **wal_dir** — number and total size of WAL segments in `<datadir>/pg_wal`;
  reports when `pg_wal` is a symlink.
- **archive_backlog** — `.ready` / `.done` files in `pg_wal/archive_status`.
- **pg_replslot** — number of slot directories. Offline mode cannot confirm a
  slot as the cause: `pg_replslot/<name>/state` is binary and not parsed.

Online, through `psql` (`--dsn` or `--online`):

- **slots** — inactive slots and how far `restart_lsn` is behind, `wal_status`
  (`extended`, `unreserved`, `lost`), `max_slot_wal_keep_size`.
- **archiver** — `pg_stat_archiver` failures, plus `archive_mode`,
  `archive_command`, `archive_library` from `pg_settings`.
- **settings** — `max_wal_size`, `min_wal_size`, `wal_keep_size`
  (`wal_keep_segments` on PG 12), `max_slot_wal_keep_size`,
  `checkpoint_timeout`, `wal_segment_size`, converted to bytes.
- **transactions** and **prepared_xacts** — age of the oldest open and
  prepared transaction. Context only (see "Assumptions").
- **waldir** — size of `pg_wal` from `pg_ls_waldir()`.
- **summary** — ranks slot-retained WAL, archive backlog and `wal_keep_size`
  against the directory size. The causes overlap, so they need not add up;
  the remainder is shown as "other or unknown", never filled in.

## What it does NOT do

- It never writes to PostgreSQL or to the data directory. No `DROP`, no
  `CHECKPOINT`, no `pg_switch_wal`, no `COMMIT PREPARED`, no file deletion.
- It never advises deleting files from `pg_wal`.
- It does not parse `postgresql.conf` (online mode reads `pg_settings`).
- It does not parse `pg_replslot/*/state`.
- It does not open a network connection in offline mode. Online mode does,
  through `psql`.

## Installation

There is no crates.io release (`publish = false`). Build it from a clone of
the repository with Rust 1.85 or newer:

```sh
git clone https://github.com/qwpp7g79y4-maker/pgwal-doctor
cd pgwal-doctor
cargo install --path .
```

Or run `cargo build --release` and use `target/release/pgwal-doctor`. Online
mode also needs `psql` on `PATH`.

## Quick start

```sh
pgwal-doctor --datadir /var/lib/postgresql/17/main    # offline, reads files only
pgwal-doctor --online                                 # online, libpq environment
```

## Aan de slag in 5 minuten

1. Build the tool (see Installation).
2. Try it on the fictional data directory in this repository. It reads a few
   invented files, opens no network connection, and writes nothing:
   `pgwal-doctor --datadir examples/quiet-datadir`. The exit code is `0`.
3. Point `--datadir` at your own data directory, as the `postgres` user.
4. For the online checks, create the login role under "Permissions" and add
   `--dsn 'host=db1 user=pgwal_doctor dbname=postgres'`.
5. Read the `hint:` line under each finding. Thresholds are assumptions, not
   recommendations (see "Assumptions and unknowns").

## Usage

```sh
pgwal-doctor --datadir /var/lib/postgresql/17/main            # offline
pgwal-doctor --online                                         # libpq environment
pgwal-doctor --dsn 'host=db1 user=pgwal_doctor dbname=postgres'
pgwal-doctor --datadir /path --dsn 'service=prod' --json      # both, JSON
pgwal-doctor --help                                           # usage, exit 0
pgwal-doctor --version                                        # version, exit 0
```

Online mode needs `psql` on `PATH` (or set `PGWAL_DOCTOR_PSQL`). Connection
settings come from `--dsn` or the usual `PG*` environment variables. Online
mode starts only on `--dsn` or `--online`.

A DSN that contains a password is refused, so the password never sits in a
process list. Use `~/.pgpass` or `PGPASSWORD`. The keys of a URI query are
percent-decoded first, so `?pass%77ord=x` is refused too. Connection errors are
shown without the DSN.

## Example

Run from the repository root; the data directory is fictional (see
[examples/README.md](examples/README.md)):

```sh
pgwal-doctor --datadir examples/quiet-datadir
```

Output, shortened (the full report has a `hint:` line under each finding):

```text
pgwal-doctor

[Ok] summary bytes=47 count=-
evidence: pg_wal holds 47 bytes. archive backlog (.ready): 0 bytes; other or unknown: 47 bytes. ...

[Ok] wal_dir bytes=47 count=1
[Ok] archive_backlog bytes=0 count=0
[Ok] pg_replslot bytes=- count=0
```

The exit code is `0`.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Nothing actionable. |
| `1` | A check reported `Warn`, `NeedsRoot` or `Error` (a failing connection is an `Error`). |
| `2` | Usage error. |

## Permissions

Create a login role that is not a superuser:

```sql
CREATE ROLE pgwal_doctor LOGIN;
GRANT pg_monitor TO pgwal_doctor;
```

`pg_monitor` is a member of `pg_read_all_settings`, `pg_read_all_stats` and
`pg_stat_scan_tables` (PostgreSQL 17,
[predefined roles](https://www.postgresql.org/docs/17/predefined-roles.html)).
`pg_ls_waldir()` is limited to superusers and `pg_monitor` members unless
`EXECUTE` is granted
([functions-admin](https://www.postgresql.org/docs/17/functions-admin.html)).
Without a privilege, a check reports `NeedsRoot` with a hint and the other
checks still run. Offline mode needs read access to `pg_wal` (usually the
`postgres` user).

## Version differences

| Fact | Status | Source |
|---|---|---|
| PG 12 has `wal_keep_segments`, no `wal_keep_size`, no `max_slot_wal_keep_size` | confirmed | [12 docs](https://www.postgresql.org/docs/12/runtime-config-replication.html) |
| PG 12 `pg_replication_slots` has no `wal_status` / `safe_wal_size` | confirmed | [12 docs](https://www.postgresql.org/docs/12/view-pg-replication-slots.html) |
| PG 13 renames `wal_keep_segments` to `wal_keep_size` and adds `max_slot_wal_keep_size` | confirmed | [13.0 notes](https://www.postgresql.org/docs/release/13.0/) |
| `wal_status` / `safe_wal_size` arrive in PG 13 | **assumption** | 13.0 notes do not name the columns |
| PG 15 adds `archive_library` | confirmed | [15.0 notes](https://www.postgresql.org/docs/release/15.0/) |
| PG 17 adds `inactive_since` and `invalidation_reason` | confirmed | [17.0 notes](https://www.postgresql.org/docs/release/17.0/) |
| When the `conflicting` column was introduced | **unknown** | not checked |
| PG 14, 16 differences in `pg_stat_archiver`, `pg_ls_waldir`, privileges | **unknown** | not checked |
| PG 18 adds `idle_replication_slot_timeout` (not used by the tool) | confirmed | [18 docs](https://www.postgresql.org/docs/18/runtime-config-replication.html) |

The tool reads columns by name. A column the server lacks is reported as
"not available on this server version", not as an error.

## Assumptions and unknowns

These claims are not verified against a PostgreSQL source or docs page. The
tool and this README state them as assumptions.

- **Thresholds are assumptions**: 100 `.ready` files, an inactive slot
  retaining 1 GiB, a transaction older than 1 hour. They are not
  recommendations.
- `.ready` means "waiting for archiving" and `.done` means "archived" in
  `pg_wal/archive_status`. The docs pages read only name the directory, via
  `pg_ls_archive_statusdir()`.
- WAL segment names are 24 hex digits; `*.history`, `*.partial`, `*.backup`
  are not segments.
- The default segment size is 16 MB (used only when the server does not
  report `wal_segment_size`, for `wal_keep_segments` on PG 12).
- `pg_wal` can be a symlink (`initdb --waldir`). The storage-layout page does
  not say so.
- A read-only session via `PGOPTIONS` (`default_transaction_read_only=on`)
  works as intended.
- SQLSTATE `42501` means a missing privilege.
- On a standby, `pg_current_wal_lsn()` fails; the tool does not handle
  standbys yet. Treat results from a standby as untested.
- **Prepared and long transactions:** the PostgreSQL docs say prepared
  transactions hold locks and hinder VACUUM. They do not say these hold WAL
  back. That bloat from such transactions produces more WAL is an
  assumption, so the tool presents them as context, not as a cause.
- That slow or failing checkpoints delay the cleanup of old WAL is an assumption.
- How `wal_level`, `archive_timeout` or a slow standby affect growth is
  **unknown**; the tool does not check them.
- Whether the last archiver failure is newer than the last archived WAL is
  decided by the server, which compares the two `timestamptz` values
  (`failed_after_archived`). The tool never compares the displayed text, which
  depends on `DateStyle` and the time zone. This query is hand-written and was
  not run against a real server. If the server gives no comparison and
  failures are counted, the finding is a warning that says so.
- `pg_stat_archiver` can be clean while archiving fails: if the command is
  killed by a signal or exits above 125, the archiver restarts and the
  failure is not counted
  ([continuous archiving](https://www.postgresql.org/docs/17/continuous-archiving.html)).
  A clean result is therefore never proof; the tool combines it with the
  `.ready` backlog when `--datadir` is also given. Whether this case can be
  reproduced in fixtures is unknown.

Sources for the causes: [WAL configuration](https://www.postgresql.org/docs/17/wal-configuration.html),
[replication slots](https://www.postgresql.org/docs/17/view-pg-replication-slots.html),
[continuous archiving](https://www.postgresql.org/docs/17/continuous-archiving.html),
[replication settings](https://www.postgresql.org/docs/17/runtime-config-replication.html).
The pages were read through a summarising step, so check a quotation against
the page before relying on it.

## Maintenance

pgwal-doctor is a small project without a release schedule. Report bugs and
questions as GitHub Issues; see [CONTRIBUTING.md](CONTRIBUTING.md). Changes
are listed in [CHANGELOG.md](CHANGELOG.md), security reports go as described
in [SECURITY.md](SECURITY.md), and what was tested, and what was not, is in
[QUALITY.md](QUALITY.md).

## Tests

Tests need no PostgreSQL: query output comes from files in
`tests/fixtures/pg/`, and a fake `psql` script covers the command line. The
fixtures are hand-written from the documented column lists, not recorded from
real servers (see `tests/fixtures/pg/README.md`).

## License

Licensed under either of [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE) at your option.
