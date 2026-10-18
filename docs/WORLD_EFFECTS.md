# Transparente Welteffekte für die Bacta-Fortsetzung

Fortsetzung vom 18.10.2026: [Sichtbare Emitter, gespeichertes Body-FadeOut, originale Missions-FX und Türbewegung](MISSION_PRESENTATION.md). Die folgenden Angaben dokumentieren den vorherigen Teilabschnitt.

Stand: 17.10.2026. Der gemeinsame CPU-/Android-Renderpfad kann jetzt transparente, unbeleuchtete Welteffekte darstellen. Die Bacta-Emitter sind **noch nicht angeschlossen**. Dieser Abschnitt schafft den benötigten Rendervertrag; er behauptet keine rekonstruierte originale Partikelsimulation oder freigegebene Geräteansicht.

## Verhalten

`rc_render::effects::TriangleEffect` liefert Original-/Adaptergeometrie, Textur/UV, einen RGBA-Multiplikator und einen expliziten Blendmodus:

| Modus | RGB-Ergebnis |
| --- | --- |
| Alpha | Quelle × Quellalpha + Ziel × (1 − Quellalpha) |
| SourceAlphaAdditive | Quelle × Quellalpha + Ziel |

Textur-RGBA und Effekt-RGBA werden multipliziert; RGB wird auf den darstellbaren Bereich begrenzt. Die endgültige Bildalpha bleibt deckend. Dreiecksnormalen beeinflussen die Effektfarbe nicht. Transparente Texel erhalten ihre tatsächliche Deckkraft, einschließlich Werten unter der bisherigen festen Alpha-Grenze. Gewöhnliche Welt-/Meshmaterialien behalten ihren bestehenden Renderpfad.

Die Reihenfolge ist Welt → normale Actors → gelieferte Effekte → Waffe → HUD. Effekte prüfen die vorhandene Welttiefe und verändern sie nicht. Nur vor der Waffe wird die Tiefe gelöscht. Die transparente Reihenfolge bleibt auch bei Texturwechseln erhalten; nur direkt aufeinanderfolgende gleichartige Effektbatches werden zusammengefasst. Der Aufrufer muss gegebenenfalls selbst sortieren. Der CPU-Referenzrenderer verwendet für Effekte eine halboffene Dreieckskante mit genauerer Kantenrechnung, damit ein Quad entlang seiner Diagonale nicht doppelt aufhellt.

Bis4.096 Effektdreiecke und insgesamt100.000 transiente Dreiecke sind zulässig. CPU-Eingänge prüfen endliche Positionen/UV, Farbwerte und Texturindizes. Es werden keine Partikelzustände, Zufallsströme oder Grafikhandles gespeichert. Ohne gelieferte Effekte bleibt die bisherige Frameversion erhalten.

## Android-Vertrag

`RCFRA003` erweitert `RCFRA002`: Nach dem unveränderten Header enthält jeder Batch `pass:u32, texture:i32, blend:u32, vertices:u32` und anschließend unverändert neun little-endian Floatwerte je Vertex: XYZ, UV, RGBA. Headerposition64 enthält die Batchzahl,68 die HUD-Rechteckzahl; native Java-Aufrufer stellen weiterhin40 Statusbytes voran.

| Pass | Blend | Tiefentest | Tiefenschreiben | Tiefenlöschung |
| --- | --- | --- | --- | --- |
|0 Actors|0|ja|ja|nein|
|1 Welteffekte|1 Alpha /2 additiv|ja|nein|nein|
|2 Waffe|0|ja|ja|einmal vor Beginn|

`FramePass` weist absteigende Passreihenfolgen, unzulässige Kombinationen und ungültige Texturindizes ab. Der bestehende Frameparser begrenzt Batch-/Vertexzahlen und prüft verkürzte/überzählige Daten. `GpuSceneView` setzt Blend-/DepthMask-Zustand für jeden Batch und vor HUD/nächstem Frame zurück. Separate Alpha-Blendingfaktoren erhalten die deckende Zielalpha. Alte `RCFRA001`-/`RCFRA002`-Frames bleiben lesbar; deren Waffenpass1 wird intern auf2 abgebildet.

Diese zwei Blendmodi sind explizite **Port-Renderregeln**. Die Zuordnung der originalen `PTDS_*`-Werte, die native Größen-/Farben-/Spawnrechnung, Spriteausrichtung, Eye-Transformation und Body-Skelettpositionen sind noch anhand des Originals anzuschließen. Insbesondere wird `PTDS_Brighten` nicht allein aufgrund seines Namens einem dieser Modi zugeordnet. Der native Partikeldraw-/Uploadnachweis in [D3D_PARTICLE_DRAW.md](D3D_PARTICLE_DRAW.md) ersetzt diese vorgelagerte Semantik nicht.

## Prüfungen

Fünf neue Rendererregressionen prüfen schwache Alpha-Werte, Verdeckung, ausbleibendes Tiefenschreiben, Waffentiefe, deckungsfreie Quadkanten beider Windungen, Near-Clipping, Nullalpha, Farbbegrenzung und ungültige Daten. Ein weiterer Spieltest liest das komplette erzeugte003-Paket und prüft Textur-/Blendreihenfolge sowie unveränderte RGBA-Werte und Simulationszustände. `FramePassCheck` prüft die Passregeln; `GpuFrameCheck` prüft den tatsächlichen Android-Frameparser mit neuen/alten Paketen, abgeschnittenen Daten und ungültigen Batchkombinationen.

`build.ps1 -Target Check` führt die Rust-/Passprüfungen aus. `build.ps1 -Target Android` kompiliert beide konfigurierten ABIs und führt zusätzlich `scripts/Test-GpuFrame.ps1` gegen den tatsächlichen Frameparser mit dem vorhandenen Android35-JAR aus. Das ist ein JVM-/Kompiliernachweis; ein GLES-Bildvergleich am Gerät bleibt Teil der abschließenden Android-Abnahme.

Arbeitsberichte: `build/transparent-effects/checks`, `android`, `lifecycle.json`. Die bestehenden Szenen-/Speicherformate und der letzte freigegebene Szenenpack bleiben unverändert. Nächster Schritt ist die belegte Originalmaterial-/Ausrichtungszuordnung und anschließend das Anschließen der Eye-Emitter mit korrekter Lebensdauer; Body-FadeOut folgt mit persistenter Partikellaufzeit.

Ergebnis: je1.138 Debug-/Release-Bibliothekstests, Materialimport-/Doc-/Clippy-/Java-/Buildvertrag sowie ARM64/x86_64-Kompilation und tatsächlicher Android-Frameparser bestehen. Der18.829-Eingabe-A-Lauf besteht mit allen fünf Bacta-Checkpointphasen, Pause/Fortsetzen und Soundregeln; `lifecycle.json` ist zum vorherigen `build/bacta-effects/lifecycle-final.json` byteidentisch. Ein neuer Szenenbau ist für diese reine Rendereränderung nicht erforderlich. Kein neues Release/Tag und keine Gerätesichtprüfung.
