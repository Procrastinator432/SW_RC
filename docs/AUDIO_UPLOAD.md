# AL-Upload und PlaySource-Befehlsfolge

**Nachfolgender interner Block:** [FALSource-Steuerung und Abschlussregeln](AUDIO_DRIVER.md) rekonstruiert die echten virtuellen Methoden als separaten Kern. Die hier dokumentierte Upload-Session verwendet weiterhin ihren belegten Basis-Play-/Pause-Vertrag; die Zusammenführung ist noch offen.

Stand: 24.10.2026. Fünf weitere **interne** Pakete nach [Aktivierungsvorbereitung](AUDIO_ACTIVATION.md). Die neue `audio_upload::Session` verbindet den vorhandenen Quellenpool mit dem restlichen `PlaySource`-Befehlsablauf. Sie erzeugt geordnete Aufträge und einen gespeicherten Folgezustand. Sie öffnet kein Audiogerät und lädt keine PCM-Daten hoch. Spielbare Referenz bleibt **0.60.0**.

## Die fünf Pakete

| Paket | Umgesetzter Vertrag | Originalnachweis |
|---|---|---|
| 1 · Grundparameter | Params-/Kategorieabfrage, Kategorie-Gain mit Originalgrenzen, Pitch, Looping und Rolloff samt Diagnosepunkten in Originalreihenfolge. | Gesamter AL-PlaySource-Aufruf; Vergleich aller Argumentbits und Aufträge. |
| 2 · Räumlicher Upload | Position in Reihenfolge X/Z/Y mit Faktor 0,01, aktueller Radius oder Listenerentfernung bei Flag `0x1000`, Relative-/Editor-/Nichtpositionszweig. | Originales `RSQRTSS` samt verfeinertem Radius; dessen CPU-Seed ist eine explizite Eingabe. Der gewöhnliche Koordinatenkern aus 0.59.0 wird damit erweitert, nicht erneut als neue Umrechnung gezählt. |
| 3 · EAX | Versionswahl 2/3, Radiusverhältnis und logarithmische Integerdämpfung, Flag-Ausnahmen, zweite Eigenschaft und gespeicherter Lazy-Cache für lokale Quellen. | Originale x87-Logarithmus-/Integerinstruktionen; zusätzlich 2.048 binäre32-Verhältnisse direkt neben Integergrenzen. |
| 4 · Pufferbindung | Beide Fortschrittswörter nullen, vorherigen Buffer lösen, einen Buffer direkt binden oder 0/mehrere Buffer in Reihenfolge einreihen; im Queuezweig Looping wieder ausschalten. | Synthetische Bufferlisten mit 0–4 Einträgen und unterschiedlichen oberen Count-Tagbits durch Originalinstruktionen. Keine neuen PCM-/Streamdaten beschafft. |
| 5 · Persistente Komposition | Aufnahme samt Doppelstop/Transient-Tausch, Upload, virtueller Basis-Play-Aufruf, Lebensdauerwort-Reset und gegebenenfalls anschließende Basispause; gemeinsamer Zustand für Quellenpool, Bufferfortschritt und EAX-Cache. | Ganze originale `PlaySource`-Aufrufe gegen vollständige Auftrags-/Zustandsfolge, kalte Snapshotübernahme und atomare Abweisung später Fehler. |

Code: [audio_upload.rs](../crates/rc-package/src/audio_upload.rs), [Tests](../crates/rc-package/src/audio_upload_tests.rs), [Originalprüfer](../scripts/Test-AudioUploadOriginal.py), eingefrorene synthetische Fälle [audio-upload-native.json](../fixtures/audio-upload-native.json). Originalbinärbytes bleiben ausschließlich in der lokalen Installation.

## Präzise Originalbefunde

`alaudio.dll` SHA-256 `60e0c8495a94b297c36bc96f29e4c64a208f020e1ac55f5d6848d5697cff0dcd`; zusätzlich die bereits fixierten Engine-/CRT-Hashes im Fixture. Originale Funktion `10008800..10008d6d`; neu rekonstruierter Erfolgsrumpf ab `100088fb`. Der Prüfer läuft vom wirklichen Funktionseintritt bis zur normalen Rückkehr, einschließlich echter Basiszuweisung, `FindSource`, `SetTransientSource`, EAX-Rechnung und Pufferverzweigung. Der zusätzliche EAX-Test läuft separat `10008a33..10008a68` mit frischem Unicorn-Übersetzungscache.

- Gain ist `output_volume * category_gain` in binary32, begrenzt durch die ursprünglichen Wörter `38d1b717` und `3f7ff972` (etwa 0,0001–0,9999). Pitch ist auf binary32 0,7–1,2 begrenzt. Rolloff wird auf 1 gesetzt.
- Positionszweig: EAX-Radiusdämpfung entfällt bei Flag `0x800`; die zweite EAX-Eigenschaft bleibt erhalten. Das Verhältnis wird bei `3727c5ac` (etwa 0,00001) nach unten begrenzt, mit `ln` und dem ursprünglichen binary32-Faktor `445925b2` verrechnet und Richtung null in Integer gewandelt. Version unter 3 nutzt Eigenschaft 9, sonst 11. Eigenschaft 7 erhält bei Flags `0x18` den Wert −10.000, sonst 0.
- Lokal-/Editorzweig: Relative=1, Position null, Referenzdistanz 10. EAX initialisiert seinen lokalen statischen Dämpfungswert einmal auf 0 (`ln(1)`); ein bereits initialisierter Wert bleibt erhalten. Im räumlichen Zweig wird dieser Cache nicht verändert.
- `0x1000` berechnet Entfernung mit **einer anderen Summenreihenfolge** als die laufende Priorität. Nullabstand wird auf null maskiert. Für andere Abstände liefert der Aufrufer den tatsächlichen `RSQRTSS`-Seed; die portable API behauptet keine CPU-unabhängige Rekonstruktion dieser Näherung.
- Die vorgemerkte Pause aus dem Aufnahmepräfix bleibt ein roher Integer. Am Ende wird bei jedem Wert ungleich null trotzdem `Pause(1)` aufgerufen. Das geschieht nach `Play` und nach dem Setzen beider Lebensdauerwörter auf null; der ursprüngliche Pausenwert wird nicht weitergereicht.

## Zustands- und Fehlervertrag

`Session::play` plant zunächst auf einer Kopie. Fehlende Bufferbeschreibung, ungültige Geometrie-/Seedwerte oder ein unzulässiger später EAX-Wert verändern den bisherigen Zustand nicht. Auch Vorbereitungsfehlschläge erhalten die ursprünglichen Präfixänderungen und erzeugen keinen Upload. Die Enginequellen werden nach ihren vorhandenen endlichen Feldgrenzen geprüft; die Session verlangt eindeutige aktive physische Slots und einen separaten Transient-Kanal. Bufferhandles sind eindeutig, maximal 4.096 Buffer pro Beschreibung und 65.536 im ganzen Bankzustand. Opaque Buffer-/Hardwarewerte sind keine echten Treiberressourcen.

Snapshot-Schema 1 speichert Pool, Bufferbeschreibungen und EAX-Cache; unbekannte Felder, falsches Schema, beschädigte Werte oder mehr als 16 MiB Eingabe werden abgewiesen. Dieses Format ist ein eigener interner Zustand und kein App-/Engine-Spielstand. Bereits extern ausgeführte Treiberaktionen ließen sich durch diese reine Zustandstransaktion nicht zurückrollen.

Der EAX-Port nutzt für den Logarithmus binary64 statt einer allgemeinen x87-Emulation. Bestätigt sind die resultierenden Integer für die eingefrorenen Fälle einschließlich 2.048 Grenzproben, keine pauschale Bitparität jedes mathematischen Zwischenwerts oder jeder Plattformbibliothek. Nichtendliche Radiusverhältnisse werden explizit abgewiesen; insbesondere `0/0` und positive Division durch null sind kein unterstützter Uploadvertrag. Positions-/Listenerwerte sind endlich und betragsmäßig höchstens 1.000.000; nichtnullige Distanz verlangt einen endlichen positiven Seed.

## Nachweise und Reproduktion

512 ganze originale Aufrufe, davon 459 mit erfolgreichem Aufnahme-/Uploadpfad, und 2.048 zusätzliche EAX-Grenzproben. Acht neue Rust-Testgruppen vergleichen Ablauf, Floatwörter, EAX-Integer, Quellen-/Buffer-/Cachezustände und gespeicherten Fortlauf. Nach dem Einfrieren wird das Fixture nochmals frisch aus den Originalinstruktionen erzeugt und vollständig verglichen.

```bash
source scripts/env-linux.sh
python scripts/Test-AudioUploadOriginal.py --game-data "$RC_GAME_DATA" \
  --output build/audio-upload/original-replay.json \
  --verify-fixture fixtures/audio-upload-native.json
./build.sh -Target Check -OutputDirectory build/audio-upload/check
```

**Zentralcheck bestanden:** je **1.248 Debug-/Release-Bibliothekstests**, Format, Materialimport-/Dokumentationstests, Clippy mit Warnungen als Fehler, Native-/Inspector-Builds, Java und Buildvertrag. Alle **997** erfassten Quell-/Skript-/Konfigurations-/Fixtureeingaben blieben während der Prüfung unverändert und waren vor ihrem Beginn geschrieben. Die kleine `fixtures/audio-upload-native.validation.json` hält die ursprüngliche Fixtureerzeugung fest; der abschließende frische Vergleich mit `fixture_verified=true` steht im lokalen Replaybericht.

Lokale Ergebnisse: `build/audio-upload/original-replay.validation.json`, `check/validation.json` und `verification-summary.json`. Der Abschlussbericht enthält die tatsächlich bestandenen Tests sowie den Hashmanifest-Nachweis aller während des Zentralchecks unveränderten Quell-/Skript-/Fixtureeingaben.

## Offene Integration und Windows-Abnahme

Die kontrollierten Grenzen sind Soundgültigkeit und Cacheantwort, Params-/Kategorie-/Listenerantworten, OpenAL-/EAX-Aufrufe samt Fehlerdiagnose sowie **Basis-Play/Basis-Pause** statt der echten FALSource-Treiberüberschreibungen. Prüfer und Port bewerten weder AL-Fehlercodes noch tatsächlichen Gerätezustand. `Check` ist ein Diagnoseaufruf, keine Bestätigung eines erfolgreichen Treiberbefehls.

Noch offen sind echter AL-Buffer-/PCM-Upload und Streamingnachfüllung, Treiber-Play/Pause/Stop und Ressourcenbesitz, globale Update-Sortierung/-Budgets, allgemeiner App-/Android-Treiberanschluss sowie Missions- und Hörabnahme. Kein neuer Android-/Missionslauf und keine neue APK für diesen Block. Die auf Nutzerwunsch zurückgestellte [Windows-Prüfliste der Geometrieanbindung](AUDIO_GEOMETRY_RUNTIME.md#windows-abnahme-bewusst-zurückgestellt) bleibt ausdrücklich offen. Neue Windows-Abnahme nach künftiger Anbindung: Einmal-/Dauerquellen, Looping/Streaming, lokale/räumliche Quellen, Pause unmittelbar nach Aktivierung und kaltes Fortsetzen über die Mission hören und prüfen.

Als nächste interne Kandidaten bieten sich die wirklichen FALSource-Play-/Pause-/Stop-/Statusmethoden, Bufferlebensdauer und Streamingnachfüllung sowie der globale Update-Sortier-/Budgetpfad an. Sie benötigen eigene Originalnachweise; die hier erfassten Aufträge ersetzen diese Methoden nicht.
