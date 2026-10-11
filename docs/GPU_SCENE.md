# geo_01a mit GPU-Ausgabe — Meilenstein 1

Stand: 10. Oktober 2026, Android 0.9.0-gpu-scene (VersionCode 11). Der Spielkern liefert jetzt eigene, unveränderliche Renderframes. Android zeichnet die vorhandene Welt, Basistexturen, vorläufige Gegner und das HUD über OpenGL ES 2. Die physische ARM64-Leistungsabnahme bleibt offen.

## Ausprobieren

Die neue APK ist `android/app/build/outputs/apk/debug/app-debug.apk`. Das bisherige `analysis/playable/geo_01a.rcplay` bleibt kompatibel und unverändert. In RC Port **Spielabschnitt geo_01a öffnen → Szene öffnen** wählen. Die Schaltfläche **GPU / CPU** wechselt den Grafikpfad auch bei pausiertem Spiel. Bei einem gemeldeten GL-Fehler schaltet der Host auf CPU-Ausgabe um. Steuerung, Kampfregeln und Szeneninhalt sind in [PLAYABLE_SCENE.md](PLAYABLE_SCENE.md) beschrieben.

Die interne Auflösung bleibt 640 × 400. Das Ergebnis wird unter Beibehaltung des Seitenverhältnisses in die Oberfläche eingepasst. Touch-Flächen und Empfindlichkeit richten sich wie bisher nach der tatsächlichen View-Größe. Das Szenenpaket wird separat geladen; Originalassets sind nicht in der APK enthalten.

## Daten, Threads und Ressourcen

`Game::render_frame` erstellt eine eigene `RenderFrame`-Beschreibung aus Kamera, transienten Dreiecken und geordneten HUD-Rechtecken. Text besteht aus denselben Glyphen/Rechtecken wie bisher. Die Beschreibung hält weder einen Game-Borrow noch Grafik-Handles. Nachfolgende Simulation verändert einen bereits erzeugten Frame nicht. `render_cpu` verarbeitet dieselbe Beschreibung mit dem bisherigen Tiefenrenderer; fünf historische Referenzbilder und die gespeicherten Gameplay-Zustände bleiben exakt erhalten.

`rc-render::gpu` erstellt den statischen Welttransfer: Positionen, UVs, vorhandene einfache diffuse Beleuchtung und RGBA-Basistexturen. Dreiecke werden nach Textur gruppiert. Die native ABI `rc_game_graphics` exportiert Welt oder Frame mit Größenabfrage und geprüfter Pufferkapazität. JNI überträgt im GPU-Modus keine fertig rasterisierten Weltpixel. Der Startframe enthält 42.968 Bytes an Grafikdaten gegenüber zuvor 1.024.000 Bytes an ARGB-Pixeln. Der einmalige Welttransfer umfasst 15.217.456 Bytes.

Der bestehende einzelne Worker besitzt die Game-Handles und übernimmt Eingaben, Simulation und Frameexport. Ein Frame gelangt über den UI-Thread in einen einzelnen Latest-Frame-Slot. So wächst bei langsamerer Grafik keine Frame-Warteschlange. Alle GL-Aufrufe, Namen und Uploads gehören ausschließlich dem GLSurfaceView-Renderthread. Die CPU-Beschreibungen sind verwaltet; ein wiederverwendbarer direkter Stagingpuffer bedient Textur-/Vertexuploads. Große Direktpuffer pro Szene oder Frame würden außerhalb des sichtbaren Java-Heaps Druck auf den nativen Speicher erzeugen.

Welt-VBOs und Texturen werden pro Szenenwechsel oder Grafikkontext einmal hochgeladen. Pro Frame folgen Kamera-Uniforms, kleine Actor- und HUD-Streams. Ein 640×400-FBO hat einen gemeinsamen Tiefenpuffer für Welt und Actors; HUD und Präsentation folgen ohne Tiefentest. Nah-Ebene ist vier Welteinheiten, Fern-Ebene zehn Millionen. OES_depth24 wird bei Verfügbarkeit verwendet, sonst 16 Bit. UV-Wiederholung erfolgt im Shader, mit Nearest-Sampling, Alpha-Verwerfung unter 0,5 und ohne Backface-Culling. Das sind Adapter für die bestehende Prototypdarstellung; die vollständige originale Direct3D-/Materialsemantik wird damit nicht behauptet.

Vor Pause werden GL-Ressourcen auf dem Renderthread ausdrücklich freigegeben. Der EGL-Kontext wird nicht konserviert. `onSurfaceCreated` erstellt Shader, FBO und Streams neu; unveränderliche CPU-Weltdaten liefern neue Weltressourcen. Szenenwechsel löscht alte VBOs/Texturen. Der Host löscht weiterhin Eingaben und Simulationsrückstand bei Pause/Fokusverlust. Neustart ändert den Spielzustand, ohne die Welt neu zu laden.

## Prüfung und Grenzen

Die Nachweise liegen getrennt von den unveränderten Meilenstein-0-Artefakten in `analysis/milestone1/`. Der GPU/CPU-Vergleich prüft die echte Android-GL-Ausgabe gegen separat erzeugte Desktop-CPU-Pixel derselben Startansicht. Vor der Messung festgelegte Toleranz: mittlerer Kanalfehler unter 2 von 255 und mehr als 98 % der Kanäle höchstens zwei Stufen abweichend. Gemessen wurden 0,456 und 99,417 %. Unterschiede an Dreieckskanten und Texturgrenzen sind erwartbar; pixelgenaue Rasterizerparität wird nicht behauptet.

`Test-PlayableGpu.py` prüft außerdem ABI-Canaries, Backend-Wechsel, einen tatsächlichen Kontextneuaufbau nach Home/Rückkehr und drei erneute Szenenladungen mit Speicherproben. Der vollständige UI-Spieltest prüft Zielen, Schießen, drei Kills, Nachladen, Exfil, Sprung, Niederlage, Neustart, Pause, beschädigte Datei und Wiederherstellung. Der getrennte Input-Probe injiziert vier echte Android-Pointer samt CANCEL/Fokusverlust, ohne Spielzustände zu setzen.

Gemessen wird auf dem API-35-x86-64-Emulator mit Software-GPU-Treiber, Rust Release und Java Debug. GL-Aufrufzeit ist CPU-Submission, keine GPU-Timerquery oder tatsächliche Display-Präsentation. Die kurze Speicherprobe ersetzt keinen Dauer-/Lecktest. Die zehnminütige physische ARM64-Abnahme mit p95 ≤33,3 ms, echte Touch-Hardware und thermisches Verhalten bleiben offen. Eine weitere Kollisionsoptimierung wird erst durch ein neues Profil begründet.

Die finale Version besteht 936 Workspace-Tests, Clippy, Format-/Diff-Prüfung, den unveränderten 1.002-Kommandos-Replay und die C-ABI-Prüfung. Beide vollständigen Android-Spielabläufe bestehen, ebenso vier gleichzeitige Pointer, CANCEL, Fokusverlust und Dialogabbruch auf dem GPU-Pfad. APK-Signatur, ZIP-Ausrichtung und alle vier ARM64-/x86-64-ELFs besitzen die geprüfte 16-KiB-Ausrichtung. Zusammengeführter Nachweis: `analysis/milestone1/validation-manifest.json`.

| Finale Emulatorprobe, 640 × 400 | GPU Median / p95 | CPU Median / p95 |
|---|---|---|
| Abstand aktiver Frameübergaben | 17,49 / 19,45 ms | 50,28 / 67,78 ms |
| JNI-Worker einschließlich Simulation/Export bzw. CPU-Zeichnung | 2,24 / 6,41 ms | 44,28 / 55,14 ms |
| GL-CPU-Aufrufzeit einschließlich Draw-Submission | 3,93 / 7,86 ms | — |
| CPU-Szenenrasterung | — | 38,29 / 45,89 ms |

Es liegen 1.223 GPU- und 404 CPU-Hostframes aus jeweils einem vollständigen UI-Spieltest vor. Die beiden letzten Tabellenzeilen messen verschiedene Stufen und ergeben keinen direkten GPU-Ausführungs-Speedup. Tatsächliche Display-Präsentation ist nicht erfasst. In beiden Läufen beträgt verworfene Simulationszeit null. Nach drei erneuten Ladungen liegen die PSS-Proben bei etwa 97–100 MiB; die letzten nativen Heap-Proben bleiben bei etwa 55,6 MiB. Die Untersuchung fand zunächst ein Wachstum um ungefähr einen Welt-VBO-Satz pro Kontextwechsel; die ausdrückliche GL-Freigabe vor Pause beseitigt dieses Wachstum in der wiederholten Probe.

## Reproduzieren

Im Projektordner; `python` muss auf eine Installation mit Pillow zeigen:

```powershell
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo fmt --all -- --check
cargo build --release --locked --offline -p rc-native
cargo run --release --locked --offline -p rc-inspect --bin rc-play-replay -- check analysis/playable/geo_01a.rcplay analysis/milestone0/replay.json analysis/milestone1/replay-check.json
./scripts/Build-Android.ps1 -Abis arm64-v8a,x86_64
python scripts/Validate-PlayableAbi.py --output analysis/milestone1/abi-validation.json
python scripts/Test-PlayableGpu.py
python scripts/Test-PlayableInput.py --output analysis/milestone1/input
python scripts/Test-PlayableEmulator.py --profile --output analysis/milestone1/android
python scripts/Test-PlayableEmulator.py --profile --software --output analysis/milestone1/cpu-android
python scripts/Summarize-PlayableGpu.py
```

Android-UI-Prüfungen nacheinander ausführen. Der Input-Test erwartet eine bereits geladene, pausierte Szene mit aktiviertem `rcProfile`. `Test-PlayableGpu.py` hinterlässt genau diesen Zustand. Referenzbild-Readback wird nur durch den Test-Intent `rcGpuCapture` eingeschaltet; normale Starts und Leistungsproben enthalten diesen synchronen Readback nicht.

Nächster Inhaltsschritt ist [Meilenstein 2](CONTINUATION_PLAN.md): einen passenden Originalgegner und die Spielerwaffe vollständig auflösen und ihre vorhandene Pose-/Skinningstrecke an Spielinstanzen anschließen.
