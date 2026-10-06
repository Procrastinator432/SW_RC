# Republic Commando → natives Android / Rust

Stand: 6. Oktober 2026. Das Projekt ist begonnen; eine spielbare Android-Portierung existiert noch nicht.

## Tatsächlich vorhanden

- Rust-Workspace mit einem plattformunabhängigen Paketleser, einem Windows-Analysewerkzeug und einer nativen Android-Bibliothek.
- 938 lokale Dateien inventarisiert; 273 Unreal-Pakete mit Namen, Importen, Exporten, Objektpfaden und geprüften Payload-Grenzen gelesen.
- 1.705 originale TextBuffer-Inhalte aus den Paketen extrahiert: 1.594 weitere Texte überwiegend UnrealScript und 111 `CppText`-Fragmente. Das ersetzt eine Bytecode-Dekompilierung für vorhandene Quellen. Gespeicherte Quellen müssen noch gegen den ausführbaren Bytecode abgeglichen werden.
- Ghidra 12.1.4 hat `ctgame.dll`, `engine.dll` und inzwischen `core.dll` analysiert; 60 bzw. 150 ausgewählte Funktionen sind als C-Pseudocode exportiert. Das ist weder eine vollständige Dekompilierung noch kompilierbarer Engine-Quellcode.
- Android-Szenenviewer 0.7 für ARM64 und x86-64. Er öffnet Originalkarten als BSP sowie am PC erzeugte Szenendateien mit statischen Meshes; Touch-Ziehen dreht die Kamera. Zoom und freie Bewegung in sechs Richtungen sind im Emulator geprüft; [Kamerasteuerung und Belege](docs/CAMERA.md). Originale Basistexturen vorhanden; noch ohne Gameplay. [Texturen und Grenzen](docs/TEXTURES.md).
- Original-Welt-BSP prüfbar: Punktzustand und Linien ohne Ausdehnung; 9.123 Modell-Tails in 79 Karten geprüft. Android-Diagnose im Emulator getestet; keine Spielerphysik. Klassenvererbung für 3.170 Klassen katalogisiert; alle 3.170 Default-Listen mit 25.119 Tags gelesen; das Pawn-Array ist strukturell dekodiert. [Klassendefaults und Grenzen](docs/CLASS_DEFAULTS.md). [BSP-Raumprüfung](docs/BSP_SOLID.md).
- Original-Startpunkte anwählbar: 755 Anchors in 79 Karten am PC mit belegten Defaults aufgelöst; bisherige 666 explizite Transforms unverändert. Direktes Android-CTM-Laden verwendet weiterhin explizite Tags. Snapshot-Version 3 und direkter CTM-Startpunkt im Emulator verifiziert; zusätzlicher entry-Anchor aus Default-Auflösung ebenfalls geprüft. [Startpunkte und Kollisionsbefunde](docs/LEVELS_AND_COLLISION.md).
- Originale Mesh-Kollisionspräfixe: 2.446 Meshes, 948.512 Kollisionsdreiecke und 2.526.483 Baumknoten gelesen. 1.696 einfache BSP-Formen validiert, 23 ausgenommen. PC-Linienproben mit Formauswahl, Actor-Transformationen und Spieler-Blockierflags: 236 einfache Formen / 30 freie Dispatches / 68 nicht blockierende Actors in geo_01a geprüft. Spielerbewegung noch ohne Kollision. [Kollisionsdaten und Grenzen](docs/MESH_COLLISION.md).
- Gemeinsame PC-Linienabfrage für Welt-BSP und Mesh-Actors mit Blockierern/Fehlern: 48 Proben in geo_01a, davon vier blockiert; zusätzlich entry geprüft. 383 Mesh-Actors einschließlich Props/Inventar erfasst. [Weltabfrage und Grenzen](docs/WORLD_COLLISION.md).
- BSP-Körperdiagnose am PC: 26.840 feste Kollisionshüllen aus 9.123 Models/79 Karten gelesen; 1.512 AABB-Sweeps mit aufgelösten Spielermaßen geprüft. Weltmodell jetzt aus Level-Referenzen in allen 79 Karten ausgewählt; 4.530 Proben, davon sechs ausdrücklich unvollständig in ras_02a. Native Grenzfälle und Bewegung offen. [Hüllen und Körperprüfungen](docs/HULL_SWEEPS.md).
- Level-Weltbindung und gemeinsame Körperprüfung: 45 anders benannte Weltmodelle korrekt referenziert. Transformierte Mesh-Hüllen mit Box-Dispatch und sortierten Kontakten; geo_01a/entry/ctf_hangar am PC geprüft. [Weltbindung und Körperkontakte](docs/LEVEL_BINDING_AND_BODY.md).
- StaticMesh-Leser: 2.446 von 2.447 Meshes dekodiert; ein Marker enthält einen nicht endlichen Wert. `geo_01a` als Szene mit 334 Mesh-Instanzen und 134.790 Dreiecken erzeugt. [Format, Prüfungen und Grenzen](docs/STATIC_MESHES.md).
- Kartenparser gegen alle 79 Originalkarten geprüft: 205.548 Property-Listen und 9.123 BSP-Modelle ohne Lesefehler. [Format, Belege und Grenzen](docs/MAP_FORMAT.md).

## Ergebnisse

| Pfad | Inhalt |
|---|---|
| `analysis/reports/inventory.json` | Dateiinventar und Parserstatus |
| `analysis/reports/packages/` | Vollständige Paket-Objekttabellen |
| `analysis/reports/sources/` | Erhaltene Originaltexte; `*.CppText.uc` sind C++-Fragmente trotz `.uc`-Dateiendung |
| `analysis/reports/binaries/` | PE-Köpfe, Sections, DLL-Importe, Exportnamen und Adressen |
| `analysis/decompiled/{ctgame.dll,engine.dll}/` | Funktionslisten und ausgewählter Ghidra-Pseudocode |
| `analysis/objects/` | Gezielt extrahierte Rohobjekte mit SHA-256-Herkunftsnachweis |
| `android/app/build/outputs/apk/debug/app-debug.apk` | Szenenviewer 0.7, Android 8+ / ARM64 und x86-64 |
| `analysis/scenes/geo_01a/world.rcscene` | Original-BSP plus 334 transformierte Mesh-Instanzen für Android |
| `analysis/scenes/geo_01a/scene.json` | Instanzen, Transformationen und ausdrücklich ausgesparte Actors |
| `analysis/scenes/geo_01a-textured/world.rcscene` | Szene mit 26 Originalbildern / 41 Materialbelegungen |
| `analysis/textures/validation.json` | Strukturprüfung von 2.312 Textur-Exporten; 47 Varianten noch nicht unterstützt |
| `analysis/meshes/validation.json` | Prüfung aller 2.447 StaticMesh-Exporte |
| `analysis/maps/geo_01a/world-bsp.png` | Untexturierte Vorschau des originalen Welt-BSP |
| `analysis/maps/validation.json` | Prüfung aller 79 Originalkarten |
| `docs/PORTING.md` | Architektur, Befunde und nächste Umsetzungsschritte |

Die originale Steam-Installation wird nur gelesen. Das Android-Paket enthält keine originalen Spielassets; exportierte Szenendateien enthalten davon abgeleitete Geometrie.

## Reproduzieren (PowerShell, im Projektordner)

```powershell
cargo test --workspace --offline
cargo clippy --workspace --offline -- -D warnings
cargo run -p rc-inspect --offline -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' 'analysis\reports'
& .\scripts\Analyze-Binary.ps1 -Binary 'System\ctgame.dll'
& .\scripts\Analyze-Binary.ps1 -Binary 'System\engine.dll'
& .\scripts\Build-Android.ps1
cargo run --offline -p rc-inspect --bin rc-map -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Maps\geo_01a.ctm' 'analysis\maps\geo_01a'
```

`Setup-Android.ps1` installiert SDK, NDK 27.2.12479018, Gradle 8.11.1 und das Rust-Ziel `aarch64-linux-android`. Ohne `-AcceptSdkLicenses` werden Lizenzen interaktiv abgefragt. Die Toolchain liegt unter `tools/`; Ghidra benötigt zusätzlich seine AppData-Einstellungen, Rust sein globales Target-Verzeichnis und Gradle den Android-Debug-Signierschlüssel.

## Verifikation

80 Rust-Unit-Tests und Clippy über alle Targets; Integration gegen alle 273 vorhandenen Pakete sowie Property-/BSP-Prüfung über alle 79 Karten; erfolgreicher Android-Build; APK-Signaturprüfung; ELF-Maschine AArch64; 16-KiB-ELF-Ausrichtung und APK-zipalign-Prüfung. Android-15-Emulator: Installation, Kaltstart, Originalkarten entry/geo_01a, Touch-Orbit, Fehlerbehandlung und erneutes Laden erfolgreich; Crash-Puffer leer. Einzelheiten und Startbefehle: [docs/EMULATOR.md](docs/EMULATOR.md). Szenenviewer 0.3: geo_01a mit 334 Mesh-Instanzen, Touch-Drehung und Karten-/Szenenwechsel ebenfalls erfolgreich geprüft. Physischer ARM64-Gerätetest und Gameplay-Verifikation stehen aus.

Die zwei gepackten Programme `System/SWRepublicCommando.exe` und `System/TestApp.exe` haben nicht direkt lesbare PE-Verzeichnisse. Diese sind im Bericht als `directory_error` mit `imports/exports: null` markiert, obwohl ihre PE-Köpfe lesbar sind. Die SSE/NONSSE-Varianten und DLLs liefern lesbare Symboltabellen.




Stand 0.5: Nah-Ebenen-Clipping, freie Kamera und Zoom mit texturierter geo_01a-Szene geprüft. Gegenbewegungen, Reset und erneutes Laden nach beschädigter Szene bestehen; Crash-Puffer leer. Details: [docs/CAMERA.md](docs/CAMERA.md).

Stand 0.6: Original-Startansicht in geo_01a, Kamera-Rückkehr und Reset exakt im Emulator bestätigt; direkter CTM-Import, ältere Szene und beschädigter neuer Snapshot geprüft. Kollisionsfunktionen und ausdrücklich abgeschaltete Actor-Flags untersucht; echte Spieler-Kollision und Gameplay fehlen weiterhin. [Details](docs/LEVELS_AND_COLLISION.md).

Stand 0.7: Welt-BSP-Raumzustand und kurze Blicklinie im Android-Viewer prüfbar (direkter CTM-Import). Solid-Boden, Rückkehr und Szenenwechsel im Emulator bestätigt. Defaults, Mesh-Kollision, Körper-Ausdehnung und Gameplay sind offen. [Details](docs/BSP_SOLID.md).

PC-Bewegungsdiagnose: Kontaktbegrenzung, Gleiten und Bodenabfrage gegen statische Körperhüllen; 126 Originalkarten-Bewegungsproben, 168 Positionsvorschläge und 42 Bewegungen aus exaktem Bodenkontakt. Berührende Ausgangspositionen sind jetzt zulässig; tatsächliches Eindringen wird separat erkannt. Noch keine vollständige Pawn-Physik oder Android-Integration. [Details](docs/MOVEMENT.md). Android-Prüfung erfolgt auf Nutzerwunsch zum Schluss.

PC-Schwerkraft und Bodenbindung ergänzt: 1.080 feste Physikschritte an sechs Startpunkten in drei Originalkarten geprüft; stabile Landung, Bewegung und Ruhephase. Eigene Integrationsregel, native Walking/Falling-Parität und Stufensteigen offen. [Details](docs/PHYSICS.md).

Konservatives PC-Stufensteigen ergänzt: 336 Originalkarten-Belastungsproben, vier angenommene Aufstiegskandidaten in ctf_hangar; zu hohe Hindernisse, niedrige Decken und fehlende Landeflächen getestet. Diagnoseparameter statt nativer Parität. [Details](docs/STEP_UP.md).

PC-Bewegungssteuerung ergänzt: Beschleunigung, Bodenbremsen, begrenzte Luftsteuerung und Sprünge bei neuer Tasteneingabe mit bestätigter Bodenunterstützung. 1.200 Originalkarten-Steuerungsschritte geprüft; zwölf Originalbewegungswerte mit Herkunft separat aufgelöst. [Details](docs/CONTROLLER.md).

Original-Bewegungsprofil und Yaw-Steuerung integriert: 14 Eigenschaften mit Herkunft, Vorwärts/Rückwärts/Seitwärts/Gehmodus, originale Körpermaße und Basis-Schwerkraft; 1.200 weitere Controller-Schritte geprüft. Aktive Volumes und native Formeln bleiben offen. [Details](docs/MOVEMENT_PROFILE.md).

PhysicsVolume-Auswahl rekonstruiert: 421 Volumes in79 Karten, 755 Startmittelpunkte; 753 eindeutige Auswahlen und zwei Prioritätsgleichstände mit unbekannter Runtime-Reihenfolge. Adapter für Bereichsschwerkraft synthetisch geprüft; Wasser/aufwärts gerichtete Schwerkraft offen. [Details](docs/PHYSICS_VOLUMES.md).

PhysicsVolumes in PC-Bewegung integriert: 2.340 Controller-Schritte an fünf Originalstartpunkten, einschließlich langer Sprünge bei geringer Schwerkraft und Bereichseintritt/-austritt in geo_01a. Alle Endzustände am Boden mit Geschwindigkeit0; Schwerkraftwechsel bei Grenzübertritt synthetisch bestätigt. [Details](docs/VOLUME_MOTION.md). Android-Prüfung weiterhin zum Schluss.

Nativer Walking-Rechenteil separat rekonstruiert: Richtungskorrektur durch GroundFriction und schwerkraftabhängiges Bremsen, 1.260 isolierte Schritte geprüft. Die bisherige PC-Steuerung bleibt bis zur Auflösung weiterer Runtimefaktoren bestehen. [Details](docs/WALKING_VELOCITY.md). Android-Prüfung weiterhin zum Schluss.

SpeedFactor aus Assembler rekonstruiert und mit Walking-Diagnose gegen Originalgeometrie verbunden: 300 Schritte auf geo_01a, diagonal bis~438,89, Gehmodus225, anschließend am Boden mit Geschwindigkeit0. Waffenfaktor/MaximumDesiredSpeed bleiben explizite Diagnosevorgaben; native Falling- und Script-Eingriffe fehlen. [Details](docs/WALKING_BRIDGE.md). Android-Prüfung weiterhin zum Schluss.
