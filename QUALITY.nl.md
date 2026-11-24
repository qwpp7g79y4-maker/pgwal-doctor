# Kwaliteit

[English](QUALITY.md) | Nederlands

Gemeten op 2026-11-24, Linux x86_64, Rust 1.99.0.

## Geteste commit

Codecommit `8bd800384010bf3b3cee5926d3f7c925d83e4d61`, de laatste commit die
iets anders wijzigt dan documenten. Commits daarna wijzigen alleen documenten
(README, CHANGELOG, QUALITY, CONTRIBUTING, KEUR-NVT).

## Resultaten

| Controle | Resultaat |
|---|---|
| `cargo test --locked` | 130 geslaagd, 0 mislukt |
| `cargo fmt --all -- --check` | schoon |
| `cargo clippy --all-targets --locked -- -D warnings` | geen waarschuwingen |
| `cargo clippy --tests --locked --target x86_64-pc-windows-gnu -- -D warnings` | geen waarschuwingen |
| `cargo check --tests --locked --target x86_64-pc-windows-gnu` | ok |
| `cargo deny check licenses` | `licenses ok` |

De 130 tests zijn 34 unit-tests en 96 integratietests, opgeteld over alle
resultaatregels van `cargo test`. Ze gebruiken met de hand geschreven fixtures
en een nep-`psql`; er was geen PostgreSQL-server bij betrokken.

## Rode cargo-regel op de vorige tip

Op `d1e0e3d`, de tip vóór deze herstelling (de tests van deze commit, de oude bron):

| Controle | Resultaat op `d1e0e3d` |
|---|---|
| `cargo test --locked --no-fail-fast` | **mislukt**: 110 geslaagd, 16 mislukt (eigen target-map) |

De 16 rode tests: de 4 nieuwe `--datadir`-tests (`/nonexistent`, lege map,
geen `pg_wal`, geen `PG_VERSION`) en de aangepaste `wal_dir`-test; de nieuwe
nep-`psql`-tests voor B-2 (tab en regeleinde in een gid, een gebruikersnaam en
een `archive_command`, kapotte CSV, `--csv` in de argumenten); en 6 oudere
tests waarvan de `psql`-nepper nu CSV uitgeeft, wat de oude tabparser niet kan
lezen. Op deze commit zijn alle 130 groen.

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
- `psql --csv` tegen een echte PostgreSQL-server: niet uitgevoerd. De
  CSV-parser en de argumenten zijn getest met een nepper die met de hand CSV
  uitgeeft; de exacte bytes van een echte `psql` 12+ zijn niet vergeleken.
- Een run tegen een echte PostgreSQL-server: niet uitgevoerd. De nieuwe query
  met `failed_after_archived` is met de hand geschreven en niet op een server
  geprobeerd.
