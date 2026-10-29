# Security policy

pgwal-doctor is a read-only command-line tool. It never writes, deletes or
changes any file in the PostgreSQL data directory and never writes to
PostgreSQL.

Offline mode (`--datadir`) only reads the file system and opens no network
connection. Online mode (`--dsn` or `--online`) is a deliberate difference
from that: it starts `psql` as a subprocess, which connects to the server you
name. The tool runs `SELECT` queries only, in a session with
`default_transaction_read_only=on` (set through `PGOPTIONS`; that this setting
is honoured is an assumption, not checked against the PostgreSQL docs). It
never runs `DROP`, `CHECKPOINT`, `pg_switch_wal`, `COMMIT PREPARED` or any
other statement that changes state.

Passwords: a DSN that contains a password is refused, so the password never
appears in a process list. Use `~/.pgpass` or `PGPASSWORD`. The tool does not
read or print secrets, and a failing connection is reported without the DSN.

## Reporting a vulnerability

Email pep_ai@icloud.com with a description and reproduction steps. There is
currently no bug bounty.
