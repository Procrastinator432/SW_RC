# Originale Startpunkte und Kollisionsanalyse, Stand 0.6

Der Android-Viewer kann jetzt die ausdrücklich gespeicherten `Engine.PlayerStart`-Positionen und Blickrichtungen einer Originalkarte anwählen. Das ist ein Kamerastandpunkt am Actor, noch kein gespawnter Spieler. Augenhöhe, Spawn-Auswahl, Inventar, Trigger und Bewegungsphysik werden nicht ausgeführt.

## Kartenbefund

`rc-level-check` prüft alle 79 `.ctm`-Karten: 755 exakte `Engine.PlayerStart`-Exports, davon 666 mit expliziter `Location` und `Rotation`; null Lesefehler. Bei 89 Starts fehlt mindestens eines dieser beiden Tags. Im direkten Android-CTM-Import sind diese weiterhin ausgeschlossen. Die PC-Szenenerzeugung löst inzwischen alle 755 Transforms auf; siehe [Klassendefaults](CLASS_DEFAULTS.md). Fehlende Bool-Tags werden ebenfalls nicht automatisch als `true` oder `false` interpretiert.

`geo_01a.PlayerStart0`:

| Eigenschaft | Originalwert |
|---|---|
| Location | `(2101.976806640625, 1829.5628662109375, -1997.7979736328125)` |
| Rotation, Pitch/Yaw/Roll | `(-291, -9815, 0)` |
| Event | `levelstart` |
| CollisionRadius des Start-Actors | `400` |

Der Radius des Start-Actors ist kein Beleg für den Kollisionsradius des Spieler-Pawns. Die erhaltene Quelle `PlayerCommando.uc` ruft beim Travel `Level.Game.FindPlayerStart(Controller)` auf. Die Auswahl des Spiels ist deshalb nicht durch die Export-Reihenfolge dieser Diagnosekamera ersetzt.

## Viewer und Szenenformat

Die Taste `Startpunkt` aktiviert die freie Kamera am nächsten enthaltenen Anchor; nach dem letzten beginnt die Auswahl wieder beim ersten. Die Szene `geo_01a` enthält einen solchen Anchor. Erneutes Anwählen stellt die Position nach Bewegung wieder her. `Übersicht` führt zur Orbit-Kamera zurück. Nach Bewegung oder Reset zeigt die Statuszeile wieder den allgemeinen Szenenstatus.

Originalkarten werden direkt über den Paketleser gelesen; am PC erzeugte Szenen erhalten dieselben Metadaten. Die Rotation verwendet die vorhandene Engine-Konvention: 65.536 Einheiten pro Umdrehung, unterste zwei Bits für den Trigonometrie-Index verworfen. Der Renderer blickt entgegen seinem Orbit-Richtungsvektor, deshalb wird Yaw um Pi gedreht und Pitch negiert. Nicht-null Roll und Pitch außerhalb des unterstützten Kamerabereichs werden ausdrücklich abgewiesen. Augenhöhe wird nicht hinzugefügt.

Snapshot-Version 3 erweitert Version 2:

- Header: `RCSC`, Version `3`, Dreieckanzahl, Texturanzahl, Startpunktanzahl; jeweils Little-Endian-u32, insgesamt 20 Byte.
- Dreiecke und Texturen wie Version 2: 68 Byte je Dreieck; Texturbilder mit Breite/Höhe und ARGB8888-Pixeln. Ohne Bilder bleiben UVs null und Texturindizes `-1`.
- Danach je Anchor: Namenslänge u32, UTF-8-Actorname, drei f32-Positionswerte und drei i32-Rotationswerte. Maximal 4.096 Anchors; Name 1–1.024 Byte; endliche Positionen mit Absolutwert höchstens 1e9; keine zusätzlichen Restbytes.

Der neue Reader akzeptiert weiterhin Version 1 und 2. Diese enthalten keine Startpunkte; ein Startpunkt-Aufruf liefert eine sichtbare Meldung, die Szene bleibt geladen. Ältere Viewer können Version 3 nicht öffnen. Der neue Export `analysis/scenes/geo_01a-start/world.rcscene` enthält dieselben 134.790 Dreiecke und 41 Texturbelegungen sowie einen Anchor; Größe 9.825.564 Byte.

## Was die Kollisionsanalyse belegt

Gezielt exportierter Ghidra-Pseudocode aus der lokalen `engine.dll`:

| Adresse | Funktion |
|---|---|
| `0x10552a60` | `UModel::FastLineCheck` |
| `0x105592b0` | `UModel::LineCheck` |
| `0x105563f0` | `UModel::PointCheck` |
| `0x10530500` | `UStaticMesh::LineCheck` |
| `0x10530a60` | `UStaticMesh::PointCheck` |

`collision-checks.c` und `collision-traversal.c` enthalten diese fünf Einstiege und sechs zusätzliche Hilfsfunktionen. Die BSP-Pfade verwenden Ebenen, Kindknoten und einen Inside/Outside-Zustand. `UModel::LineCheck` verzweigt bei einer von null verschiedenen Ausdehnung in einen anderen Prüfpfad. `UStaticMesh::LineCheck` unterscheidet ebenfalls Prüfungen mit/ohne Ausdehnung sowie alternative Primitive-/Modellpfade. Die inneren Hilfsfunktionen und ihre von Ghidra teilweise falsch rekonstruierten Signaturen müssen weiter mit dem Serializer und den Aufrufstellen abgeglichen werden. Windows-Feldadressen sind keine direkt übernehmbaren ARM64-Struct-Offsets.

Die Szeneninstanzen protokollieren jetzt ausdrücklich gespeicherte Kollisionsproperties. Von 334 aufgenommenen sichtbaren Mesh-Actors in `geo_01a` haben 68 jeweils `bCollideActors=False`, `bBlockActors=False` und `bBlockPlayers=False` gespeichert. Die ergänzte skalare Default-Auflösung belegt bei den übrigen 266 Actors True/True/True aus der Klasse. Daraus folgt noch keine implementierte Kollisionsform. Details und Herkunft: [Klassendefaults](CLASS_DEFAULTS.md).

`rc-render::trace_geometry` prüft zu Diagnosezwecken endliche Segmente gegen die **sichtbaren** Dreiecke, beidseitig und mit nächstem Treffer. Das Werkzeug verwendet f64-Rechnung, ist aber kein Original-LineCheck, kein BSP-Solid-Test und kein Sweep für einen Pawn-Körper. Es steuert die Android-Bewegung nicht.

Sechs 10.000-Einheiten-Proben am Startpunkt werden im Szenenbericht gespeichert. Die abwärts gerichtete Probe trifft sichtbare Geometrie nach etwa 689,152 Einheiten. Das ist keine gemessene Bodenfreiheit des Spielers: sichtbare Meshes können unpassierbar, nicht blockierend oder von einer separaten Kollisionsform verschieden sein.

## Reproduzieren

```powershell
cargo run --offline -p rc-inspect --bin rc-level-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Maps' analysis/levels/validation.json
cargo run --offline -p rc-inspect --bin rc-scene -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Maps\geo_01a.ctm' 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\StaticMeshes' analysis/scenes/geo_01a-start
```

Zusätzliche Belege: `analysis/levels/validation.json`, `analysis/scenes/geo_01a-start/scene.json`, `start-0.png` und `analysis/objects/geo_01a__PlayerStart0.bin.json` mit Herkunfts-Hashes. Die alten Szenen bleiben für Kompatibilitätsprüfungen erhalten.

## Nächste Umsetzung

UClass-Defaults und Vererbung auflösen; BSP-Tails einschließlich des Outside-Zustands und StaticMesh-Kollisionsrepräsentation aus den Original-Serializern lesen. Danach Ausdehnungsprüfungen, Pawn-Abmessungen, Augenhöhe, Timing und Bewegung gegen das Original verifizieren. Animation, Gameplay und Squad-Steuerung bleiben offen.

## Verifikation am 6. Oktober 2026

23 Rust-Tests und Clippy über alle Targets bestehen: Snapshot-v3-Roundtrip, alle Kürzungen und beschädigte Namen/Positionen/Anzahlen, Winkelkonvention, nicht unterstützte Winkel, C-ABI-Puffer-/Index-/Handle-Prüfungen und beidseitige Geometriesegmente mit nächstem Treffer. Die bestehenden v1-/v2-Tests bleiben erfolgreich.

Android 0.6.0-start-viewer für ARM64 und x86-64 gebaut; APK-v2-Signatur und 16-KiB-zipalign geprüft. Im API-35-x86-64-Emulator bestehen Szenenladen, Original-Startpunkt, Bewegung, erneute Anchor-Auswahl, Reset, direkter CTM-Startpunkt, v2-Szene ohne Anchor, beschädigte v3-Szene und erneutes Laden. Bildvergleich: Startansicht verändert 568.583 Pixel, Bewegung 567.433 Pixel; Rückkehr zum Anchor, Reset und Fehler-Recovery verändern jeweils null Pixel gegenüber ihrer Ausgangsansicht. Crash-Puffer leer. Physischer ARM64-Test steht aus.

Laufzeittest: `scripts/Test-StartEmulator.ps1` bei bereits gestarteter AVD ausführen; danach `scripts/Compare-StartFrames.py` mit Python und Pillow. Der Vergleich liest die tatsächlichen ImageView-Grenzen aus den UI-XMLs. Belege liegen unter `analysis/emulator/start-*.xml`, `start-*.png`, `start-validation.json`, `start-version.txt` und `start-crash-log.txt`. Die verwendete beschädigte Datei ist eine 32-Byte-Kürzung des neuen Snapshots (`analysis/emulator/truncated-start.rcscene`).

## Stand 0.7

Originale Model-Tails bis RootOutside/Linked werden jetzt gelesen; Punkte und Linien ohne Ausdehnung können im Welt-BSP geprüft werden. 9.123 Modelle in 79 Karten ohne Fehler geprüft. Klassenvererbung für 1.302 Klassen katalogisiert; DefaultProperties und vollständige Spieler-/Mesh-Kollision bleiben offen. 27 Tests/Clippy, ARM64-/x86-64-Build, Signatur/zipalign und Emulator-Diagnose bestehen. Siehe [BSP_SOLID.md](BSP_SOLID.md).
