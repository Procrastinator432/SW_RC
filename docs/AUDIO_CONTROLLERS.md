# Allgemeine Audio-Controller: fünf intern geprüfte Kerne

**Weitere Quellfortsetzung 24.10.2026:** [Pause-/Stopbefehle und persistente Warteschlange](AUDIO_COMMANDS.md), fünf intern umgesetzte Punkte mit 1.088 Originalbefehlsaufrufen. Dort ist auch die korrigierte Zuordnung `IsPaused +1c` / `IsQueued +20` dokumentiert. Allgemeiner App-/Treiberanschluss bleibt offen.

**Nachfolgende Quellfortsetzung 24.10.2026:** [Audioquellenverwaltung](AUDIO_SOURCE_RUNTIME.md) ergänzt fünf intern umgesetzte Punkte, neun Testgruppen und 768 originale Zusammenschritte. Allgemeiner App-/Treiberanschluss und Missions-/Hörabnahme bleiben offen.

Stand: 24.10.2026. Die fünf in [NEXT_INTERNAL_WORK.md](NEXT_INTERNAL_WORK.md) ausgewählten Pakete sind als unabhängige Rust-Kerne umgesetzt. Sie benötigen weder Android noch vorbereitete Missionsszenen. Die App verwendet diese allgemeinen Controller noch nicht; die spielbare Referenz bleibt 0.60.0. Die bereits zurückgestellte [Windows-Missionsabnahme der Geometrieanbindung](AUDIO_GEOMETRY_RUNTIME.md#windows-abnahme-bewusst-zurückgestellt) bleibt offen.

## Umsetzung

| Paket | Modul in `crates/rc-package/src` | Original und geprüfter Vertrag |
|---|---|---|
| Kurven | `audio_curve.rs` | Setter `103d1cf0`, Auswertung `103d1dc0`, Hilfen `103d1b30`–`103d1ca0`: Arten 0–3, Nullzeit-Sentinels, Umkehr, Cache und einseitige Endbegrenzung. |
| Fade | `audio_fade.rs` | `GetFadeMultiplier` `103d2810`: Ein-/Ausblenden, Unterbrechung und Abschluss. Ausgewählter StopSound-Dispatch `103d6bf6`–`103d6c73` für sofortiges Stoppen sowie gestreamte/nicht gestreamte Kurvenwahl. `Fade::update_volume` verbindet den Controller mit der vorhandenen Lautstärkerechnung. |
| Ducking | `audio_duck.rs` | `UpdateDuckVolume` `103d1210` und `GetDuckMultiplier` `103d2920`: Stimmenwechsel, drei Kategorien, Unterbrechung/Rückkehr sowie Slot-, Quellen- und Levelausnahmen. Konfiguration, Stimmenzahl und Identitäten sind explizite Eingaben. |
| Laufende Priorität | `audio_priority.rs` | `UpdatePriority` `103d50e0`: Entfernung, Radius, Flags und Slotboni einschließlich Ambient-Slot-3-Bypass. Getrennt von der bestehenden Aufnahmepriorität. |
| Quellenpool | `audio_pool.rs` | Ganze `FindSource`-Funktion `103d0d10` mit kontrollierten Listen-/Treiber-Callbacks: erster freier Slot, ID-/Schutzbit, streng niedrigere Priorität und Stoppen→Übernehmen. |

Die Zustände von Kurven, Fade und Ducking speichern Fließkommawerte als rohe `u32`-Wörter. Die Tests serialisieren/deserialisieren sie nach jedem Vergleichsschritt und prüfen den bitgenauen Fortlauf. Ungültige Eingaben und beschädigte Zustände werden abgewiesen; fehlgeschlagene Controlleränderungen übernehmen keinen Teilzustand.

Der portable Vertrag begrenzt angeforderte Kurvenziele auf 0–1 und Controllerzeiten auf 0–60 Sekunden; Ducking-Angriffs-/Rückkehrzeiten müssen positiv sein. Direkte Kurvenschritte verlangen `delta < 1`, während der Fade-Controller den ursprünglichen Bypass für größere Schritte anbietet. Rohzustände haben endliche Betragsgrenzen, Prioritätseingaben begrenzte Koordinaten/Radien und Pools höchstens 4.096 Slots. Dies sind validierte Schnittstellengrenzen, keine Behauptung, dass die Originalengine beliebige beschädigte Eingaben abweist. Diese neuen Zustandsformate sind noch nicht Teil des App-Checkpointschemas.

Der Pool bietet getrennte Auswahl und Anwendung über `PoolHost`. Seine externen Callbacks laufen in Originalreihenfolge, auch beim freien Slot mit `stop` vor `attach`. Ein Stopfehler verhindert Attach; ein Attachfehler kann einen bereits ausgeführten externen Stop nicht zurückrollen.

## Bewusst erhaltene Originaldetails

- Die nichtlinearen Kurven sind keine angenommenen Standard-Easingkurven. Arten 2/3 verwenden die ermittelten Potenzformeln in beiden Richtungen. Zwischenwerte können leicht negativ oder größer als 1 werden. `Volume::update_controller_factors` erhält diese Rohfaktoren; die bisherige normalisierte `Volume::update`-Schnittstelle behält ihre Grenzen.
- Fade liefert bei `delta >= 1` den Faktor 1 ohne Zustandsfortschritt. Ein Fade-out-Abschluss liefert 0; ein Fade-in-Abschluss liefert den zuvor ausgewerteten Wert und setzt anschließend den Controller zurück.
- Der Ducking-Zweig vergleicht die Richtung im Original gegen **1**. Die normale Eintrittsrichtung 1 verwendet dadurch ebenfalls die Rückkehrdauer in der Zwischenrechnung. Sprünge/Extrapolationen werden erhalten. Das Quellen-Ausnahmeflag stammt aus Quelle `+3c` beziehungsweise Params `+2c`, Bit 0; es wird nicht aus der Streamed-Klasse abgeleitet.
- Der Pool beendet die Suche beim ersten freien Slot. Eine passende geschützte ID in einem späteren Slot wird dann nicht mehr untersucht. Bei Verdrängung zählen strikte Vergleiche; Gleichstände verdrängen nicht.

## Präzision und Grenzen

`CurveMath` macht die Rundungsgrenze der nichtlinearen Originalhilfen explizit. `PortableCurveMath` stimmt auf diesem Prüfrechner mit sämtlichen aufgezeichneten Hilfs- und Zustandswerten bitgenau überein. Das ist kein universeller Beweis für jede Plattformimplementierung von `asin`, `sin` und `pow`; die Originalfunktionen verwenden x87 und die ursprüngliche CRT. Vor Anbindung an weitere Zielplattformen sind deren Ergebnisse separat zu prüfen.

`RunningPriority::update` verlangt bei positivem Abstand den ursprünglichen `RSQRTSS`-Seed als Eingabe und bildet die nachfolgende SSE-Verfeinerung nach. Der Originalprüfer zeichnet diesen Seed auf. `update_portable` ist eine ausdrücklich alternative `sqrt`-Politik; daraus folgt keine Bitparität zur Näherung jeder x86-CPU.

Nicht enthalten sind die globale Stimmenverwaltung, vollständige StopSound-Suche/Queue, echte Treiberlebensdauer, ein vollständiger Quellenallocator, Streaming oder Android-SoundPool-Anbindung. Der synthetische Stop-Callback im Lautstärkevergleich protokolliert den Aufruf, ohne eine vollständige Enginequelle freizugeben. Ebenso sind Prioritäts-Listener, Level und Poolverwaltung kontrollierte Grenzen. Diese Einschränkungen sind Teil des internen Vertrags.

## Nachweis

Die eingefrorene synthetische Fixture `fixtures/audio-controllers-native.json` enthält ausschließlich Eingaben und Ergebnisse, keinen Originalcode oder Audiosamples. Der frische lokale Prüfer führt die Original-x86-Instruktionen einschließlich der ursprünglichen CRT-Hilfen aus:

| Vergleich | Fälle / Schritte |
|---|---:|
| Kurven, sechs rohe Zustandswörter und Hilfsproben | 1.728 Schritte |
| Fade-Zustände | 1.536 Schritte |
| StopSound-Dispatch | 72 Fälle |
| UpdateVolume mit echten Fade-/Ducking-Callbacks | 768 Schritte |
| Ducking-Zustände | 1.152 Schritte |
| Quellenabhängige Duckingfaktoren | 256 Fälle |
| Laufende Priorität mit aufgezeichnetem Seed | 1.024 Fälle |
| Ganzer FindSource-Pfad samt Callbackreihenfolge | 512 Fälle |
| **Summe** | **7.048** |

Sieben neue Rust-Testgruppen prüfen sämtliche Ergebnisse, gespeicherten Fortlauf, unveränderte Cacheabfragen und atomare Fehlerabweisung. Die Bibliotheksläufe bestehen mit jeweils **1.215 Tests in Debug und Release**. Der zentrale Projektcheck besteht einschließlich Format, Debug/Release, Materialimport-/Dokumentationstests, Clippy, Native-/Inspector-Build, Java und Buildvertrag. Der frische Originalvergleich bestätigt die eingefrorene Fixture vollständig. Alle 975 erfassten Quell-/Skript-/Konfigurations-/Fixturedateien blieben während des Checks unverändert. Die Belege liegen unter `build/audio-controllers/`; der zusammenfassende Beleg ist `verification-summary.json`.

Originaldateien werden vor Ausführung gehasht:

- `engine.dll`: `f8b3fc9d62414828a1c31d8b5f3b2bcb951f7c0f142fdf2a9846c98d7b5d35a5`
- `msvcr71.dll`: `8094af5ee310714caebccaeee7769ffb08048503ba478b879edfef5f1a24fefe`

Reproduktion unter Linux nach Werkzeugbootstrap:

```bash
source scripts/env-linux.sh
cargo test --locked --offline -p rc-package --lib audio_controller_tests
python scripts/Test-AudioControllersOriginal.py --game-data "$RC_GAME_DATA" \
  --verify-fixture fixtures/audio-controllers-native.json \
  --output build/audio-controllers/original-replay.json
./build.sh -Target Check -OutputDirectory build/audio-controllers/check
```

Unter Windows mit eingerichteten Werkzeugen aus PowerShell 7:

```powershell
cargo test --locked --offline -p rc-package --lib audio_controller_tests
python scripts/Test-AudioControllersOriginal.py --game-data $env:RC_GAME_DATA `
  --verify-fixture fixtures/audio-controllers-native.json `
  --output build/audio-controllers/original-replay.json
./build.ps1 -Target Check -OutputDirectory build/audio-controllers/check
```

Der frische Originalvergleich braucht die lokale Originalinstallation und `config/analysis-requirements.txt`; die Rusttests verwenden nur die versionierte Fixture. Keine neuen absoluten PC-Pfade werden in den Kern eingebaut.

## Noch erforderliche Abnahme

Die fünf internen Kernpakete sind umgesetzt. Ihre spätere Verbindung mit allgemeinen Spielquellen, Stimmenverwaltung und Treiberereignissen ist ein eigener Integrationsschritt. Dafür sind anschließend Missions-/Checkpoint-/Androidprüfungen und tatsächliche Hörabnahme erforderlich. Es wurde für diesen Block keine APK gebaut und keine neue Mission oder Hörprüfung als bestanden verbucht. Die Windows-Prüfliste für den vorherigen Geometrieblock bleibt zusätzlich bestehen.
