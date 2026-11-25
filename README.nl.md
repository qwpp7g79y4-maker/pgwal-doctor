# pgwal-doctor

[English](README.md) | Nederlands

Alleen-lezen rapport over waarom `pg_wal` van PostgreSQL blijft groeien. De
tool kijkt in één keer naar de gebruikelijke oorzaken en geeft één rapport,
zodat je niet zes views met de hand hoeft te bevragen.

Status: versie 0.1.0, nog niet uitgebracht (`publish = false`).
Man-pagina: [man/pgwal-doctor.1](man/pgwal-doctor.1).

## Wat het controleert

Offline, vanuit de datadir (`--datadir`):

- **wal_dir** — aantal en totale grootte van WAL-segmenten in
  `<datadir>/pg_wal`; meldt als `pg_wal` een symlink is.
- **archive_backlog** — `.ready`- en `.done`-bestanden in
  `pg_wal/archive_status`.
- **pg_replslot** — aantal slotmappen. Offline kan een slot niet als oorzaak
  worden bevestigd: `pg_replslot/<naam>/state` is binair en wordt niet gelezen.

Online, via `psql` (`--dsn` of `--online`):

- **slots** — inactieve slots en hoe ver `restart_lsn` achterloopt,
  `wal_status` (`extended`, `unreserved`, `lost`), `max_slot_wal_keep_size`.
- **archiver** — falen in `pg_stat_archiver`, plus `archive_mode`,
  `archive_command` en `archive_library` uit `pg_settings`.
- **settings** — `max_wal_size`, `min_wal_size`, `wal_keep_size`
  (`wal_keep_segments` op PG 12), `max_slot_wal_keep_size`,
  `checkpoint_timeout`, `wal_segment_size`, omgerekend naar bytes.
- **transactions** en **prepared_xacts** — leeftijd van de oudste open en
  voorbereide transactie. Alleen context (zie "Aannames").
- **waldir** — grootte van `pg_wal` via `pg_ls_waldir()`.
- **summary** — rangschikt slot-vastgehouden WAL, archief-backlog en
  `wal_keep_size` naast de mapgrootte. De oorzaken overlappen, dus ze hoeven
  niet op te tellen; de rest staat als "other or unknown", nooit verzonnen.

## Wat het NIET doet

- Het schrijft nooit naar PostgreSQL of naar de datadir. Geen `DROP`, geen
  `CHECKPOINT`, geen `pg_switch_wal`, geen `COMMIT PREPARED`, geen bestanden
  wissen.
- Het raadt nooit aan om bestanden uit `pg_wal` te verwijderen.
- Het leest `postgresql.conf` niet (online komen de instellingen uit
  `pg_settings`).
- Het leest `pg_replslot/*/state` niet.
- Offline maakt het geen netwerkverbinding. Online wel, via `psql`.

## Installatie

Er is geen crates.io-release (`publish = false`). Bouw de tool vanuit een
kloon van de repository, met Rust 1.85 of nieuwer:

```sh
git clone https://github.com/qwpp7g79y4-maker/pgwal-doctor
cd pgwal-doctor
cargo install --path .
```

Of draai `cargo build --release` en gebruik `target/release/pgwal-doctor`.
Online heeft ook `psql` (versie 12 of nieuwer) nodig op `PATH`.

## Snelstart

```sh
pgwal-doctor --datadir /var/lib/postgresql/17/main    # offline, leest alleen bestanden
pgwal-doctor --online                                 # online, libpq-omgeving
```

## Aan de slag in 5 minuten

1. Bouw de tool (zie Installatie).
2. Probeer hem op de verzonnen datadir in deze repository. Hij leest een paar
   verzonnen bestanden, maakt geen netwerkverbinding en schrijft niets:
   `pgwal-doctor --datadir examples/quiet-datadir`. De exitcode is `0`.
3. Wijs `--datadir` naar je eigen datadir, als gebruiker `postgres`.
4. Maak voor de online checks de inlogrol onder "Rechten" en voeg
   `--dsn 'host=db1 user=pgwal_doctor dbname=postgres'` toe.
5. Lees de regel `hint:` onder elke bevinding. Drempels zijn aannames, geen
   aanbevelingen (zie "Aannames en onbekenden").

## Gebruik

```sh
pgwal-doctor --datadir /var/lib/postgresql/17/main            # offline
pgwal-doctor --online                                         # libpq-omgeving
pgwal-doctor --dsn 'host=db1 user=pgwal_doctor dbname=postgres'
pgwal-doctor --datadir /pad --dsn 'service=prod' --json       # beide, JSON
pgwal-doctor --help                                           # gebruik, exitcode 0
pgwal-doctor --version                                        # versie, exitcode 0
```

Online heeft `psql` (versie 12 of nieuwer, het draait met `--csv`) nodig op
`PATH` (of zet `PGWAL_DOCTOR_PSQL`).
Verbindingsinstellingen komen uit `--dsn` of de gewone `PG*`-variabelen.
Online start alleen bij `--dsn` of `--online`.

Een DSN met wachtwoord wordt geweigerd, zodat het wachtwoord niet in de
procestabel staat. De sleutels van een URI-query worden eerst van procenten
ontdaan, dus ook `?pass%77ord=x` wordt geweigerd. Verbindingsfouten worden
zonder DSN getoond.

## Voorbeeld

Draai vanuit de hoofdmap van de repository; de datadir is verzonnen (zie
[examples/README.md](examples/README.md)):

```sh
pgwal-doctor --datadir examples/quiet-datadir
```

Uitvoer, ingekort (het volledige rapport heeft onder elke bevinding een regel
`hint:`):

```text
pgwal-doctor

[Ok] summary bytes=47 count=-
evidence: pg_wal holds 47 bytes. archive backlog (.ready): 0 bytes; other or unknown: 47 bytes. ...

[Ok] wal_dir bytes=47 count=1
[Ok] archive_backlog bytes=0 count=0
[Ok] pg_replslot bytes=- count=0
```

De exitcode is `0`.

## Afsluitcodes

| Code | Betekenis |
|---|---|
| `0` | Niets om op te handelen. |
| `1` | Een check meldde `Warn`, `NeedsRoot` of `Error`. Een mislukte verbinding is een `Error`, en een `--datadir` die geen datadirectory is (geen `PG_VERSION` of geen `pg_wal`) ook. |
| `2` | Gebruiksfout. |

## Rechten

Maak een inlogrol zonder superuser:

```sql
CREATE ROLE pgwal_doctor LOGIN;
GRANT pg_monitor TO pgwal_doctor;
```

`pg_monitor` is lid van `pg_read_all_settings`, `pg_read_all_stats` en
`pg_stat_scan_tables` (PostgreSQL 17,
[predefined roles](https://www.postgresql.org/docs/17/predefined-roles.html)).
`pg_ls_waldir()` is beperkt tot superusers en leden van `pg_monitor`, tenzij
`EXECUTE` is verleend
([functions-admin](https://www.postgresql.org/docs/17/functions-admin.html)).
Zonder recht meldt een check `NeedsRoot` met een hint en draaien de andere
checks door. Offline is leesrecht op `pg_wal` nodig (meestal de gebruiker
`postgres`).

## Versieverschillen

| Feit | Status | Bron |
|---|---|---|
| PG 12 heeft `wal_keep_segments`, geen `wal_keep_size`, geen `max_slot_wal_keep_size` | bevestigd | [12-docs](https://www.postgresql.org/docs/12/runtime-config-replication.html) |
| PG 12 `pg_replication_slots` heeft geen `wal_status` / `safe_wal_size` | bevestigd | [12-docs](https://www.postgresql.org/docs/12/view-pg-replication-slots.html) |
| PG 13 hernoemt `wal_keep_segments` naar `wal_keep_size` en voegt `max_slot_wal_keep_size` toe | bevestigd | [13.0-notes](https://www.postgresql.org/docs/release/13.0/) |
| `wal_status` / `safe_wal_size` komen in PG 13 | **aanname** | 13.0-notes noemen de kolommen niet |
| PG 15 voegt `archive_library` toe | bevestigd | [15.0-notes](https://www.postgresql.org/docs/release/15.0/) |
| PG 17 voegt `inactive_since` en `invalidation_reason` toe | bevestigd | [17.0-notes](https://www.postgresql.org/docs/release/17.0/) |
| Wanneer de kolom `conflicting` is ingevoerd | **onbekend** | niet nagekeken |
| Verschillen in PG 14 en 16 voor `pg_stat_archiver`, `pg_ls_waldir`, rechten | **onbekend** | niet nagekeken |
| PG 18 voegt `idle_replication_slot_timeout` toe (de tool gebruikt het niet) | bevestigd | [18-docs](https://www.postgresql.org/docs/18/runtime-config-replication.html) |

De tool leest kolommen op naam. Een kolom die de server mist wordt gemeld als
"not available on this server version", niet als fout.

## Aannames en onbekenden

Deze beweringen zijn niet gecontroleerd tegen PostgreSQL-broncode of een
docs-pagina. De tool en deze README noemen ze aannames.

- **Drempels zijn aannames**: 100 `.ready`-bestanden, een inactief slot dat
  1 GiB vasthoudt, een transactie ouder dan 1 uur. Het zijn geen adviezen.
- `.ready` betekent "wacht op archivering" en `.done` "gearchiveerd" in
  `pg_wal/archive_status`. De gelezen docs-pagina's noemen alleen de map,
  via `pg_ls_archive_statusdir()`.
- WAL-segmentnamen zijn 24 hexcijfers; `*.history`, `*.partial` en `*.backup`
  zijn geen segmenten.
- De standaard segmentgrootte is 16 MB (alleen gebruikt als de server
  `wal_segment_size` niet geeft, voor `wal_keep_segments` op PG 12).
- `pg_wal` kan een symlink zijn (`initdb --waldir`). De storage-layout-pagina
  zegt dat niet.
- Een alleen-lezen sessie via `PGOPTIONS` (`default_transaction_read_only=on`)
  werkt zoals bedoeld.
- SQLSTATE `42501` betekent een ontbrekend recht.
- Op een standby faalt `pg_current_wal_lsn()`; de tool ondersteunt standby's
  nog niet. Zie resultaten van een standby als ongetest.
- **Voorbereide en lange transacties:** de PostgreSQL-docs zeggen dat
  voorbereide transacties locks vasthouden en VACUUM hinderen. Ze zeggen niet
  dat ze WAL vasthouden. Dat bloat door zulke transacties meer WAL oplevert is
  een aanname, dus de tool toont ze als context, niet als oorzaak.
- Dat trage of mislukte checkpoints het opruimen van oud WAL vertragen is een
  aanname.
- Hoe `wal_level`, `archive_timeout` of een trage standby de groei
  beïnvloeden is **onbekend**; de tool controleert ze niet.
- Of de laatste archiverfout nieuwer is dan het laatst gearchiveerde WAL,
  beslist de server, die de twee `timestamptz`-waarden vergelijkt
  (`failed_after_archived`). De tool vergelijkt nooit de getoonde tekst, die
  van `DateStyle` en de tijdzone afhangt. Deze query is met de hand geschreven
  en niet tegen een echte server gedraaid. Geeft de server geen vergelijking
  en zijn er mislukkingen geteld, dan is de bevinding een waarschuwing die dat
  zegt.
- `pg_stat_archiver` kan schoon zijn terwijl archiveren faalt: als het
  commando door een signaal eindigt of een exitcode boven 125 geeft, herstart
  de archiver en wordt het falen niet geteld
  ([continuous archiving](https://www.postgresql.org/docs/17/continuous-archiving.html)).
  Een schoon resultaat is dus nooit bewijs; de tool combineert het met de
  `.ready`-backlog als ook `--datadir` is opgegeven. Of dit geval in fixtures
  te reproduceren is, is onbekend.

Bronnen voor de oorzaken: [WAL configuration](https://www.postgresql.org/docs/17/wal-configuration.html),
[replication slots](https://www.postgresql.org/docs/17/view-pg-replication-slots.html),
[continuous archiving](https://www.postgresql.org/docs/17/continuous-archiving.html),
[replication settings](https://www.postgresql.org/docs/17/runtime-config-replication.html).
De pagina's zijn via een samenvattende stap gelezen; controleer een citaat
tegen de pagina voordat je erop vertrouwt.

## Onderhoud

pgwal-doctor is een klein project zonder releaseschema. Meld bugs en vragen
als GitHub Issues; zie [CONTRIBUTING.md](CONTRIBUTING.md). Wijzigingen staan
in [CHANGELOG.md](CHANGELOG.md), beveiligingsmeldingen volgen
[SECURITY.md](SECURITY.md), en wat getest is, en wat niet, staat in
[QUALITY.nl.md](QUALITY.nl.md).

## Tests

De tests hebben geen PostgreSQL nodig: query-uitvoer komt uit bestanden in
`tests/fixtures/pg/`, en een nep-`psql`-script dekt de commandoregel. De
fixtures zijn met de hand geschreven uit de gedocumenteerde kolomlijsten, niet
opgenomen van echte servers (zie `tests/fixtures/pg/README.md`).

## Licentie

Onder een van beide licenties naar keuze: [MIT](LICENSE-MIT) of
[Apache-2.0](LICENSE-APACHE).

De licenties van de externe crates in het programma staan in [THIRD-PARTY-LICENSES.html](THIRD-PARTY-LICENSES.html).
