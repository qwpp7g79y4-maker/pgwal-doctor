# Query-output fixtures

One directory per scenario. Each file is the output of one query, as
`psql -X -A -F '<tab>' -P null='\N' -P footer=off` prints it: a header line,
then one line per row, `\N` for SQL NULL.

| File | Query |
|---|---|
| `pg_replication_slots.tsv` | `SELECT *, pg_current_wal_lsn() AS current_lsn FROM pg_replication_slots` |
| `pg_stat_archiver.tsv` | `pg_stat_archiver` plus the column `failed_after_archived` (see `src/psql.rs`) |
| `pg_stat_activity.tsv` | client sessions with `xact_age_s` (see `src/psql.rs`) |
| `pg_prepared_xacts.tsv` | `pg_prepared_xacts` with `age_s` (see `src/psql.rs`) |
| `pg_settings.tsv` | `name, setting, unit, source` for the WAL settings |
| `pg_ls_waldir.tsv` | `SELECT name, size, modification FROM pg_ls_waldir()` |

A `<view>.denied` file stands for SQLSTATE 42501. A missing file stands for a
view the server does not have.

## These fixtures are hand-written, not recorded

No PostgreSQL server was available when they were made. They were written by
hand from the column lists in the PostgreSQL documentation, and the column
order, the exact `wal_status` values per version and the timestamp format are
from memory, not from a real server. The tool reads columns by name, so the
order does not matter to it, but treat the files as shaped examples.

Directories named `pg12`, `pg13`, `pg15`, `pg17` model the column set of that
major version (PG 12: no `wal_status`; PG 13: `wal_status`, `safe_wal_size`;
PG 17: `inactive_since`, `invalidation_reason`). Other directories are single
scenarios on top of PG 17.

## Recording real output (still to do)

On a throw-away cluster outside this repository, once per major version:

```sh
psql -X -A -F "$(printf '\t')" -P null='\N' -P footer=off \
  -c "SELECT *, pg_current_wal_lsn() AS current_lsn FROM pg_replication_slots" \
  > pg_replication_slots.tsv
```

and likewise for the other files, using the queries in `src/psql.rs`. Replace
the hand-written files and rerun the tests.
