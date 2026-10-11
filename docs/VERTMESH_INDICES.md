# Vertex-Mesh: LOD-Indexgeometrie bis zum Sektionsdraw

Stand: 2026-10-10. Fuenf weitere Aufgaben implementiert in `crates/rc-package/src/vertmesh_indices.rs`: Dreiecksauswahl nach LOD, Vertex-Collapse-Ketten, originale Indexreihenfolge, Sektionsstarts/Primitivecounts und gemeinsame Integration mit Upload und Sektionsdraws. Adapter: `rc-vertmesh-indices-check`. Grundlage: `Engine!UVertMeshInstance::Render` 10560378..105604c6.

Die bisher extern gelieferten Sektions-Primitivecounts und Indexcallbackbytes werden in diesem Pfad nun aus den Quellarrays berechnet. Der ganzzahlige LOD-Wert ist weiterhin eine Eingabe; seine vorherige kamera-/projektionsabhaengige Berechnung und die eigentliche Vertexanimation/-positionserzeugung sind nicht Teil dieses Schritts.

## 1. Dreiecksauswahl

Die 80-Byte-Sektionsbilder werden mit dem signed-29-Bit-Arraycount durchlaufen. WORD +0e gibt den ersten Quellface und WORD +10 dessen Anzahl an. Der Face-LOD-Wert stammt aus dem WORD-Array bei Mesh+94. Der native Vergleich ist signed nach Zero-Extension des WORDs: **FaceThreshold <= LOD** schliesst das Dreieck ein. Ein negativer LOD schliesst alle unsigned Facewerte aus. Fuer ausgeschlossene Faces werden die Vertexindices nicht gelesen.

## 2. Vertex-Collapse

Der aktuelle Vertexindex liest den WORD-Wert aus seinem 12-Byte-Record bei Mesh+ac. Solange **VertexThreshold >= LOD**, wird der naechste Index aus dem Collapse-WORD-Array bei Mesh+a4 geladen. Die Kette endet strikt unterhalb des LOD-Wertes. Der Gleichheitsfall kollabiert also weiter, waehrend der Face-Gleichheitsfall ein Dreieck zulaesst.

Fehlende benoetigte Arrayeintraege, Zyklen und Ketten mit mehr als 1024 Links werden sicher abgewiesen. Die nativen Erfolgsinstruktionen werden ausgefuehrt; diese Fehlerbehandlung ist eine Port-Sicherheitsregel. Bei LOD null kann eine erreichbare Kette ohne terminalen Wert unendlich laufen; der sichere Port erkennt dies. Es wird keine zusaetzliche Degeneratfilterung eingefuehrt: zusammenfallende Vertexindices bleiben wie im Original im Ausgabedreieck.

## 3. Originale Indexreihenfolge

Ein Quellface ist acht Bytes gross. Das Original kopiert zwei DWORDs auf den Stack und liest die drei WORDs rueckwaerts von +4, +2 und +0. Aus `[a,b,c,metadata]` entsteht daher **`[collapse(c),collapse(b),collapse(a)]`**. Das vierte WORD bleibt unbenutzt. Die resultierende Indexquelle ist immer ein little-endian u16-Buffer.

## 4. Sektionsstarts und Counts

Vor jeder Sektion wird deren WORD +02 auf den aktuellen akkumulierten Indexcount gesetzt. Der originale WORD-Store schneidet hohe Bits ab: 65538 vorherige Indizes ergeben FirstIndex 2. Danach wird der Primitivecount nur fuer tatsaechlich ausgegebene Dreiecke erhoeht und in das lokale DWORD-Countarray geschrieben. Auch leere beziehungsweise vollstaendig gefilterte Sektionen erhalten ihren aktuellen FirstIndex und Count null. Alle anderen Sektionsbytes, einschliesslich Min-/Maxvertex und Quellfacebereich, bleiben erhalten.

Positive Sektionscounts sind auf 256 begrenzt, die Gesamtzahl besuchter Quellfaces auf 65536. Arraybilder muessen ausreichend lang sein. Nichtpositive signed-29-Bit-Counts fuehren zu keiner Sektionsiteration. Die Schutzgrenzen sind ausdrueckliche sichere Portregeln und bilden keine nativen Fehlerpfade nach.

## 5. Gemeinsame Geometrie-/Upload-/Draw-Transaktion

`Runtime::submit_geometry` erzeugt zuerst Indexgeometrie und Sektionscounts. Das Ergebnis ersetzt `mesh.sections`, Indexquellbreite, Indexquellgroesse, Indexcallbackbytes und gemeldete Indexbytezahl im geklonten Request. Anschliessend laeuft `submit_mesh` aus `D3D_MESH_DRAW.md` mit einem Vertex-/Indexuploadpaar, den Materialrouten je Sektion und der kompletten Materialpass-/Flush-/Drawkette.

Die originale Eingangskopie `request.geometry` bleibt als Quellsnapshot erhalten; die mutierten Sektionsbilder stehen in `request.mesh.sections` und im Geometrieplan. Runtime und gesamter Request werden erst bei Gesamterfolg ersetzt. Geometriefehler, fehlende Materialantworten und spaetere Upload-/Drawfehler erhalten auch beide CPU-Spiegel und alle bestehenden Buffer-/Passzustaende. Die Vertexcallbackbytes werden weiterhin als Eingang bereitgestellt; dieser Meilenstein erzeugt keine Vertexpositionen.

## Originalabgleich

`Generate-VertMeshIndices.py` verknuepft die zuvor SHA256-geprueften Mesh- und Materialpassfixtures. Die Roharrays sind synthetische ABI-Proben. 50 Sequenzen mit 146 Anfragen pruefen mehrere LOD-Werte, Null-/negative Counts, obere Arrayflags, gefilterte und leere Sektionen, mehrstufige Collapse-Ketten, verschiedene Materialrouten und direkte/Scratchuploads. Ein grosser integrierter Fall erzeugt 65538 Indizes und belegt den WORD-Umlauf des folgenden Sektionsstarts.

`Record-VertMeshIndices.py` interpretiert den originalen Geometriekoerper, die WORD-Stores, Sektionsmutation und Primitivecount-Stores. Die nativen Array-Appendhelfer 103327f0 und 10304740 bleiben ausdrueckliche Allocatorgrenzen mit bereitgestellten Pointer-/Countantworten; deren Allokationskoerper und die vorausgehende Reservierung werden nicht als ausgefuehrt behauptet. Danach laufen der bestehende Original-Uploaduebergang und die vollstaendige Mesh-/D3D-Pruefkette. Ergebnisse, mutierte Requests, alle Runtimebilder und CPU-Ziele werden nach jeder Anfrage verglichen.

Die Replay-VM unterstuetzt jetzt auch die unteren 16-Bit-Register AX/BX/CX/DX/SI/DI/BP/SP, einschliesslich des Erhalts ihrer oberen Registerhaelften. Acht direkte Regressionen pruefen dies. Die neue Geometriepruefung fuehrt unter anderem die originalen Stores von CX und SI aus. Ihre kuenstlichen Quell-/Zielarrays liegen ausserhalb der Scratchbereiche. Die vorherige Meshpruefung mit 3.614.789 Originalinstruktionen wurde nach der VM-Erweiterung erneut bestanden, ohne historische Berichte umzuschreiben.

Ergebnis: **5.176.806 Originalinstruktionen**, **50 Sequenzen / 146 Anfragen**, **67.464 erzeugte u16-Indizes / 22.488 ausgegebene Dreiecke**, **590 gefilterte Faces**, **1.341 Collapse-Links**, **724 erzeugte Sektionscounts**, **146 Vertex-/146 Indexuploads**, **346 Drawanforderungen** und **1.475 Materialpass-Durchlaeufe**. 378 Sektionen erzeugen Count null. Materialrouten: 77 Actor-Overrides, 39 Resolver-Fallbacks und 230 Wire-/Debug-Sektionen.

**12 sichere Fehlerfaelle** pruefen unvollstaendige Arrays, Collapse-Zyklen, fehlende Links, Uploadfehler und fehlende Materialantworten; Runtime und Request bleiben jeweils unveraendert. Die rohen Min-/Maxvertex-Werte und separat gelieferten Vertexcallbackbytes sind weiterhin synthetische Eingaben; die Indexpruefung allein belegt noch keine vollstaendig passende Live-Geometrie.

**15 neue Rusttests, insgesamt 840 bestandene Workspace-Tests.** Debug/Release bytegleich; Clippy mit `-D warnings`, Format- und Diffpruefung bestanden.

```powershell
python scripts/Generate-VertMeshIndices.py
cargo run -p rc-inspect --bin rc-vertmesh-indices-check -- analysis/reports/vertmesh-indices.input.json analysis/reports/vertmesh-indices.json
cargo run --release -p rc-inspect --bin rc-vertmesh-indices-check -- analysis/reports/vertmesh-indices.input.json analysis/reports/vertmesh-indices-release.json
cargo test --workspace *> analysis/reports/vertmesh-indices-tests.log
python scripts/Record-VertMeshIndices.py
```

Berichte: `analysis/reports/vertmesh-indices*`; Evidenzschluessel `vertmesh_indices_validation`. Originalexport: `engine-vertmesh-geometry.asm` und der vollstaendige Decompilertext in `engine-particle-dynamic.c`. Aufbauend auf `D3D_MESH_DRAW.md`, `D3D_PARTICLE_DRAW.md` und `D3D_POOLS.md`.

Offen bleiben kameraabhaengige LOD-Auswahl, Vertexpositionen/Animation, Vorverarbeitung der Quellarrays, ganze Sichtbarkeits-/Setupstrecke, Materialresolver/-kompilierung, Wire-Shaderanlage und echte COM-/GPU-Ausfuehrung. Das Spiel ist noch nicht spielbar; Android wird wie vereinbart zum Schluss geprueft.
