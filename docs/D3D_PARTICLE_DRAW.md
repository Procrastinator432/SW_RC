# Weitere dynamische Engine-Aufrufer: Partikel und Vertex-Mesh

Stand: 2026-10-09. Fuenf weitere Draw-/Uploadpfade sind in `d3d_dynamic_draw::Caller` an die vorhandene Pool-, Ring-, Scratch- und Materialpasskette angeschlossen. Diagnoseadapter: `rc-d3d-particle-draw-check`. Diese Ausschnitte beginnen mit bereits erzeugten Quelldaten und ausgewaehlten Materialpasses; sie ersetzen noch keine vollstaendigen Emitter oder Meshrenderer.

| Aufrufer | Originalfunktion / Interfaceaufrufe | Drawvertrag |
| --- | --- | --- |
| Ribbon-Batch | URibbonEmitter::RenderParticles 104e2740; +88 104e3d67, +90 104e3d79, +94 104e3d9f | Art 5, Start Indexoffset, Primitive 2*Pairs, Min 0, Max 2*Pairs-1 |
| Spark-Batch | USparkEmitter::RenderParticles 1051b940; +88 1051bb1c, +8c 1051bb2c, +94 1051bb48 | Art 2, Start Vertexoffset, Primitive Sparks*Points, **Min und Max ffffffff** |
| Sprite-Batch | USpriteEmitter::RenderParticles 1051ece0; +88 1051ef70, +90 1051ef7f, +94 1051ef9c | Art 5, Start Indexoffset, Primitive 2*Sprites, Min 0, Max 4*Sprites-1 |
| Trail-Batch | UTrailEmitter::RenderParticles 1055c040; +88 1055c2d8, +90 1055c2e7, +94 1055c2fe | Art 5, Start Indexoffset, unabhaengige Primitiveanzahl, Min 0, Max Vertices-1 |
| Vertex-Mesh-Sektionen | UVertMeshInstance::Render 10560100; +88 105606e0, +90 105606ef, +94 105608ff | Art 5, Start Indexoffset + unsigned WORD FirstIndex, Primitiveanzahl aus aufgeloestem Sektionscount; Min/Max aus unsigned WORDs |

Alle DWORD-Produkte, Additionen und Subtraktionen laufen wie im Original um. Die rohen Nullwerte erzeugen in diesen Ausschnitten weiterhin einen Draw mit gegebenenfalls umlaufendem Maxvertex; vorgelagerte Partikelauswahl- und Sichtbarkeitsgates sind ausdrueckliche Eingangsvoraussetzungen. Pairs/Sparks/Sprites sind die am Uploadpunkt bereits aufgeloesten lokalen Counts, keine Behauptung ueber unveraenderte Funktionsparameter.

Die vier indizierten Pfade geben den Vertexoffset an die dynamische Indexbindung als Basis weiter. Die Draws verwenden deren Indexoffset. Sparks loesen vorher die statische Indexbindung mit Nullquelle/Basis null. Beide CPU-Ziele, Cache-/Poollisten, Resize-/Scratchzustand, Bindungen, komplette Deferred-Zustaende, Lichter, Zaehler und mutierte Materialpasses werden zusammen transaktional uebernommen. Fehler lassen Runtime und Request unveraendert.

## Mesh-Sektionsgrenze

`MeshSection` enthaelt `first_index`, `min_vertex`, `max_vertex` und `enabled_word` als u16 sowie den aufgeloesten u32-Primitivecount. Der originale DWORD-Countgate bei 105607e7 und WORD-Gate bei 105607f7 ueberspringen eine Sektion, wenn einer der Werte null ist. Jeder andere WORD-Wert, auch 8000/ffff, ist aktiv. Die Uploads erfolgen auch bei leerer Sektionsliste oder ausschliesslich uebersprungenen Sektionen. Die Reihenfolge bleibt erhalten; maximal 256 gelieferte Sektionen sind im sicheren Adapter erlaubt.

Die aeussere native Arrayiteration, Ganze-Mesh-Gates, Materialbeschaffung und Materialwechsel pro Sektion bleiben ausserhalb dieses Adapters. Mehrere aufgeloeste Sektionen verwenden hier dieselben gelieferten vorbereiteten Passbilder. Daher ist dies eine gepruefte Sektions-Draw-/Offsetroute mit gemeinsamen Materialpasses, noch kein kompletter Vertex-Mesh-Renderer. Geometrieerzeugung, Animation und Quelldatenfuellung bleiben ebenfalls offen.

## Nachweis und Regressionen

`Generate-D3DParticleDraw.py` verknuepft SHA256-gepruefte Pool- und Materialpassfixtures. 48 Sequenzen enthalten jeweils alle fuenf Aufrufer: **240 Anfragen, 240 Vertexuploads, 192 Indexuploads, 48 explizite Index-Unbinds, 257 Drawanforderungen und 1021 Materialpass-Durchlaeufe**. Davon entfallen jeweils 48 Draws auf die vier Partikeltypen und 65 auf Mesh-Sektionen. Weitere 18 Sektionen werden durch Count null und 10 durch WORD null gefiltert.

`Record-D3DParticleDraw.py` interpretiert **1724513 Originalinstruktionen** entlang dieser Engine- und bestehenden D3D-Koerper. Vollstaendige Ergebnisse und Zustandsbilder werden nach jeder Anfrage mit Rust verglichen. Die Quelldaten und COM-/Allocatorantworten bleiben synthetische ABI-Fixtures; die rohen Drawbereiche belegen keine passenden Live-Geometriegroessen. Unterschiedliche Meshmaterialien sind nicht abgedeckt.

Die gemeinsame Replay-VM liest `word ptr` jetzt korrekt mit 16 Bits. Drei ausdrueckliche Regressionen pruefen MOVZX ffff, CMP eines Null-WORDs mit benachbartem Nichtnull-WORD und CMP 8000. Der gesamte vorherige Fuenf-Aufrufer-Replay mit 1996505 Originalinstruktionen wurde nach dieser VM-Erweiterung erneut ohne Aenderungen an seinen historischen Berichten geprueft.

**12 neue Rusttests, insgesamt 807 bestandene Workspace-Tests.** Vierzehn sichere Fehlerfaelle pruefen unter anderem Caller/Index-Mismatch, fehlerhafte Uploads, ungueltige Passbilder, zu viele Meshsektionen und spaete Fehler nach den Uploads. Debug/Release sind bytegleich. Clippy mit `-D warnings`, Format- und Diffpruefung bestehen.

```powershell
python scripts/Generate-D3DParticleDraw.py
cargo run -p rc-inspect --bin rc-d3d-particle-draw-check -- analysis/reports/d3d-particle-draw.input.json analysis/reports/d3d-particle-draw.json
cargo run --release -p rc-inspect --bin rc-d3d-particle-draw-check -- analysis/reports/d3d-particle-draw.input.json analysis/reports/d3d-particle-draw-release.json
cargo test --workspace *> analysis/reports/d3d-particle-draw-tests.log
python scripts/Record-D3DParticleDraw.py
```

Berichte: `analysis/reports/d3d-particle-draw*`; Evidenzschluessel `d3d_particle_draw_validation`. Originalexporte: `engine-particle-dynamic.c`, `engine-{ribbon,spark,sprite,trail,vertmesh}-upload.asm`, `engine-vertmesh-draw.asm`. Gemeinsamer Vertrag: `D3D_DYNAMIC_DRAW.md` und `D3D_POOLS.md`.

Echte Allocator-/COM-/GPU-Ausfuehrung, vollstaendige Emitter-/Meshgeometrie und spielbare Integration bleiben offen. Android wird wie vereinbart zum Schluss geprueft.
