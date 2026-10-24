# Audio-Geometrie im ausgewählten Quellenhost

Stand: 24.10.2026 · Quellfortsetzung nach 0.60.0, **keine neue spielbare Prüflieferung**.

Die ersten vier internen Arbeitspakete sind im Code umgesetzt. Paket 5 enthält neue bestandene Rechen-/Checkpointregressionen; der vollständige Missionslauf und historische Szene-/Spielstandpaare bleiben wegen fehlender lokaler Missionseingaben offen. Android- und physische Hörabnahme gehören zur anschließenden Prüfung. Die bisherige spielbare Referenz bleibt 0.60.0.

## Die fünf Arbeitspakete

| Paket | Umsetzung und Nachweis |
|---|---|
| 1 · Zonendistanzen | Originalen Block aus `UGameEngine::LoadMap`, `1041f33e..1041f4bc`, rekonstruiert. Zwölf synthetische Tabellen und alle 4.096 Bytes jeder der vier Kartentabellen stimmen mit der ausgeführten Original-DLL überein. |
| 2 · Actor-Modi und Regionen | `SoundOcclusion` aus Instanz/Klassenhierarchie und Mover-Vererbung importiert. Alle sechs ausgewählten Quellen haben ursprünglich Modus 0. Actor-/Listenerregionen liegen in positionsgebundenen Caches; Bewegungen invalidieren sie. Cachewerte werden beim Laden gegen den originalen BSP geprüft. |
| 3 · Szenenimport | Der normale `rc-presentation-build` importiert BSP, Connectivity, Tabelle und Actor-Regeln in das optionale `ambient_geometry`-Modul. Der unabhängige `rc-audio-geometry-assets` importiert und prüft die vier Karten ohne vorbereitete Missionsszene. |
| 4 · Laufzeit und Persistenz | Hörbarkeit/Zonenfaktor beeinflussen Aufnahme und Freigabe; der originale Radiusfortlauf beeinflusst den ausgegebenen Wiedergaberadius. Quelleninitialisierung, kaltes Laden, Ausblenden, erneute Aufnahme, unveränderliche Abfragen und atomare Fehlerabweisung sind in begrenzten Laufzeitfixtures geprüft. |
| 5 · Regressionen | Quell-/Originalprüfungen und synthetische alte/neue Checkpoints sind vorhanden. **Offen:** reguläre A/B/C→Fabrik-Eingabeläufe sowie historische Szene-/Spielstandpaare. Der tatsächliche Szenenbau stoppt bereits am fehlenden `local-assets/mission1/scene-inputs.json`. Synthetische Tests ersetzen diese Abnahme nicht. |

## Interne Prüfung

Der zentrale Quellcheck besteht mit je **1.208 Debug-/Release-Bibliothekstests**, Clippy aller Targets, Materialimport-/Doc-Tests, nativem Bibliotheksbau, Java-Prüfungen und Buildvertrag. Sieben neue Bibliothekstests prüfen Originaltabellen, Quelleninitialisierung, Cacheinvalidierung/Modi, kaltes Fortsetzen, erneute Aufnahme, Radiusüberschwingen, sieben atomar abgewiesene Geometrieschäden und den unveränderten Serialisierungsvertrag alter Szenen. Die Game-Laufzeitfixtures sind synthetisch; die Originalkartenprüfung liest echte Kartendaten. Beides ersetzt keine vollständigen Missions-/historischen Paarläufe.

## Originalbefunde und Grenzen

`FZoneProperties` wird in `1045c230` als Actorreferenz, Connectivity (u64), Visibility (u64), LastRenderTime (f32) serialisiert. Der bisherige Import übersprang diese 20 Bytes nach dem Actor. Nur **Connectivity** speist die Audiotabelle; Actorreferenzen und Visibility werden nicht als Verbindungen umgedeutet.

LoadMap initialisiert die Diagonale mit 0 und alle anderen Tabellenbytes mit 255. In den Iterationen 1 bis 63 setzt es erreichbare, noch größere Distanzen auf den Iterationswert und **quadriert anschließend die gerichtete Erreichbarkeitsrelation**. Es ergänzt keine Symmetrie oder Selbstverbindungen. Ohne Selbstverbindungen kann ein Drei-Kanten-Ziel unerreichbar bleiben, während ein Vier-Kanten-Ziel Distanzbyte 3 erhält. Das ist ausdrücklich keine gewöhnliche kürzeste Weglänge. Die synthetischen Originalfälle enthalten gerichtete Ketten, reflexive Ketten, Ring, höchste Bits, leere/volle und zufällige Netze.

Die ursprüngliche Radiusinitialisierung aus `FAbstractSource::Init`, `103d2c4d..103d2c7b`, setzt Prioritäts-/Zielradius auf den ausgewählten Basisradius, den anfänglichen Wiedergaberadius auf `max(Basisradius, 1)` und den Änderungszeitpunkt auf **-1**. Fünf zusätzliche Originalausführungen prüfen diese Werte. Jede erneute Aktivierung initialisiert ihre Radiuszeit neu.

Die vorhandene originale Radiusrechnung kann nach längerem unverändertem Ziel über- oder unterschwingen; ihre Rohbits bleiben unverändert gespeichert. Der portable Wiedergabehost erhält einen Radius in 0..1.000.000. Ein auf 0 begrenzter geometrischer Radius erzeugt bei aktivem Distanzabfall keinen Stream, denn der bisherige Androidadapter deutet Radius 0 als deaktivierten Abfall. Diese Ausgabegrenze ist eine Portregel, kein Nachweis identischen OpenAL-Verhaltens für negative Radien. Seed, gewählte Originalparameter und Aktivierungskennung werden dadurch nicht neu ausgewählt.

Radiuszeitbits werden als zwei u32-Wörter gespeichert. Damit bleibt f64 exakt, ohne die allgemeine Checkpointgrenze für JSON-Zahlen zu erweitern. Szenen ohne `ambient_geometry` erhalten weder neue Cache-/Radiusfelder im gespeicherten Zustand noch neues Hörbarkeitsverhalten. Neue Szenenidentitäten benötigen zugehörige neue Checkpoints; es gibt keine stille Migration alter Spielstände in eine geänderte Szene.

Der ausgewählte Host nutzt weiter die bestehende Präsentationskamera und die Bacta-Eye-/Effektpositionen. Der Simulationsclock ist `ticks × 1/60`, Hintergrundzeit wird nicht nachgeholt; die originale globale Enginezeit und globale RNG-Historie werden damit nicht behauptet. Der Positionscache ist ein deterministischer Portcache, kein vollständiger `AActor::SetZone`-/ZoneChange-Ereignishost.

Modus 3 erfordert den noch nicht portierten allgemeinen Trace mit Maske `0x286`, einschließlich der passenden Mesh-/Moverkollision. Solche Quellen werden beim Szenenimport ausdrücklich abgewiesen; ein Mover erzwingt wie im Original den Bypass. Die sechs aktuellen Quellen benötigen diesen Trace nicht. Allgemeine Einmalgeräusche, Quellenverdrängung, Mixer/EAX/HRTF/Doppler und freie physische Abnahme bleiben eigenständige Umfänge.

## Windows-Abnahme bewusst zurückgestellt

**Entscheidung vom 24.10.2026:** Auf ausdrücklichen Nutzerwunsch bleibt Paket 5 bis zur Rückkehr an den Windows-PC offen. Die fehlenden Missionseingaben werden für die nächsten internen Arbeiten hier vorerst nicht wiederhergestellt. Diese Zurückstellung ist keine bestandene Missions-/APK-Abnahme.

Am Windows-PC noch durchzuführen:

- [ ] Den dann aktuellen Quellstand und den vorhandenen, gehashten Missionseingabesatz unter `local-assets/mission1` bereitstellen; Originalinstallation und historische passende Szene-/Spielstandpaare verfügbar halten.
- [ ] Native DLL und Quellprüfungen neu bauen. Die Szenen aus diesem Quellstand neu erzeugen, damit das optionale Geometriemodul tatsächlich enthalten ist; ein altes 0.60.0-Paket prüft diese Änderung nicht.
- [ ] Die regulären A/B/C→Fabrik-Eingabeläufe einschließlich Kartenwechseln, Checkpoints, exaktem Fortlauf und Neustart über die Szenenpipeline bestehen lassen.
- [ ] Den neuen Quellenpfad mit `Test-SpatialAmbient.py` prüfen: Aufnahme/Freigabe, Bacta-/Feuerquellen, Radiusbits, Ausblendung, erneute Aufnahme, kaltes Laden und beschädigte Zustände.
- [ ] Historische Szene-/Spielstandpaare mit der neuen DLL auf unveränderten Zustand und Darstellung prüfen. Alte Saves jeweils mit ihrer zugehörigen alten Szene testen; keine stille Übertragung in eine neu erzeugte Szene.
- [ ] Quellstand, DLL-/Szenen-/Replay-Hashes und neue Prüfberichte dokumentieren; Fehler beheben und die betroffenen Läufe wiederholen. Android- und freie Hör-/Bild-/Touch-Abnahme bleiben anschließende eigene Schritte.

PowerShell 7, mit dem tatsächlichen lokalen Spielpfad:

```powershell
$gameData = 'D:/Pfad/zur/Installation/GameData'
./build.ps1 -Target Check -OutputDirectory build/windows-audio-check
./build.ps1 -Target Scenes -SceneInputs local-assets/mission1/scene-inputs.json `
  -GameData $gameData -OutputDirectory build/windows-audio-check
python scripts/Test-SpatialAmbient.py --dll target/release/rc_native.dll `
  --scenes build/windows-audio-check/scenes --output build/windows-audio-check/native-spatial
```

Die erste Zeile ist eine Pfadvorlage. Die Befehle bauen und prüfen ohne Android-Emulator; ihre erfolgreichen Ergebnisse stehen für diesen Quellstand noch aus. [Nächste unabhängig intern prüfbare Aufgaben](NEXT_INTERNAL_WORK.md).

## Reproduzieren

Quellprüfung, Windows mit PowerShell 7:

```powershell
./build.ps1 -Target Check -OutputDirectory build/audio-integration/check
```

Linux:

```bash
source scripts/env-linux.sh
./build.sh -Target Check -OutputDirectory build/audio-integration/check
cargo run --release -p rc-inspect --bin rc-audio-geometry-check -- "$RC_GAME_DATA" build/audio-integration/maps
cargo run --release -p rc-inspect --bin rc-audio-geometry-assets -- "$RC_GAME_DATA" build/audio-integration/assets
python scripts/Test-ZoneDistancesOriginal.py --game-data "$RC_GAME_DATA" --output build/audio-integration/original-zone-distances.json --verify-fixture fixtures/zone-distances-native.json --models build/audio-integration/maps
python scripts/Test-AudioGeometryOriginal.py --game-data "$RC_GAME_DATA" --output build/audio-integration/original-geometry.json --verify-fixture fixtures/audio-geometry-native.json --models build/audio-integration/maps
```

Die Originalprüfungen benötigen `config/analysis-requirements.txt`; sie prüfen den festgehaltenen Hash von `engine.dll` und beim bestehenden Geometrieprüfer zusätzlich `msvcr71.dll`. Die neue versionierte Fixture enthält nur synthetische Eingaben und Originalergebnisse. Originaldekompilate und abgeleitete Kartendaten bleiben lokal im ignorierten Buildbereich.

Sobald der lokale Missionseingabesatz wieder vorliegt, folgen der normale Szenenbau, `Test-SpatialAmbient.py` gegen den neuen nativen Host und die historische Paarprüfung. Der Spatial-Prüfer vergleicht nun den ausgegebenen Radius mit den gespeicherten Radiusbits, die Aufnahmepriorität weiterhin mit dem ursprünglichen Basisradius. Neue beschädigte Geometriezustände werden zusätzlich geprüft. Danach folgen Android und Handy; bis dahin wird keine neue Missions-/APK-Abnahme behauptet.

Lokale Prüfbelege: [Gesamtbericht](../build/audio-integration/verification-summary.json), [Quellcheck](../build/audio-integration/check/validation.json), [Zonentabellen](../build/audio-integration/original-zone-distances.validation.json), [Geometrie](../build/audio-integration/map-original-validation.json), [Actor-/Kartenimport](../build/audio-integration/assets/validation.json), [Missionsblocker](../build/audio-integration/mission-attempt/validation.json).
