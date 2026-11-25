# Kwaliteit

[English](QUALITY.md) | Nederlands

Gemeten op 2026-11-25, Linux x86_64, Rust 1.99.0. Alleen Linux.

## Geteste commit

Codecommit `b04083159d1da423c0d9f45d288fcdc41b3d23b3`, de laatste commit die
iets anders wijzigt dan documenten. Commits daarna wijzigen alleen documenten
(README, CHANGELOG, QUALITY, CONTRIBUTING, KEUR-NVT).

## Resultaten

De drie controles liepen in deze volgorde. Alle drie eindigden met exitcode 0.

| Controle | Exitcode | Resultaat |
|---|---|---|
| `cargo fmt --check` | 0 | schoon |
| `cargo clippy --all-targets --locked -- -D warnings` | 0 | geen waarschuwingen |
| `cargo test --locked` | 0 | 130 geslaagd, 0 mislukt |

De 130 tests zijn 34 unit-tests en 96 integratietests, opgeteld over alle
resultaatregels van `cargo test`. Ze
gebruiken met de hand geschreven fixtures en een nep-`psql`; er was geen
PostgreSQL-server bij betrokken.

## Wisselende test

`binary_e2e` faalde willekeurig (bijvoorbeeld
`tab_and_newline_in_a_prepared_gid_do_not_forge_a_row` en
`datadir_and_online_together_use_the_stand_in`) met `Text file busy (os error
26)` bij het starten van de nep-`psql`. De oorzaak zit in het testhulpje, niet
in de tool: een testthread schreef het nepscript terwijl een andere thread een
kindproces forkte dat de open schrijfhandle erfde, waardoor Linux het script
niet wilde starten. `write_exe` en elk kind dat dit bestand start delen nu
een slot. Met 24 drukke lussen naast de tests faalde de oude code in ongeveer
25 van 400 runs van `binary_e2e` (32 threads); de nieuwe code in 0 van 800.
Alleen Linux; macOS en Windows zijn niet geprobeerd.

## Niet uitgevoerd

- `cargo test --release`: niet uitgevoerd op deze commit.
- macOS: niet uitgevoerd. Deze commit zet `cfg(not(target_os = "macos"))` op
  `status_locked` en de `ExitStatus`-import, zodat clippy daar geen dode code
  meer meldt (CI 45674968685, macos-15-intel); dat is niet op macOS gecontroleerd.

- GitHub Actions: niet uitgevoerd op deze commit. Er wordt niets beweerd over
  een macOS- of Windows-run, een testrun of een compileercontrole.
- Windows en macOS: niet gemeten op deze commit.
- `cargo deny check licenses`: niet uitgevoerd op deze commit.
- `cargo audit`: niet uitgevoerd (het heeft netwerk nodig voor de
  advisory-database).
- Bouwen met Rust 1.85 (het opgegeven minimum): niet uitgevoerd.
- `psql --csv` tegen een echte PostgreSQL-server: niet uitgevoerd. De
  CSV-parser en de argumenten zijn getest met een nepper die met de hand CSV
  uitgeeft; de exacte bytes van een echte `psql` 12+ zijn niet vergeleken.
- Een run tegen een echte PostgreSQL-server: niet uitgevoerd. De query met
  `failed_after_archived` is met de hand geschreven en niet op een server
  geprobeerd.
