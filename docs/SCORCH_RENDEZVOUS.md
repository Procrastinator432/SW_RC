# Spielbare Scorch-Begegnung / Android0.24.0

Der Abschnitt bis Originalziel4 „Rendezvous with Six-Two“ ist zusammenhängend spielbar. Die Landung, Bacta-Heilung, Absturzstelle, Granatenaufnahme und Geschützzerstörung führen jetzt durch den niedrigen Trümmerdurchgang zu Scorch. Er bewegt sich zum Schusspunkt und anschließend zum Begrüßungspunkt; beim Treffen folgen Originaldialoge, Squad-Anzeige und der Abschluss des Abschnitts. Die gesamte erste Mission ist noch nicht abgeschlossen; der anschließende Squad-Sprengbefehl folgt separat.

## Lieferung und Start

- APK für ARM64 und x86_64: `analysis/mission5-playable/republic-commando-0.24.0.apk` (VersionCode27).
- Passende Szene: `analysis/mission5-playable/geo_01a-scorch.rcplay` (RCPLAY07).
- APK installieren, die Szene aufs Gerät kopieren, „SPIELABSCHNITT GEO_01A ÖFFNEN“ und „SZENE ÖFFNEN“ auswählen. Die APK benötigt die separate Szenendatei.

Die bestehende Steuerung bleibt erhalten. Neu: **DUCKEN gedrückt halten**; bei Tastaturbedienung **C**. Nach der Geschützzerstörung dem grünen Zielmarker folgen und unter den niedrigen Trümmern geduckt hindurchgehen. Loslassen richtet den Spieler erst auf, wenn ausreichend Platz vorhanden ist. Scorch hinter dem Bunker treffen und seine HUD-Verknüpfung/Advisor-Dialoge abwarten. Der Abschnitt endet nach dem Originalziel4-Abschluss mit „SCORCH IM SQUAD“.

## Ursache und Umsetzung des Anschlusswegs

Die vorherige Wegprüfung scheiterte mit dem stehenden Spielerzylinder40/84. Bild- und Körperproben zeigen den niedrigen Originaldurchgang; an PathNode295/Export1162 ist die stehende Position in Slab/BSPTrim, die geduckte Position dagegen frei. Der neue Weg nutzt die originalen Duckmaße Radius40/Höhe56 und CrouchSpeedRatio0,5. Native Höhenkompensation und die vorhandene vollständige statische Platzierungsprüfung sind in den Spielkern eingebunden. Die Füße bleiben beim Formwechsel am selben Ort; blockiertes Aufstehen behält die Hocke bei.

101 bodengestützte Originalanker und774 physisch geprüfte Verbindungen liefern14 Wegpunkte bis zum ursprünglichen Begegnungsauslöser458. Es wurden keine zusätzlichen Kollisionen entfernt und keine Spielerpositionen über den Durchgang versetzt. Die zehn dynamischen Freigaben aus Zielen0–3 und die35 BunkerBlown-Hüllen bleiben bestehen. Die Wegsuche und ihre Sprungversuche sind Port-Adapter; der erfolgreiche Eingabelauf benötigt auf diesem Anschlussweg Ducken.

## Scorch, Welt und Dialoge

- Fabrik457 und Script454 bestimmen Start, Schusspunkt1130, Begrüßungspunkt792 und die ursprüngliche GestureSignalOK-Animation. Der Begleiter verwendet den vorhandenen kollisionsgeprüften Pawn-Controller mit GroundSpeed450; vorbereitete Originalwege verbinden die beiden Standorte.
- Originales `Clone.CloneCommando`-Mesh mit ActionBreathe, RunForward und GestureSignalOK. Die gelbe Scorch-Textur stammt aus der serialisierten Skins-Liste von `Properties.Commando62DeltaDefaults`: `CloneTextures.CloneTextures.CloneCommando62_Shader`. Das generische Proxy-Material wird ersetzt.
- Der große Bunker851/Prop12 erhält1165 zuvor fehlende Originalflächen mit Oberflächentexturen. Seine vorhandene Kollision bleibt erhalten.
- Neun ausgewählte Scriptspuren mit55 Originalaktionen und13 zusätzlichen PCM-Samples sind angebunden. Die gesamte Szene besitzt33 Audio-Rollen. Spielertrigger532 startet das Intro,1133 den Dialog hinter dem Bunker,458 die Squad-Aufnahme. Zwei Skripte können durch dasselbe Ereignis starten; nicht wartende Sounds blockieren den Ablauf nicht.
- Nach dem Geschützabschluss setzt die neue Szene das Spiel fort. Ziel4 beendet den Abschnitt nach Squad-Aufnahme und Advisor-Dialogen. HUD und Missionsstatus zeigen Scorchs Beitritt; der Ereignishost setzt die nachfolgende Sprengladung auf Auto und gibt ihren Marker frei. Der Sprengbefehl selbst ist noch nicht bedienbar.
- Ältere RCPLAY01–06-Szenen behalten ihren Abschnittsabschluss. Ducken wird für die neue RCPLAY07-Begegnung aktiviert; native Eingabe erhält Bit64.

## Abnahme

Der Rust-Eingabelauf übernimmt den bisherigen nativen Eingabestrom ab der Landung und erweitert ihn um den Anschlussweg. Nach10448 normalen Eingaben erreicht er Ziel4 mit5 Abschüssen und20HP. Er prüft Scorchs Ankunft am Begrüßungspunkt, Intro-/Hinter-Bunker-Kontakt, Squad-Aufnahme, Zielabschluss, Aktivierungsereignisse, blockiertes Aufstehen, exakten Neustart und unverändertes statisches GPU-Paket. Der Geschützabschluss-Zeitpunkt bleibt beim Weiterlaufen einmalig gespeichert.

Windows-native ABI und Android-native x86_64 wiederholen denselben Eingabestrom bis zum Abschluss; alle13 Begegnungssamples werden genau einmal ausgelöst. Native Puffergrenzen, CPU-Bild und Neustart bestehen. Im Android-API35-Emulator bestehen zusätzlich APK-Dateiauswahl, Szenenladen, GPU-Darstellung, Touch-Schießen mit Originalwaffensound, Pause und Touch-Ducken.48 beobachtete Duck-Frames zeigen die vorgesehene Absenkung um28 Einheiten. Die APK enthält ARM64- und x86_64-Bibliotheken.1044 Workspace-Bibliothekstests einschließlich eines zusätzlichen Tests für parallele Originalskripte bestehen; abschließende Format-/Clippy-Prüfungen und gezielte rc-game-Tests sind Teil der Lieferbelege.

Belege: `playthrough/validation.json`, `native/validation.json`, `android/native-replay.json`, `android/validation.json`, `android/crouch-input-validation.json`, `validation-manifest.json`. Bilder: `playthrough/scorch-recruited.png`, `playthrough/crouch-passage.png` und Android-Screenshots.

## Verbleibende Grenzen

Der Begleiter verwendet einen begrenzten Controller für die beiden Originalbewegungen und die Begrüßung; allgemeine Squad-GoalQueue, Folge-/Kampfverhalten, Waffenattachment, Wiederbelebung und Befehle sind noch nicht integriert. Die Schlachtfeld-, brennenden Bunker- und Intro-Fabriken samt Fodder-Droiden werden noch nicht als vollständige zusätzliche Gegner-/Effektsimulation ausgeführt: ihre ausgewählten Ereignisse und Schadens-/Stopanforderungen bleiben im nachvollziehbaren Ereignisprotokoll. Das prozedurale SoundMultiple `ambPS_GEO_rockImpact` ist außerhalb des ausgewählten PCM-Fragments. Kein allgemeiner UnrealScript-/Factory-VM-Port oder vollständige Originalparität wird behauptet.

Der Android-Gesamtlauf verwendet die native Eingabe-ABI; eine vollständige manuelle Touch-Abnahme und ein Test der neuen Fassung auf einem physischen ARM64-Gerät bleiben offen. Bisherige begrenzte Kampf-, Explosions- und Physikadapter gelten weiter. Der spielbare Abschnitt schließt Ziel4 ab; Ziel5 „Command demolition maneuver“ und Ziel6 „Infiltrate Geonosian spire“ folgen.

## Reproduktion

Im Projektverzeichnis, mit `CARGO_INCREMENTAL=0`, Originaldaten unter `GAME_DATA`:

```powershell
cargo run --release --locked --offline -p rc-inspect --bin rc-rendezvous-assets -- "$GAME_DATA" analysis/mission5-playable --encounter
cargo run --release --locked --offline -p rc-inspect --bin rc-scorch-assets -- "$GAME_DATA" analysis/mission5-playable
cargo run --release --locked --offline -p rc-inspect --bin rc-rendezvous-route-check -- "$GAME_DATA" analysis/mission4-complete/geo_01a-gunner.rcplay analysis/mission5-playable/route-crouched --crouched
cargo run --release --locked --offline -p rc-inspect --bin rc-rendezvous-build -- "$GAME_DATA" analysis/mission4-complete/geo_01a-gunner.rcplay analysis/mission5-playable
cargo run --release --locked --offline -p rc-inspect --bin rc-rendezvous-scene-check -- analysis/mission5-playable/geo_01a-scorch.rcplay analysis/mission4-complete/native/input-replay.bin analysis/mission5-playable/playthrough
cargo build --release --locked --offline -p rc-native
python scripts/Validate-RendezvousAbi.py
scripts/Build-Android.ps1 -Abis arm64-v8a,x86_64
python scripts/Test-RendezvousAndroid.py
python scripts/Summarize-ScorchMission.py
```

Für den Android-Test muss vorher der kleine C-Eingabeprüfer `scripts/android-rendezvous-replay.c` mit dem vorhandenen x86_64-NDK und librc_native nach `analysis/mission5-playable/android-rendezvous-replay` gebaut sein. Die Szenenherstellung ist lokal und enthält aus der vorhandenen Spielinstallation abgeleitete Assets.
