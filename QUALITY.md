# Quality

English | [Nederlands](QUALITY.nl.md)

Measured on 2026-11-24, Linux x86_64, Rust 1.99.0.

## Tested commit

Code commit `8bd800384010bf3b3cee5926d3f7c925d83e4d61`, the last commit that
changes anything other than documents. Commits after it change only
documents (README, CHANGELOG, QUALITY, CONTRIBUTING, KEUR-NVT).

## Results

| Check | Result |
|---|---|
| `cargo test --locked` | 130 passed, 0 failed |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | no warnings |
| `cargo clippy --tests --locked --target x86_64-pc-windows-gnu -- -D warnings` | no warnings |
| `cargo check --tests --locked --target x86_64-pc-windows-gnu` | ok |
| `cargo deny check licenses` | `licenses ok` |

The 130 tests are 34 unit tests and 96 integration tests, summed over all
`cargo test` result lines. They use hand-written fixtures and a fake `psql`;
no PostgreSQL server was involved.

## Red cargo line on the previous tip

On `d1e0e3d`, the tip before this fix (the tests of this commit, the old source):

| Check | Result on `d1e0e3d` |
|---|---|
| `cargo test --locked --no-fail-fast` | **failed**: 110 passed, 16 failed (own target directory) |

The 16 red tests: the 4 new `--datadir` tests (`/nonexistent`, empty directory,
no `pg_wal`, no `PG_VERSION`) and the changed `wal_dir` test; the new
fake-`psql` tests for B-2 (tab and line break in a gid, a user name and an
`archive_command`, broken CSV, `--csv` in the arguments); and 6 older tests
whose `psql` stand-in now prints CSV, which the old tab parser cannot read.
On this commit all 130 are green.

The Windows check is a compile check only. The Unix-only tests (symlinks, file
modes, `sh` stand-ins; `tests/wal_dir.rs`, `tests/binary_e2e.rs`, part of
`tests/cli.rs`) run on Unix only and are left out on Windows.

## Not run

- GitHub Actions: not run. The workflow in `.github/workflows/ci.yml` has not
  run for this commit, so nothing is claimed about a Windows or macOS run. The
  Windows result above is `cargo check` for a target, not a test run.
- `cargo audit`: not run (it needs the network to fetch the advisory database).
- Build with Rust 1.85 (the declared minimum): not run.
- `psql --csv` against a real PostgreSQL server: not run. The CSV parser and
  the arguments are tested with a stand-in that prints CSV by hand; the exact
  bytes of a real `psql` 12+ were not compared.
- Any run against a real PostgreSQL server: not run. The new
  `failed_after_archived` query is hand-written and untested on a server.
