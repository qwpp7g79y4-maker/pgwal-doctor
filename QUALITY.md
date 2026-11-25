# Quality

English | [Nederlands](QUALITY.nl.md)

Measured on 2026-11-25, Linux x86_64, Rust 1.99.0. Linux only.

## Tested commit

Code commit `8f4980a9784487336b30a68318abe54a34b462e0`, the last commit that
changes anything other than documents. Commits after it change only
documents (README, CHANGELOG, QUALITY, CONTRIBUTING, KEUR-NVT).

## Results

The four checks ran in this order, each stopping the run on the first failure.
All four exited 0.

| Check | Exit code | Result |
|---|---|---|
| `cargo fmt --check` | 0 | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | 0 | no warnings |
| `cargo test --locked --no-fail-fast` | 0 | 130 passed, 0 failed |
| `cargo test --release --locked --no-fail-fast` | 0 | 130 passed, 0 failed |

The 130 tests are 34 unit tests and 96 integration tests, summed over all
`cargo test` result lines (the same in the debug and the release run). They
use hand-written fixtures and a fake `psql`; no PostgreSQL server was
involved.

## Not run

- GitHub Actions: GitHub run 45630158383 on 8f4980a ended red (Ubuntu:
  closed_stdout_pipe_exits_2_without_panic; macOS: non_utf8_file_name_does_not_panic).
  This commit has not been through CI. Nothing is claimed
  about a macOS or Windows run, a test run or a compile check.
- Windows and macOS: not measured on this commit.
- `cargo deny check licenses`: not run on this commit.
- `cargo audit`: not run (it needs the network to fetch the advisory database).
- Build with Rust 1.85 (the declared minimum): not run.
- `psql --csv` against a real PostgreSQL server: not run. The CSV parser and
  the arguments are tested with a stand-in that prints CSV by hand; the exact
  bytes of a real `psql` 12+ were not compared.
- Any run against a real PostgreSQL server: not run. The
  `failed_after_archived` query is hand-written and untested on a server.
