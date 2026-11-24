# Contributing

Contributions start with a GitHub Issue. Open an issue in the
[Issues](https://github.com/qwpp7g79y4-maker/pgwal-doctor/issues) tab of the
repository before you write code: describe what you saw or what you want, and
which PostgreSQL version and mode (offline or online) it concerns. Please do
not send a pull request for something nobody has discussed in an issue yet.

Security problems do not belong in a public issue. Follow
[SECURITY.md](SECURITY.md).

## What the tool will not become

pgwal-doctor is read-only. It does not write to PostgreSQL or to the data
directory, and it does not advise deleting files from `pg_wal`. Issues that
ask for this will be closed.

## Before you propose a change

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The tests need no PostgreSQL. The tests that use symlinks, file modes or an
`sh` stand-in for `psql` (`tests/wal_dir.rs`, `tests/binary_e2e.rs` and part of
`tests/cli.rs`) are marked `cfg(unix)`: they run on Unix only and are left out
on Windows, where the other tests still compile and run. A change in behaviour comes with a test, and a
change in an option comes with an update of `man/pgwal-doctor.1` and both
READMEs. `tests/doc_headings.rs` checks that README.md and README.nl.md keep
the same sections and that this file points to Issues and to SECURITY.md.

Licensing: your contribution is offered under the same terms as the project,
MIT or Apache-2.0 at the user's option (see the README).
