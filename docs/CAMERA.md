# Kamera, Stand 0.5

Der native Android-Viewer bietet jetzt Orbit-Zoom und eine frei bewegliche Diagnosekamera. Die texturierte Originalszene `geo_01a` kann aus der Nähe untersucht werden. Das ist noch keine Spielerbewegung: Es gibt keine Kollision, Schwerkraft, Actor-Steuerung oder Spiellogik.

## Bedienung

- Szene öffnen; Ziehen auf dem Bild dreht die Kamera.
- `Zoom +` / `Zoom −`: Orbit-Abstand verändern, Faktor 1,4 pro Schritt; Bereich 0,5 bis 32.
- `Freie Kamera`: aktuelle Position und Blickrichtung übernehmen. Ziehen verändert dann den Blick bei fester Position.
- `Vor`, `Zurück`, `Links`, `Rechts`, `Hoch`, `Runter`: pro Tippen 0,04 Szenenradien bewegen. Vor/Zurück folgt der Blickrichtung; Hoch/Runter folgt der Welt-Z-Achse. Zoom-Tasten bewegen im freien Modus vorwärts/rückwärts.
- `Übersicht` oder erneutes Tippen auf den freien Kameramodus: Ausgangsposition, Zoom und Blick zurücksetzen. Das Laden einer anderen Szene setzt ebenfalls zurück.

Die Bewegungsschritte sind bewusst für die Dateninspektion relativ zu den Szenengrenzen gewählt. Sie sind keine rekonstruierten Laufgeschwindigkeiten des Originalspiels. Es gibt noch keine gedrückt gehaltenen Bewegungstasten, Mehrfinger-Zoom oder Controller-Eingabe.

## Implementierung

`rc-render::View` erweitert den bestehenden Orbit-Renderer um eine explizite Position in Szenenradien relativ zum Mittelpunkt. Yaw und Pitch behalten die bisherige Orientierung bei; der Blick zeigt entgegen dem aus diesen Winkeln berechneten Richtungsvektor. Die freie Kamera übernimmt die Position des aktuellen Orbits ohne beabsichtigten Blicksprung.

Vor der perspektivischen Projektion werden Dreiecke an der Nah-Ebene geschnitten (0,002 Szenenradien). Kamerakoordinaten und UVs werden an Schnittpunkten interpoliert; ein entstandenes Viereck wird in zwei Dreiecke zerlegt. Dadurch verschwinden teilweise sichtbare Flächen bei Annäherung nicht mehr vollständig. Perspektivische UV-Interpolation, Tiefenpuffer und Alpha-Test bleiben aktiv.

Die neue C-ABI `rc_scene_draw_view` akzeptiert Zoom und freie Position. Die alte `rc_scene_draw` bleibt kompatibel. JNI übergibt die Kamera an Rust. Java fasst Rendering-Anfragen zusammen und verwirft Ergebnisse einer zuvor geladenen Szene über eine Generation. Rendering bleibt ein CPU-Software-Renderer mit 640 × 400 Pixeln im Android-Viewer.

## Geprüft am 6. Oktober 2026

- 20 Rust-Tests und Clippy über alle Targets erfolgreich. Der neue Test prüft Schnittpositionen/UVs, teilweise sichtbare Flächen, ungültige Kameraposition und vollständig hinter der Kamera liegende Geometrie.
- Android-Build 0.5.0-camera-viewer für ARM64 und x86-64; APK-v2-Signatur und 16-KiB-zipalign erfolgreich.
- API-35-x86-64-Emulator `RC_Test_API35`: Szene mit 134.790 Dreiecken und 41 Texturbelegungen geladen; vier Zoom-Schritte, freie Kamera, alle sechs Bewegungsrichtungen, freies Umsehen und Reset geprüft.
- Bildvergleich des tatsächlichen Renderbereichs: Zoom verändert 346.733 Pixel; Vor/Zurück, Rechts/Links und Hoch/Runter ergeben nach dem jeweiligen Paar exakt das Ausgangsbild; Reset ebenfalls exakt. Der Übergang vom Orbit zur freien Kamera verändert 74 Pixel durch Rundung der separat in Java berechneten Position.
- Beschädigte Szene wird abgewiesen; anschließend lässt sich die Originalszene erneut laden. Crash-Puffer leer. Physischer ARM64-Test steht aus.

Belege: `analysis/emulator/camera-validation.json`, `camera-*.png`, UI-XMLs und `camera-crash-log.txt`; kompakte Nahansicht in `camera-near-view.png`. Originalassets werden weiterhin aus den lokal vorbereiteten Szenendateien geladen, nicht mit der APK verteilt.

## Nächster Schritt

Level-Referenzen und Klassendefaults auflösen sowie PlayerStart und Kollisionsdaten rekonstruieren. Erst danach lassen sich eine an das Original angelehnte Spielerposition und Bewegung prüfen. Shader-Effekte, Animation, Gegner und Squad-System bleiben offen.

## Stand 0.6

Original-Startpunkte aus CTM und Snapshot-Version 3 lassen sich anwählen; keine Pawn-Augenhöhe oder Spawn-Logik. 23 Tests/Clippy, ARM64-/x86-64-Build, Signatur/zipalign und Emulatorprüfung einschließlich exakter Rückkehr/Reset und Fehler-Recovery bestehen. Details und Testskripte: [LEVELS_AND_COLLISION.md](LEVELS_AND_COLLISION.md).
