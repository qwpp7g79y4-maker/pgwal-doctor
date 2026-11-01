# Examples

Fictional fixtures for `pgwal-doctor`. Nothing here is a copy of a real PostgreSQL data directory. The command below does not open a socket, a network connection, or `/var/lib/postgresql`.

## Quiet data directory

`examples/quiet-datadir` is an invented offline data directory:

- `PG_VERSION` contains `99` (not a real server version).
- `pg_wal/000000010000000000000001` is a short text file whose name is 24 hex digits. It is not a WAL record.
- `pg_wal/archive_status/000000010000000000000001.done` is an invented archive marker. There is no `.ready` file.
- `pg_replslot/not-a-slot` is a file, so the tool does not count a slot directory.

Run this from the repository root:

```sh
pgwal-doctor --datadir examples/quiet-datadir
```

Expected exit code: 0
