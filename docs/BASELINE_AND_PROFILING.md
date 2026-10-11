# Meilenstein 0: Referenz, Eingabe-Replay und Laufzeitmessung

Stand: 10. Oktober 2026. Abgeschlossen mit Android 0.8.1. Der erste Spielabschnitt bleibt funktional unverändert; Referenzsicherung, reproduzierbare Eingaben, zuschaltbare CPU-Messungen und zusätzliche Android-Eingabeprüfungen sind ergänzt. Die nächste Ausbaustufe ist [Meilenstein 1 im Fortsetzungsplan](CONTINUATION_PLAN.md).

## Gesicherte Ausgangsversion

`analysis/baselines/0.8.0-playable-scene/` enthält:

- `source.zip`: 641 Quelldateien und Projektdateien einschließlich der vorher vorhandenen lokalen Änderungen; etwa 1,6 MiB komprimiert.
- Die ursprüngliche APK 0.8.0, das vollständige lokale geo_01a-Szenenpaket, Szenenmetadaten, Android-Screenshots/-Protokolle sowie Build- und Prüfnachweise.
- `manifest.json`: SHA256 für Quellen, Archiv und kopierte Artefakte; Git-Ausgangscommit und damaliger Änderungsstand, Rust-Versionen und Android-Werkzeugversionen.

Vor der Sicherung wurden alle 295 Quellenhashes und die Artefakthashes des bisherigen Spielmeilensteins mit den lokalen Dateien verglichen. Die 41 kopierten Artefakte und das Quellenarchiv werden im neuen Profiling-Lauf erneut auf unveränderte Hashes geprüft. Die Sicherung überschreibt keine vorhandene Baseline. Der Quellstand ist als lokales Archiv gesichert, ohne die vorherigen Änderungen zu committen.

Für die Wiederherstellung das Quellenarchiv in einen eigenen leeren Projektordner entpacken und die benötigten Artefakte an ihre gespeicherten relativen Pfade kopieren. Originalinstallation und lokale Abhängigkeits-/Werkzeugcaches bleiben externe Voraussetzungen; die Baseline archiviert diese umfangreichen Installationen nicht. Das bereits exportierte Szenenpaket genügt für die Spiel-/Replay-Prüfung.

## Eingabe-Replay

`rc-game::replay` zeichnet ausschließlich die eingereichten Frame-Eingaben samt Zeit, Suspend und Restart auf. Die Szenendaten bestimmen die Ausgangslage. Das Aufzeichnen ermittelt Zielrichtungen aus der Szene und erzeugt daraus normale Blick-Eingaben; Positionen, Lebenspunkte, Treffer und Zielzustände werden beim Abspielen nicht gesetzt.

`analysis/milestone0/replay.json` enthält 1.002 Kommandos mit Abschnitten für Sprung/Landung, seitliche Bewegung, kurz gepufferte Eingabe, Pause/Fortsetzen, Kampf, Nachladen, Exfil-Sieg, Gegnerbeschuss/Niederlage und Neustart. Ein eigener Zeitlimit-Fall prüft das Verwerfen von Rückstand. Die aktuelle Simulation verwendet keinen Zufallsgenerator; das Replay speichert diese Voraussetzung ausdrücklich.

Pro Kommando werden Tickanzahl und ein Fingerprint des gesamten veränderlichen Spielzustands geprüft, einschließlich Zeitakkumulator und noch gepufferter Eingaben. Ausgewählte Referenzbilder werden zusätzlich verglichen. Die Rust-Fingerprints verwenden FNV-1a zur schnellen Abweichungserkennung. Die Python-Orchestrierung prüft die SHA256-Bindung zwischen Replay und Szenenpaket und hält kryptografische Artefakthashes im Abschlussmanifest fest.

Sechs Release-Durchläufe mit wechselnder Reihenfolge von aktivierter und deaktivierter Detailmessung liefern dieselben Zustände und Referenzbilder. Der Debug-Build spielt dieselbe gespeicherte Spur exakt ab. Veränderte erwartete Zustände, zusätzliche versteckte Eingabefelder, falscher Bundle-Fingerprint und falsche SHA256-Bindung werden gezielt abgewiesen.

Der frühere `rc-play-check` läuft zusätzlich gegen die neue Implementierung. Seine Zustände und alle fünf PPM-Bilder stimmen exakt mit den gesicherten 0.8-Nachweisen überein.

## Messung und Grenzen

Der threadlokale Collector in `rc-package::performance` ist standardmäßig deaktiviert. Messwerte gehören nicht zum Spielzustand. Aktivierte Bereiche messen Simulation, statische Körper-Sweeps, Actor-Geometrieaufbereitung, Szenenzeichnung, Bildpuffer, HUD, native Pixelkopie und JNI-Arrayarbeit. Auf dem PC kann zusätzlich die Rasterung pro Dreieck gemessen werden.

Kategorien sind teilweise ineinander enthalten: Kollision liegt innerhalb der Simulation; Rasterung und Bildpuffer liegen innerhalb der Szenenzeichnung. Ihre Werte dürfen nicht zusammen als Gesamtzeit addiert werden. Die Restzeit der Szenenzeichnung enthält Transformation, Clipping, Projektion und weitere Verwaltungsarbeit. Die Kollisionskategorie umfasst die instrumentierten `StaticBodyWorld`-Sweeps, keine behauptete vollständige Original-Engine-Profilerabdeckung.

Feine Timer pro Dreieck erwiesen sich im Emulator als erheblich zu teuer. Android verwendet deshalb die gröbere Messung ohne einzelne Rastertimer; dort bedeutet der Rasterzähler null ausdrücklich „nicht separat gemessen“. Auf dem PC wurde der Messaufwand durch gepaarte Durchläufe bestimmt: etwa 15 % Mehrkosten für den Bildpfad mit Detailmessung. Die nicht instrumentierte Vergleichsmessung ist separat gespeichert.

Android-Messung kann über den bestehenden Launcher mit `--ez rcProfile true` zugeschaltet werden. Der normale App-Start lässt sie ausgeschaltet. `RCPerf` protokolliert auf Wunsch CPU-Zeiten, tatsächliche Frame-Eingaben, beobachtete Position/Blickrichtung, Spielzähler und Heap-Belegung. Diese Beobachter setzen keinen Spielzustand.

JNI-Wandzeit umfasst den nativen Aufruf insgesamt; Arrayarbeit und native Pixelkopie sind darin enthalten. Android-Bitmap-Zeit erfasst `setPixels` und die Übergabe an `ImageView`. Tatsächliche Bildschirmpräsentation, GPU-Komposition und Eingabe-bis-Display-Latenz werden damit nicht gemessen.

## Ergebnisse bei 640 × 400

Windows/x86-64, Rust Release, lokal gespeicherte Eingabespur. Die PC-Messung tastet 72 Renderframes pro Durchlauf ab und ist kein interaktiver FPS-Test.

| PC-Bereich | Median | 95. Perzentil |
|---|---:|---:|
| Simulation je Eingabeframe mit Ticks, überwiegend ein Tick | 1,14 ms | 1,56 ms |
| Bildpfad ohne Detailinstrumentierung, einschließlich Pixel-Fingerprint | 36,88 ms | 39,66 ms |
| Szenenzeichnung mit Detailinstrumentierung | 41,16 ms | 43,73 ms |
| Rasterung innerhalb dieser Szenenzeichnung | 30,08 ms | 31,71 ms |

API-35-/x86-64-Emulator, Software-Grafik, Rust Release in Debug-APK 0.8.1. 388 eingereichte Frames aus dem vollständigen normalen Android-Spielablauf:

| Android-Bereich | Median | 95. Perzentil |
|---|---:|---:|
| Simulation je Frame, variable Zahl fester Ticks | 5,45 ms | 11,49 ms |
| CPU-Szenenzeichnung | 39,19 ms | 49,39 ms |
| Native Pixelkopie | 0,38 ms | 0,63 ms |
| JNI-Arrayarbeit | 0,05 ms | 0,13 ms |
| Bitmap-Aktualisierung und Übergabe | 0,19 ms | 0,40 ms |
| Gesamter JNI-Aufruf, einschließlich Simulation und Zeichnung | 45,39 ms | 59,35 ms |

Die Szenenzeichnung benötigt etwa 85,7 % der erfassten, überlappungsfrei zusammengestellten nativen Stufenzeiten. Auf dem PC entfällt etwa 72,7 % der detaillierten Szenenzeit auf Rasterung. Ein GPU-Pfad für die Originalwelt ist damit die konkret begründete nächste Optimierung. Die Kollisionsabfragen dominieren zwar die Simulation, deren absolute Kosten liegen im aktuellen Ablauf deutlich unter denen der Zeichnung.

Im normalen gemessenen Android-Ablauf wurde keine Simulationszeit verworfen. Lesen des Szenenpakets benötigt im Median etwa 91 ms, Erzeugen der nativen Spielsitzung etwa 41 ms; dafür liegen nur zwei erfolgreiche Ladeproben vor. Die beobachteten Heap-Spitzen betragen etwa 60,0 MiB Java und 36,6 MiB nativ. Eine zusätzliche `dumpsys meminfo`-Momentaufnahme liegt bei etwa 54,6 MiB Gesamt-PSS. Diese Werte belegen keinen Langzeit-Speichertest und sind nicht einfach addierbar.

## Android-Funktions- und Eingabeprüfung

Der bisherige vollständige UI-Test besteht mit zugeschalteter grober Messung: normales Dateiauswählen, Zielen/Schießen, drei Kills, Nachladen, Exfil-Sieg, Sprung, Niederlage, Neustart, unveränderte Pause sowie Abweisen einer beschädigten Datei mit anschließendem erneutem Laden.

Die separate lokale Test-APK `org.rcport.inputprobe` injiziert Android-`MotionEvent`s über `UiAutomation`. Sie ruft keine Spiel- oder JNI-Funktionen auf. Vier nicht aufeinanderfolgende Pointer-IDs (2, 7, 19, 23) prüfen:

- Bewegung, Blickänderung und Feuer erreichen denselben Rust-Eingabeframe; Sprung und Feuer können gleichzeitig anliegen.
- Beobachtete Position, Blickrichtung, Schuss- und Sprungzähler ändern sich tatsächlich.
- `ACTION_CANCEL` gibt alle gehaltenen Aktionen frei; Feuer bleibt nicht hängen.
- Home während gehaltener Eingabe stoppt die Simulation. Rückkehr in dieselbe Aufgabe erhält den eingefrorenen Zustand und löscht gehaltene Eingaben.
- Abbrechen des normalen Dateidialogs führt zurück in die bestehende Spielsitzung.

Die Testinitialisierung verwendet jetzt den echten MAIN-/LAUNCHER-Intent. Dadurch kehrt der Fokus-Test wie beim normalen App-Start in die vorhandene Aufgabe zurück, statt durch einen abweichenden Test-Intent eine zusätzliche Hauptansicht anzulegen.

Die Tast-/Mehrfingerprüfung gilt für Android-Ereignisverarbeitung im Emulator. Ein physisches ARM64-Gerät, echte Touch-Hardware, thermisches Dauerverhalten und das Ziel von stabilen 30 FPS auf einem benannten Gerät bleiben offen.

## Prüfungen und Reproduktion

934 Workspace-Tests, Clippy mit verweigerten Warnungen und Formatprüfung bestanden. Die native DLL wurde mit dem echten Bundle geprüft: kurze Puffer/Canaries, Profiling-Beginn/-Ende, unveränderte Pixel, Rücksetzen, freigegebene Handles und lesender Positionsbeobachter. Die APK enthält ARM64 und x86-64; Signatur, ZIP-Ausrichtung und alle vier ELF-Bibliotheken mit mindestens 16-KiB-Ausrichtung sind geprüft. Originalassets sind weiterhin nicht in der APK enthalten.

Im Projektordner mit dem lokalen Python aus der Codex-Runtime (für Screenshot-Prüfungen einschließlich Pillow):

```powershell
python scripts/Profile-PlayableScene.py
cargo build --release --offline -p rc-native
python scripts/Validate-PlayableAbi.py --output analysis/milestone0/abi-validation.json
./scripts/Build-Android.ps1 -Abis arm64-v8a,x86_64
./scripts/Build-InputProbe.ps1
./scripts/Start-TestEmulator.ps1
python scripts/Test-PlayableEmulator.py --profile --output analysis/milestone0/android
python scripts/Test-PlayableInput.py
python scripts/Summarize-PlayableProfile.py
```

`Summarize-PlayableProfile.py` setzt die aktuellen Workspace-/Clippy-/Format-/Build- sowie Signatur-/Ausrichtungsprotokolle in `analysis/reports/milestone0-*.log` voraus. Die zugrunde liegenden Prüfkommandos stehen außerdem in der bisherigen [Spielanleitung](PLAYABLE_SCENE.md). Die vorhandene Baseline wird bei der Reproduktion nicht neu erstellt.

Zusammengeführter Nachweis: `analysis/milestone0/validation-manifest.json`. Einzelberichte: `cpu-summary.json`, `cpu-profile.json`, `android-summary.json`, `android/validation.json`, `input/validation.json` und `abi-validation.json`. Hashes verbinden Quellen, Replay, APK und Prüfarbelege.

## Nächster Schritt

Meilenstein 1 beginnt mit einer klaren Renderdatenübergabe aus dem unveränderten Rust-Spielzustand. Danach folgt ein kleiner GPU-Pfad für statische geo_01a-Geometrie, Basistexturen, Kamera und Tiefe; dieselben Eingabespuren sichern Bewegung, Kampf und Ziele ab. Die detaillierte Direct3D-/Materialrekonstruktion und vorhandene Animationsbausteine werden entsprechend den benötigten sichtbaren Funktionen angeschlossen.
