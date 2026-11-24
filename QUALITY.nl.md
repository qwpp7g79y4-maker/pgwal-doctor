# Kwaliteit

[English](QUALITY.md) | Nederlands

Gemeten op 2026-11-24, Linux x86_64, Rust 1.99.0.

## Geteste commit

Codecommit `1d48ba1d282e2d4d883839ceb6ad2627d8447dcd`, de laatste commit die
iets anders wijzigt dan documenten. Commits daarna wijzigen alleen documenten
(README, CHANGELOG, QUALITY, CONTRIBUTING, KEUR-NVT).

## Resultaten

| Controle | Resultaat |
|---|---|
| `cargo test` | 107 geslaagd, 0 mislukt |
| `cargo fmt --all -- --check` | schoon |
| `cargo clippy --all-targets -- -D warnings` | geen waarschuwingen |
| `cargo deny check licenses` | `licenses ok` |

De 107 tests zijn 27 unit-tests en 80 integratietests, opgeteld over alle
resultaatregels van `cargo test`. Ze gebruiken met de hand geschreven fixtures
en een nep-`psql`; er was geen PostgreSQL-server bij betrokken.

## Niet uitgevoerd

- GitHub Actions: niet uitgevoerd. De workflow in `.github/workflows/ci.yml`
  is voor deze commit niet gedraaid, dus er wordt niets beweerd over Windows
  of macOS.
- `cargo audit`: niet uitgevoerd (het heeft netwerk nodig voor de
  advisory-database).
- Bouwen met Rust 1.85 (het opgegeven minimum): niet uitgevoerd.
- Een run tegen een echte PostgreSQL-server: niet uitgevoerd.
