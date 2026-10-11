# Erster spielbarer Abschnitt: geo_01a

Stand: 10. Oktober 2026, Android 0.8.0. Ein begrenzter Spielabschnitt auf der Originalkarte ist spielbar: bewegen, zielen, springen, schießen, nachladen, Gegner ausschalten, Exfil erreichen, verlieren und neu starten. Der Abschnitt läuft im gemeinsamen Rust-Spielkern und ist über normale Android-Touch-Eingaben geprüft.

Das ist ein spielbarer Prototyp mit vorläufigen Roboter-/Waffenmodellen und eigenen Kampf-/Begegnungsregeln. Originale Kampagnen-Scripts, CTGame-Waffenverhalten, Gegner-KI, Squad, Gespräche und die vollständige Spielparität sind weiterhin offen. Der bisher rekonstruierte Direct3D-Aufrufplan wird hier noch nicht auf einer echten GPU ausgeführt; die ursprüngliche Version 0.8 verwendet den Software-Renderer. Android 0.9.0 zeichnet dieselbe Szene zusätzlich über einen eigenen OpenGL-ES-Adapter; vollständige Original-Materialparität bleibt offen.

Fortsetzung 0.8.1: Referenzsicherung, Eingabe-Replay, Laufzeitmessung und Emulator-Mehrfinger-/Fokusprüfung sind abgeschlossen. [Aktueller Prüfstand und Messwerte](BASELINE_AND_PROFILING.md). Die ursprünglichen 0.8-Nachweise unten bleiben als Referenz erhalten.

Fortsetzung 0.9.0: Standardmäßig wird die GPU-Ausgabe verwendet. **GPU / CPU** wechselt den Grafikpfad. Das vorhandene Szenenpaket bleibt unverändert kompatibel. [GPU-Aufbau, Messwerte und Nachweise](GPU_SCENE.md).

Fortsetzung 0.10.0: **Animierte Originalgegner und DC-17m** sind in der separaten Datei `analysis/milestone2/geo_01a-animated.rcplay` enthalten. Diese Datei für den neuen Stand verwenden; die nachstehend beschriebene alte Szene bleibt Vergleichsreferenz. [Neue Szene, Anleitung, Herkunft und Grenzen](ANIMATED_SCENE.md).

## Starten

1. `android/app/build/outputs/apk/debug/app-debug.apk` installieren. Die APK enthält ARM64 und x86-64, Android 8 oder neuer.
2. `analysis/playable/geo_01a.rcplay` auf das Gerät kopieren, beispielsweise in Downloads.
3. In **RC Port** die Schaltfläche **Spielabschnitt geo_01a öffnen** wählen, anschließend **Szene öffnen** und `geo_01a.rcplay` auswählen.
4. Drei Drohnen ausschalten und danach zur grünen Exfil-Markierung gehen. **Neustart** setzt Spieler, Munition, Gegner und Ziel zurück.

Links befindet sich der Bewegungsstick. Ziehen im freien rechten Bildschirmbereich dreht den Blick. **Feuer**, **Sprung**, **Laden**, **Pause/Fortsetzen** und **Neustart** sind separat erreichbar. Die Eingabe verarbeitet mehrere Pointer mit festen Rollen; gleichzeitige Mehrfingerbedienung auf einem physischen Gerät ist noch nicht gesondert geprüft.

Für Tastatureingabe: WASD/Pfeile bewegen, Leertaste springt, linkes Strg schießt, R lädt, linkes Shift geht langsam, Enter startet neu. Die bisherige Diagnosekamera bleibt über die Startansicht erreichbar.

## Inhalt und Herkunft

| Bestandteil | Umfang / Regel |
|---|---|
| Originalumgebung | 134.790 Dreiecke, 334 vorbereitete StaticMesh-Instanzen, 41 Textureinträge |
| Körperkollision | Level-referenziertes Weltmodell plus 254 nichtleere statische Actor-Formen; zusammen 255 Collider / 1.505 Hüllen |
| PhysicsVolumes | Zwei klassifizierte Originalvolumes mit Einstellungen und originalen BSP-Knoten/Actor-Transformationen |
| Spielerprofil | Originaldefaults: Radius 40, Halbhöhe 84, GroundSpeed 450, AccelRate 1024, JumpZ 475 sowie Richtungs-/Gehfaktoren |
| Physik | Vorhandene Walking-/Falling-Rechnung mit eigenem statischem AABB-, Boden- und Stufenadapter |
| Simulationszeit | Feste Schritte von 1/60 s; höchstens acht Schritte pro Bild, längerer Rückstand wird verworfen |
| Begegnung | Drei kollisionsgeprüfte, unterstützte und vom Start aus erreichbare Positionen; eigene Robotergeometrie |
| Kampf | Eigene Hitscan-Regel bis 8.000 Einheiten, 30 Schaden, 20 Schuss/Magazin, 200 Reserve, 1,2 s Nachladen |
| Gegner | 60 HP, kurze kollisionsgeprüfte Annäherung, Sichtkontakt über statische Kollisionshüllen, 5 Schaden pro Schuss |
| Abschluss | Alle drei Gegner tot und Spieler innerhalb 140 Einheiten um die Exfil-Markierung |
| Feedback | HUD, Treffer-/Schadensanzeige, Mündungs-/Einschlagmarkierung und einfache synthetische Android-Töne |

Die zuerst aufgelöste `PlayerStart0`-Position wird beim Export für 120 feste Schritte auf den realen Boden abgesenkt. Der hier verwendete Startmittelpunkt ist `(2101.976806640625, 1829.5628662109375, -2603.49951171875)`. Die originale Anchor-Höhe beträgt `-1997.7979736328125`. Dieser vorbereitete Bodenspawn ersetzt für den Prototyp die noch fehlende originale Spawn-/Cutscene-Logik. Yaw stammt aus dem Start-Actor, der anfängliche Pitch ist horizontal. Augenhöhe ist eine eigene Regel: 0,65 der Körperhalhöhe, hier 54,6 Einheiten.

Die Kollisionsvollständigkeit gilt für das Level-Weltmodell und die klassifizierten statischen Mesh-Actors am gespeicherten Pose. Bewegte Türen, Mover, Skelettkörper, Script-gesteuerte Hindernisse, Wasser und originale native Kontakt-/Backoff-Parität werden nicht behauptet. `Engine.ConvexVolume` erscheint in der bestehenden Namensheuristik des Volume-Audits, gehört nicht zu den zwei hier klassifizierten PhysicsVolumes. Die vorbereitete sichtbare Szene übernimmt die bisherigen dokumentierten Auslassungen von `rc-scene`; sie bildet keine vollständige Original-Level-Laufzeit ab.

## Architektur

`rc-game` besitzt Spieler, Gegner, Kampflogik, Zielzustand und feste Zeitschritte. Es verbindet vorhandene `rc-package`-Bewegungs-/Kollisionsmodule mit `rc-render`. Ein fehlgeschlagener Tick setzt den gesamten eingereichten Frame zurück. Kurze Sprung-, Feuer- und Nachladeimpulse bleiben bis zum nächsten Physikschritt erhalten. Pause/Fokusverlust löscht Eingaben und Zeitrückstand.

`rc-inspect::play_scene` liest Originaldefaults, Weltmodell, Actor-Transformationen und Mesh-Box-Dispatch wie die vorhandenen Kollisionsproben. `rc-play-build` wählt die Begegnung und erzeugt ein `RCPLAY01`-Bundle. Es enthält begrenzte JSON-Metadaten und einen bestehenden RCSC-Snapshot mit Geometrie/Texturen. Binäre f64-Werte bleiben durch `serde_json/float_roundtrip` beim Metadaten-Roundtrip erhalten. Nicht aufgelöste Collider-/Volume-Formen brechen den Export ab. Das Bundle ist ein Szenenpaket, kein Spielstand.

`rc-native::play` bietet unabhängige Game-Handles mit Create/Advance/Draw/Status/Pause/Restart/Release. Die JNI-Brücke ruft denselben Kern auf. `PlayActivity` verwaltet Dateidialog, Touch-Rollen, Bitmap-Anzeige und Lebenszyklus; sie gibt keine Treffer, Positionen oder Siegantworten vor. Alle Spielzustände entstehen in Rust.

Der Renderer zeichnet Welt und vorläufige Actors in denselben Tiefenpuffer. Kamera und Schussrichtung stimmen überein; Rechtsbewegung folgt der tatsächlichen Bildschirmrichtung. Für den Spielmodus gilt eine eigene Nah-Ebene von vier Welteinheiten. Die bisherigen Diagnose-Renderaufrufe behalten ihre ursprünglichen Einstellungen.

## Reproduzieren

Im Projektordner:

```powershell
cargo run --release --offline -p rc-inspect --bin rc-play-build -- `
  'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' `
  analysis/scenes/geo_01a-textured/world.rcscene analysis/playable/geo_01a.rcplay
cargo run --release --offline -p rc-inspect --bin rc-play-check -- `
  analysis/playable/geo_01a.rcplay analysis/playable/validation
cargo build --release --offline -p rc-native
python scripts/Validate-PlayableAbi.py
& ./scripts/Build-Android.ps1 -Abis arm64-v8a,x86_64
& ./scripts/Start-TestEmulator.ps1
python scripts/Test-PlayableEmulator.py
```

Das Python aus der Codex-Runtime enthält Pillow für die Screenshot-Prüfung. `rc-play-build` benötigt die lokal erzeugte texturierte Szene aus dem bisherigen Szenenexport. Originalassets bleiben in der lokalen Installation beziehungsweise in abgeleiteten lokalen Szenenpaketen; sie werden nicht in die APK eingebettet.

## Prüfung und Belege

- Rust: 930 Workspace-Tests bestanden, Clippy und Formatprüfung sauber. Tests zu festen Zeitschritten, kurzen Eingaben, Sprung-Latch, Körper-/Schussblockierung an Wänden, Nachladen, Sieg, Niederlage, exaktem Neustart, Pause, Rückstandslimit, Fehler-Rollback, Bundle-Fehlern, Kamerarichtung und gemeinsamer Tiefenverdeckung. Debug/Release liefern identische Spielzustände und fünf bytegleiche gerenderte Frames.
- Originalkarte: `rc-play-check` durchläuft Sprung/Landung, sechs Treffer/drei Kills, Exfil-Sieg, Gegnerbeschuss/Niederlage und exakten Neustart. Belege in `analysis/playable/validation/`.
- C-ABI: echtes geo_01a-Bundle; kurze Puffer, Canary-Wörter, ungültige Eingaben, Pause, Sprung, identische Reset-Pixel und ungültige freigegebene Handles. `analysis/playable/abi-validation.json`.
- Android API 35 / x86-64: normales Dateiauswählen, Touch-Zielen/Schießen, drei Kills, Touch-Bewegung bis zum Sieg, Nachladen, Touch-Sprung, Gegnerbeschuss/Niederlage, Neustart und identische pausierte Screenshots. Beschädigte Szenendatei abgewiesen, danach normales erneutes Laden erfolgreich. Keine injizierten Spielzustände. `analysis/playable/android/validation.json`, Screenshots und Logcat-Belege; Crash-Puffer ohne App-Absturz.
- ARM64 und x86-64 bauen; APK-Signatur und 16-KiB-ELF-/APK-Ausrichtung geprüft. Ein physischer ARM64-Lauf ist noch offen. Zusammengeführte Hash-/Buildnachweise: `analysis/playable/validation-manifest.json`.

Dieser Meilenstein bestätigt einen durchspielbaren ersten Abschnitt. Für eine originalgetreue Republic-Commando-Szene folgen insbesondere originale Gegner-/Waffenmodelle samt Animation, Kampflogik, Missionstrigger, Audio und die weitere Renderer-Anbindung.


Fortsetzung 0.11.0: Separate [Kampfversion mit Originaldefaults und Geräuschen](COMBAT_SCENE.md). Szene `analysis/milestone3/geo_01a-combat.rcplay`; M3 ist für native KI-/Projektil-/Schadensparität noch in Arbeit.
