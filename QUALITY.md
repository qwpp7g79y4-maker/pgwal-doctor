# Quality

English | [Nederlands](QUALITY.nl.md)

Measured on 2026-11-24, Linux x86_64, Rust 1.99.0.

## Tested commit

Code commit `1d48ba1d282e2d4d883839ceb6ad2627d8447dcd`, the last commit that
changes anything other than documents. Commits after it change only
documents (README, CHANGELOG, QUALITY, CONTRIBUTING, KEUR-NVT).

## Results

| Check | Result |
|---|---|
| `cargo test` | 107 passed, 0 failed |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --all-targets -- -D warnings` | no warnings |
| `cargo deny check licenses` | `licenses ok` |

The 107 tests are 27 unit tests and 80 integration tests, summed over all
`cargo test` result lines. They use hand-written fixtures and a fake `psql`;
no PostgreSQL server was involved.

## Not run

- GitHub Actions: not run. The workflow in `.github/workflows/ci.yml` has not
  run for this commit, so nothing is claimed about Windows or macOS.
- `cargo audit`: not run (it needs the network to fetch the advisory database).
- Build with Rust 1.85 (the declared minimum): not run.
- Any run against a real PostgreSQL server: not run.
