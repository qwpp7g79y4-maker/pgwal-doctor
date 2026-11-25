# Quality

English | [Nederlands](QUALITY.nl.md)

Measured on 2026-11-25, Linux x86_64, Rust 1.99.0. Linux only.

## Tested commit

Code commit `d7100436f5a40578ef8c5f90c2a6b5acecdeff47`, the last commit that
changes anything other than documents. Commits after it change only
documents (README, CHANGELOG, QUALITY, CONTRIBUTING, KEUR-NVT).

## Results

The four checks ran in this order. All four exited 0.

| Check | Exit code | Result |
|---|---|---|
| `cargo fmt --check` | 0 | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | 0 | no warnings |
| `cargo test --locked` | 0 | 131 passed, 0 failed |
| `cargo test --release --locked` | 0 | 131 passed, 0 failed |

Rust 1.85.1 (rustc 4eb161250, 2025-03-15), measured on 2026-11-25 on the same
code commit, Linux x86_64. The three checks ran in this order and all exited 0.

| Check | Exit code | Result |
|---|---|---|
| `cargo +1.85 fmt --check` | 0 | clean |
| `cargo +1.85 clippy --all-targets --locked -- -D warnings` | 0 | no warnings |
| `cargo +1.85 test --locked` | 0 | 131 passed, 0 failed |

On 1.85, `cargo test --release --locked` was not run.

The 131 tests are 34 unit tests and 97 integration tests, summed over all
`cargo test` result lines. They
use hand-written fixtures and a fake `psql`; no PostgreSQL server was
involved.

## Flaky test

`binary_e2e` failed at random (for example
`tab_and_newline_in_a_prepared_gid_do_not_forge_a_row` and
`datadir_and_online_together_use_the_stand_in`) with `Text file busy (os error
26)` when it started the fake `psql`. The cause is in the test helper, not in
the tool: one test thread wrote the stand-in script while another thread
forked a child that inherited the open write handle, so Linux refused to run
the script. `write_exe` and every child the file starts now share one lock.
With 24 busy loops running next to the tests, the old code failed in about 25
of 400 runs of `binary_e2e` (32 threads); the new code failed in 0 of 800.
Linux only; macOS and Windows were not tried.

## Not run

- macOS: not run. This commit gates `status_locked` and the `ExitStatus`
  import with `cfg(not(target_os = "macos"))` so clippy should stop reporting
  dead code there (CI 45674968685, macos-15-intel); that was not checked on
  macOS.

- GitHub Actions: not run on this commit. Nothing is claimed about a macOS or
  Windows run, a test run or a compile check.
- Windows and macOS: not measured on this commit.
- `cargo deny check licenses`: not run on this commit.
- `cargo audit`: not run (it needs the network to fetch the advisory database).
- Rust 1.85 `cargo test --release --locked`: not run. Rust 1.85 fmt, clippy and
  debug test: see Results.
- `psql --csv` against a real PostgreSQL server: not run. The CSV parser and
  the arguments are tested with a stand-in that prints CSV by hand; the exact
  bytes of a real `psql` 12+ were not compared.
- Any run against a real PostgreSQL server: not run. The
  `failed_after_archived` query is hand-written and untested on a server.
