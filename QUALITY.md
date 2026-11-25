# Quality

English | [Nederlands](QUALITY.nl.md)

Measured on 2026-11-25, Linux x86_64, Rust 1.99.0. Linux only.

## Tested commit

Code commit `b04083159d1da423c0d9f45d288fcdc41b3d23b3`, the last commit that
changes anything other than documents. Commits after it change only
documents (README, CHANGELOG, QUALITY, CONTRIBUTING, KEUR-NVT).

## Results

The three checks ran in this order. All three exited 0.

| Check | Exit code | Result |
|---|---|---|
| `cargo fmt --check` | 0 | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | 0 | no warnings |
| `cargo test --locked` | 0 | 130 passed, 0 failed |

The 130 tests are 34 unit tests and 96 integration tests, summed over all
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

- `cargo test --release`: not run on this commit.
- macOS: not run. This commit gates `status_locked` and the `ExitStatus`
  import with `cfg(not(target_os = "macos"))` so clippy should stop reporting
  dead code there (CI 45674968685, macos-15-intel); that was not checked on
  macOS.

- GitHub Actions: not run on this commit. Nothing is claimed about a macOS or
  Windows run, a test run or a compile check.
- Windows and macOS: not measured on this commit.
- `cargo deny check licenses`: not run on this commit.
- `cargo audit`: not run (it needs the network to fetch the advisory database).
- Build with Rust 1.85 (the declared minimum): not run.
- `psql --csv` against a real PostgreSQL server: not run. The CSV parser and
  the arguments are tested with a stand-in that prints CSV by hand; the exact
  bytes of a real `psql` 12+ were not compared.
- Any run against a real PostgreSQL server: not run. The
  `failed_after_archived` query is hand-written and untested on a server.
