# Quality

English | [Nederlands](QUALITY.nl.md)

Measured on 2026-11-24, Linux x86_64, Rust 1.99.0.

## Tested commit

Code commit `5f23ba13285809bce008c21dd0d079b37d422dfd`, the last commit that
changes anything other than documents. Commits after it change only
documents (README, CHANGELOG, QUALITY, CONTRIBUTING, KEUR-NVT).

## Results

| Check | Result |
|---|---|
| `cargo test --locked` | 116 passed, 0 failed |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | no warnings |
| `cargo clippy --tests --locked --target x86_64-pc-windows-gnu -- -D warnings` | no warnings |
| `cargo check --tests --locked --target x86_64-pc-windows-gnu` | ok |
| `cargo deny check licenses` | `licenses ok` |

The 116 tests are 30 unit tests and 86 integration tests, summed over all
`cargo test` result lines. They use hand-written fixtures and a fake `psql`;
no PostgreSQL server was involved.

## Red cargo line on the previous tip

On `12be42a`, the commit before the fixes:

| Check | Result on `12be42a` |
|---|---|
| `cargo check --tests --locked --target x86_64-pc-windows-gnu` | **failed**: 10 test targets did not compile (`cannot find unix in os`, `from_mode`, `from_vec`) |
| `cargo test --locked --no-fail-fast`, with the new tests and the old source | **failed**: 3 archiver tests (German date, DST hour, no server comparison) and 3 binary tests (full stdout, closed pipe, encoded password key) |

The Windows check is a compile check only. The Unix-only tests (symlinks, file
modes, `sh` stand-ins; `tests/wal_dir.rs`, `tests/binary_e2e.rs`, part of
`tests/cli.rs`) run on Unix only and are left out on Windows.

## Not run

- GitHub Actions: not run. The workflow in `.github/workflows/ci.yml` has not
  run for this commit, so nothing is claimed about a Windows or macOS run. The
  Windows result above is `cargo check` for a target, not a test run.
- `cargo audit`: not run (it needs the network to fetch the advisory database).
- Build with Rust 1.85 (the declared minimum): not run.
- Any run against a real PostgreSQL server: not run. The new
  `failed_after_archived` query is hand-written and untested on a server.
