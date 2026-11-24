# Kwaliteit

[English](QUALITY.md) | Nederlands

Gemeten op 2026-11-24, Linux x86_64, Rust 1.99.0.

## Geteste commit

Codecommit `5f23ba13285809bce008c21dd0d079b37d422dfd`, de laatste commit die
iets anders wijzigt dan documenten. Commits daarna wijzigen alleen documenten
(README, CHANGELOG, QUALITY, CONTRIBUTING, KEUR-NVT).

## Resultaten

| Controle | Resultaat |
|---|---|
| `cargo test --locked` | 116 geslaagd, 0 mislukt |
| `cargo fmt --all -- --check` | schoon |
| `cargo clippy --all-targets --locked -- -D warnings` | geen waarschuwingen |
| `cargo clippy --tests --locked --target x86_64-pc-windows-gnu -- -D warnings` | geen waarschuwingen |
| `cargo check --tests --locked --target x86_64-pc-windows-gnu` | ok |
| `cargo deny check licenses` | `licenses ok` |

De 116 tests zijn 30 unit-tests en 86 integratietests, opgeteld over alle
resultaatregels van `cargo test`. Ze gebruiken met de hand geschreven fixtures
en een nep-`psql`; er was geen PostgreSQL-server bij betrokken.

## Rode cargo-regel op de vorige tip

Op `12be42a`, de commit vóór de herstellingen:

| Controle | Resultaat op `12be42a` |
|---|---|
| `cargo check --tests --locked --target x86_64-pc-windows-gnu` | **mislukt**: 10 testdoelen compileerden niet (`cannot find unix in os`, `from_mode`, `from_vec`) |
| `cargo test --locked --no-fail-fast`, met de nieuwe tests en de oude bron | **mislukt**: 3 archivertests (Duitse datum, zomertijdgrens, geen vergelijking van de server) en 3 binarytests (volle stdout, dichte pijp, gecodeerde wachtwoordsleutel) |

De Windows-controle is alleen een compileercontrole. De unix-tests (symlinks,
bestandsrechten, `sh`-nepprogramma's; `tests/wal_dir.rs`,
`tests/binary_e2e.rs`, een deel van `tests/cli.rs`) draaien alleen op Unix en
blijven op Windows weg.

## Niet uitgevoerd

- GitHub Actions: niet uitgevoerd. De workflow in `.github/workflows/ci.yml`
  is voor deze commit niet gedraaid, dus er wordt niets beweerd over een
  Windows- of macOS-run. Het Windows-resultaat hierboven is `cargo check` voor
  een doelplatform, geen testrun.
- `cargo audit`: niet uitgevoerd (het heeft netwerk nodig voor de
  advisory-database).
- Bouwen met Rust 1.85 (het opgegeven minimum): niet uitgevoerd.
- Een run tegen een echte PostgreSQL-server: niet uitgevoerd. De nieuwe query
  met `failed_after_archived` is met de hand geschreven en niet op een server
  geprobeerd.
