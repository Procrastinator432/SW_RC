# HardwareShader-Lader, Renderpass und Snapshot-Mesh-Prüfung

Stand 2026-10-09. Drei Aufgaben abgeschlossen: gemeinsamer Lader für die serialisierten HardwareShader-Eingaben, wiederverwendbarer CPU-Hologramm-Pass und Verbindung der originalen Szenenspeicherlayouts mit einer vollständigen Mesh-Zeichnung.

`Assets::hardware_shader_inputs` lädt VertexShaderText, PixelShaderText, VS-/PS-Konstanten und feste Texturplätze zusammen. Er erhält Nullreferenzen und Array-Indizes, qualifiziert Paketpfade und weist doppelte Texturplätze bzw. fehlende/ungültige Programmtexte ab. Er verlangt einen HardwareShader und liest ausdrückliche Instanzdaten; Klassen-Defaults, Wrapper-Overrides und native Sampler-/Renderzustände werden hier nicht ergänzt.

`rc_render::hologram_pass::HologramPass` verbindet die vorhandenen Vertex-/Pixel-Interpreter mit dem diagnostischen Dreiecksraster. `prepare` verlangt initialisierte 96-/8-Register-Bänke und wertet alle drei Vertices vor einer Änderung des Renderziels aus. Es nutzt sämtliche gespeicherten Registerwörter, auch erhaltene Werte außerhalb des gecachten Uploadzählers. Der Pass verändert keine Konstanten. `shade` verwendet DynamicHolograms Ausgabelayout T0.x, T1.xy, T2.xy und D0.rgba; T0.y bleibt ausdrücklich null. `draw` übernimmt Tiefe, Alpha-Test und Blendmodus vom Aufrufer. Fehler während der Fragmentauswertung erhalten bereits geschriebene Zielpixel. Dies ist ein begrenzter CPU-Diagnosepass, kein allgemeiner HardwareShader-Backend oder optimierter GPU-Renderer.

`rc-snapshot-draw-check` führt alle 256 ursprünglichen synthetischen Szenensnapshots durch den Speicheradapter und die Original-DynamicHologram-Konstantenbindungen. Dabei bleiben beide Bänke und Flicker-Zustand erhalten; 128 Updates gelingen und 128 treffen auf die zuvor verifizierten nativen Lichttabellen-Lücken. Die erfolgreichen Snapshots 251 und 255 zeichnen jeweils alle 3500 Dreiecke des rekonstruierten CloneCommando-LOD0-Streams. Die gezeichneten Konstantenbänke stimmen exakt mit den unabhängig geprüften Snapshot-Ergebnissen überein; nach dem Dispatch werden keine Register überschrieben.

Die drei Texturen kommen unmittelbar aus den Original-HardwareShader-Plätzen: GreyHoloFade an 0 und 1, NoiseDigital an 2. Insbesondere wird an Platz 1 kein CloneCommando-Diffuse angenommen. Die native HsHologram-Wrapper-Verbindung zu Textur-/Farb-Overrides bleibt offen.

Ausdrückliche Diagnoseannahmen: anfängliche Register null außer c17=.25, portable RSQRT-Saatpolitik, um den Meshmittelpunkt normalisierte Eingabepositionen mit Faktor 1.5 zur Sichtprüfung, 128×128 Ziel, nächster Texel mit Wiederholung, halbe Pixelzentren, LessEqual-Tiefentest ohne Tiefenschreiben, Float-Alpha >0 und SourceAlpha/Additive-Blending. Die originalen Szenensnapshots sind synthetisch. Diese Einstellungen und Ergebnisse belegen keine Original-/Android-Rastergleichheit oder Live-Szenenanbindung.

`Record-SnapshotDraw.py` prüft Programmtexte, Bindungen und Texturplätze unabhängig aus den Originalpaketen, vergleicht die Bänke mit dem vorherigen Snapshot-Bericht, berechnet sämtliche 21000 Vertex-Ausgaben und alle 32768 Bildpixel separat nach und erzeugt zwei PNG-Vorschauen. Debug-/Release-Berichte sind bytegleich. Zwei neue Tests prüfen erhaltene Register jenseits des Zählers und die Ablehnung nicht initialisierter Bänke/unvollständiger Vertex-Ausgaben. **589 Workspace-Tests**, Clippy mit `-D warnings`, Format- und Diffprüfung bestanden.

Artefakte: `analysis/reports/snapshot-draw{,-release,-validation}.json`, `snapshot-draw-tests.log`, `snapshot-mesh-0.png` und `snapshot-mesh-1.png`. Die Bilder sind diagnostische Mesh-Ausschnitte mit originalen Shader-Eingaben; die bisherigen separat überschriebenen Vorschauen bleiben unverändert erhalten.

```powershell
cargo run -p rc-inspect --bin rc-snapshot-draw-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/shader-snapshots.input.json analysis/reports/skeletal-draw.bin analysis/reports/snapshot-draw.json
cargo run --release -p rc-inspect --bin rc-snapshot-draw-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/shader-snapshots.input.json analysis/reports/skeletal-draw.bin analysis/reports/snapshot-draw-release.json
cargo test --workspace *> analysis/reports/snapshot-draw-tests.log
python scripts/Record-SnapshotDraw.py
```

Offen: vollständige Wrapper-/Renderzustands-/Sampleranbindung, Live-Szenenerzeugung und spielbare Engine. Allgemeine Diffuse-Abdeckung bleibt 271/289. Android-Prüfung weiterhin zum Schluss.

Korrektur 2026-10-09: Die hier aufgezeichnete VS-vor-PS-Reihenfolge mit getrennten persistenten Banken ist eine Diagnose-Fixture. Der Originalaufruf nutzt PS-vor-VS und einen gemeinsamen Scratchpuffer mit getrennten Geraeteuploads; siehe [HardwareShader-Uebergabe](HARDWARE_HANDOFF.md). Der Originaltreiber importiert Direct3D 8.
