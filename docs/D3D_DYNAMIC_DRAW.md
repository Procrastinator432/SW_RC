# Engine-Aufrufer mit dynamischen Uploads und Draws verbinden

Stand: 2026-10-09. Fuenf zusammenhaengende Aufruferpfade abgeschlossen in `crates/rc-package/src/d3d_dynamic_draw.rs`; Diagnoseadapter `rc-d3d-dynamic-draw-check`. Die neue Runtime verbindet die Pool-/Scratch-/Ringkette aus `D3D_POOLS.md` mit der vollstaendigen bestehenden Materialpass- und Drawplanung.

## ABI und gemeinsame Verbindung

Die D3DDrv-Vtable beginnt bei 1006f8d8; ihre Verweise werden durch Konstruktorreferenzen bei 1000366b und 10026736 und die originalen PE-Worte belegt. Interface+88 zeigt auf dynamische Vertexbindung 10020540, +90 auf dynamische Indexbindung 10020a90, +94 auf Draw 10020cb0 und +8c auf statische Indexbindung 10020840. Diese Interfaceoffsets sind von den tieferen COM-Gerateoffsets zu unterscheiden.

Bei den drei indizierten Aufrufern wird der Rueckgabewert des Vertexuploads direkt als Basisvertex an den Indexupload weitergegeben. Dessen Rueckgabe liefert den Draw-Start. Min/Max-Vertex bleiben lokale Geometriewerte; der Basisvertex wird nicht noch einmal hinzuaddiert. Canvas und Linien verwenden den Vertexoffset direkt als Draw-Start und rufen vorher die statische Indexbindung mit Nullquelle und Basis null auf. Ein bereits leerer Indexwrapper behaelt dabei den originalen bedingten Unbindvertrag.

`Runtime::submit` verbindet beide Uploads, Cache-/Poolmutation, gegebenenfalls expliziten Index-Unbind, Drawargumente, Deferred-Flush, Materialpasses, Fog-Wiederherstellung, Lichter und Drawzaehler. Die CPU-Spiegel fuer Vertex- und Indexlocks werden getrennt erhalten. Ein Request stellt die bereits ausgewaehlten Materialpasses und Quellcallbackdaten bereit. Bei Erfolg werden veraenderte Passbilder und der tatsaechlich verwendete Indexbasiswert im Request zurueckgegeben; bei Fehler bleiben Runtime und Request vollstaendig erhalten.

## 1. Framegrid

Originalfunktion `FFrameGrid::Render`, 103a9c50, relevante Instruktionen 103aa0c1..103aa11b. Primitiveart fuenf; Start ist der Indexoffset. Primitiveanzahl ist `(2*Rows-2)*(Columns-1)`, Minvertex null und Maxvertex `Columns*Rows-1`. DWORD-Multiplikationen und Subtraktionen bleiben umlaufend, auch bei Nullwerten. Vorherige Sichtbarkeits-, Geometrie-, Transform- und Materialentscheidungen liegen ausserhalb dieses Aufruferausschnitts.

## 2. Fluidoberflaeche

Originalfunktion `AFluidSurfaceInfo::Render`, 1040bdd0. Der Uploaduebergang liegt bei 1040bfc7..1040bfe0. Im normalen Gridzweig gilt dieselbe Formel mit den Feldern +28c/+290. Der alternative Quad-Zweig bei 1040c095..1040c0a5 zeichnet zwei Primitive mit Min/Max 0/3. Die Zweigentscheidung ist eine ausdrueckliche Eingangsinformation; Objekt-/Materialbeschaffung und die spaeteren Debugboxen werden nicht als rekonstruiert behauptet.

## 3. Segmentierte Beams

Originalfunktion `UBeamEmitter::RenderParticles`, 103dd3f0. Nach beiden Uploads wird fuer jedes Segment ein Draw geplant. Primitiveanzahl ist die vorzeichenbehaftete Division des umlaufenden Produkts `Primitives*Copies` durch `Segments`. Der Indexschritt wird separat aus `Indices*Copies / Segments` berechnet. Start ist `Indexoffset + Indexschritt*Segment`, Maxvertex `Vertices*Copies-1`, Minvertex null.

Nichtpositive vorzeichenbehaftete Segmentzahlen fuehren nach den Uploads zu keinem Draw und keiner Division. Positive Segmentzahlen werden im sicheren Port auf 256 Drawanforderungen begrenzt. Divisionen runden gegen null. Der native Rechenweg einschliesslich CDQ/IDIV und Schleifenfortschritt wird interpretiert.

## 4. Canvas-Flush

Originalfunktion `FCanvasUtil::Flush`, 104c77a0. Nichtpositive vorzeichenbehaftete Primitiveanzahl ueberspringt den untersuchten Pfad; ein gueltiges Renderinterface ist in diesem Modell vorausgesetzt. Der Vertexoffset wird direkt zum Draw-Start. Primitiveart und Anzahl bleiben rohe Eingaben; Maxvertex ist der signierter-29-Bit-Arraycount minus eins. Das Flag an +b0 waehlt unabhaengig zwischen dem Count an +a4 und dem alternativen Count an +ac. Beide Zweige loesen vorher die Indexbindung.

Die optionale vorherige Bildschirmverzerrung, Materialbeschaffung und anschliessende Batch-/Arraybereinigung bleiben ausserhalb des Ausschnitts. Der Adapter erzeugt keine Canvasgeometrie.

## 5. Linienbatch-Flush

Originalfunktion `FLineBatcher::Flush`, 104d27f0. Nur die unteren 29 Countbits bestimmen, ob der Batch leer ist. Bei nichtleerem Batch wird der Count mittels SHL/SAR vorzeichenbehaftet interpretiert. Primitiveart zwei, Start Vertexoffset, Primitiveanzahl Count/2 mit Rundung gegen null, Minvertex null, Maxvertex Count-1. Auch negative gepackte Counts folgen der originalen Arithmetik. Materialobjekte und anschliessende Batchbereinigung bleiben ausdrueckliche Grenzen.

## Nachweis

`Generate-D3DDynamicDraw.py` verknuepft die zuvor geprueften Pooleingaben und Materialpassfixtures per SHA256. 48 Sequenzen enthalten jeweils alle fuenf Aufrufer. Geprueft werden neue und wiederverwendete Pools, beide Indexbreiten, direkte und Scratchuploads, Null-/negative Counts, alternative Fluid-/Canvaszweige, nichtindizierte Unbinds, unbekannte Primitivearten sowie leere und mehrfache Materialpassfolgen. Die Geometrie- und Callbackantworten sind synthetische ABI-Fixtures, keine neuen Live-Aufnahmen und kein Nachweis passender Geometriegroessen fuer die rohen Drawbereiche.

`Record-D3DDynamicDraw.py` interpretiert die Engine-Interfaceargumente, Offsetweitergabe, Countgates und Drawarithmetik. An den Interfaceaufrufen werden die vorhandenen Originalkoerper fuer Poolanlage, Ressourcenlisten, Resize, Ringupload, Scratchtransfer und Bindung eingesetzt. Anschliessend laufen die vorhandenen originalen Materialpass-, Flush- und Drawkoerper. Verglichen werden geordnete Aufrufe, alle Cachekoepfe und Wrapperbytes, Poolslots, Renderer, Deferred-Zustand, Lichter, Zaehler, veraenderte Passbilder sowie beide vollstaendigen CPU-Ziele nach jeder Anfrage. Ein eigener synthetischer Allocatorbereich verhindert eine Aliasueberlagerung mit dem Materialpassbereich der Replay-VM.

Ergebnis: **1996505 Originalinstruktionen**, **48 Sequenzen / 240 Anfragen**, **232 Vertexuploads**, **144 Indexuploads**, **88 explizite statische Index-Unbinds**, **316 Drawanforderungen** (48 Grid, 48 Fluid, 132 Beam, 44 Canvas, 44 Linien), **1216 Materialpass-Durchlaeufe**, **8 uebersprungene leere Aufrufer**. Nicht jede Drawanforderung fuehrt zu einem COM-Draw: leere Passfolgen und unbekannte Primitivearten behalten ihre bisherigen Regeln. Zwoelf sichere Fehlerfaelle pruefen Rollback beider Uploads und spaeter Passfehler einschliesslich des Requests.

**15 neue Rusttests, insgesamt 795 Workspace-Tests**. Debug-/Releaseberichte bytegleich; Clippy mit `-D warnings`, Format- und Diffpruefung bestanden.

```powershell
python scripts/Generate-D3DDynamicDraw.py
cargo run -p rc-inspect --bin rc-d3d-dynamic-draw-check -- analysis/reports/d3d-dynamic-draw.input.json analysis/reports/d3d-dynamic-draw.json
cargo run --release -p rc-inspect --bin rc-d3d-dynamic-draw-check -- analysis/reports/d3d-dynamic-draw.input.json analysis/reports/d3d-dynamic-draw-release.json
cargo test --workspace *> analysis/reports/d3d-dynamic-draw-tests.log
python scripts/Record-D3DDynamicDraw.py
```

Berichte: `analysis/reports/d3d-dynamic-draw*`; Evidenzschluessel `d3d_dynamic_draw_validation`. Originalexporte: `engine-dynamic-bridge.c`, `engine-dynamic-simple.c`, `engine-dynamic-grid.asm`, `engine-dynamic-fluid.asm`, `engine-dynamic-beam.asm`, `engine-dynamic-canvas*.asm`, `engine-dynamic-lines.asm`, `engine-dynamic-line-*.asm`, `d3d-dynamic-entries.txt`, `d3d-interface-table.txt`. Die Suche nach indirekten Engineaufrufen ist ein Rechercheindex und nicht fuer jeden Treffer eine Renderinterfacezuordnung.

Andere dynamische Engine-Aufrufer, Geometrieerzeugung/Fuellcallbacks, Materialbeschaffung, Batchbereinigung, Shadererzeugung und echte Allocator-/COM-/GPU-Ausfuehrung bleiben offen. Native Fehlerbehandlung wird nicht ausgefuehrt. Das Spiel ist weiterhin nicht spielbar; Android wird zum Schluss geprueft.
