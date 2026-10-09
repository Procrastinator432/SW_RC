# Vertex-Mesh: originale Sektionen und Materialwechsel

Stand: 2026-10-09. Die naechsten fuenf Aufgaben schliessen die bisherige Grenze gemeinsamer Materialpasses aus `D3D_PARTICLE_DRAW.md`. Modul: `crates/rc-package/src/d3d_mesh_draw.rs`; Adapter: `rc-d3d-mesh-draw-check`. Grundlage ist `Engine!UVertMeshInstance::Render` (10560100), insbesondere 105607b4..10560906. Geometrie, Animation, vorangehende Sichtbarkeits-/Setupentscheidungen und Callbackantworten bleiben Eingaben.

## 1. Originale Arrayiteration

`Sections` enthaelt das rohe Arraybild, den gepackten Count und die bereits berechneten Primitivecounts. Wie SHL/SAR im Original werden die unteren 29 Countbits vorzeichenbehaftet interpretiert. Nichtpositive Counts iterieren nicht; die Uploads finden in diesem Ausschnitt trotzdem statt. Positive Counts sind auf 256 Sektionen begrenzt. Die Bilder muessen fuer den deklarierten Count vollstaendig sein; das ist eine ausdrueckliche sichere Vorpruefung.

Die originale Sektionsschrittweite betraegt **80 Bytes / 0x50**. Die uninterpretierten Restbytes bleiben erhalten. WORDs werden little-endian und ohne Vorzeichenerweiterung gelesen:

| Offset | Bedeutung im untersuchten Ausschnitt |
| --- | --- |
| +00 | Materialindex |
| +02 | Erster Index |
| +04 | Minvertex |
| +06 | Maxvertex |
| +10 | Nichtnull-WORD fuer Sektionsfreigabe |

Die native Schleife und ihr Fortschritt werden jetzt interpretiert; die vorherige Aufruferroute hatte bereits aufgeloeste Sektionen erhalten. Der rohe Count kann obere Arrayflags tragen oder einen negativen signed-29-Bit-Wert darstellen. Das globale Mesh-/Geometrie-/Sichtbarkeitsgate vor den Uploads ist weiterhin ausserhalb dieses Adapters.

## 2. Sektionsfilter und Drawbereiche

Der originale DWORD-Primitivecount aus dem lokalen Array wird zuerst auf null geprueft. Erst danach folgt der WORD-Gate an +10. Ein nichtnull WORD, auch 8000 oder ffff, ist aktiv. Die originale Reihenfolge bleibt erhalten. Der Draw verwendet Primitiveart 5, Start `Indexoffset + unsigned FirstIndex` mit DWORD-Umlauf, den separat berechneten Primitivecount und unveraenderte unsigned Min-/Max-WORDs. Invertierte Bereiche werden wie beim bisherigen rohen Drawvertrag nicht normalisiert.

## 3. Materialroute je Sektion

Die Reihenfolge der urspruenglichen Schnittstellen bleibt ausdruecklich sichtbar in `Selection.calls`:

1. Mesh+a0(Actor) liefert den als Antwort bereitgestellten Materialresolver.
2. Resolver+d8(Materialslot, Actor) wird **auch bei Wire-/Debug-Darstellung** aufgerufen. Der Slot stammt aus dem nativen Materialrecord bei `MaterialIndex*8+4`.
3. Bei Wire oder nichtnull Debugword wird der bereits erzeugte Shader aus 108b9b28 gewaehlt.
4. Sonst entscheidet der signed-29-Bit-Overridecount des Actorobjekts: MaterialIndex kleiner als Count ruft Actorobjekt+cc(MaterialIndex) auf; andernfalls bleibt das Resolverergebnis.
5. Renderinterface+50(Material,0,0,0) markiert den SetMaterial-Uebergang.

Eine Null-Materialantwort wird an SetMaterial weitergereicht, sofern ein entsprechender Materialbankeintrag vorhanden ist. Null-Receiver, fehlende Slot-/Getterantworten und ein fehlender Wire-Shader werden sicher abgewiesen. Die Erstellung des Wire-Shaders, IsWire, Resolver-/Actoraccessor-Koerper und die Materialkompilierung innerhalb SetMaterial werden nicht als ausgefuehrt behauptet: ihre Antworten beziehungsweise vorbereiteten Passbilder sind Eingaben.

## 4. Getrennte Materialpasses und gemeinsame Passidentitaet

Die Materialbank ordnet jedem eindeutigen Materialtoken eigene vorbereitete Passbilder zu. Jede aktive Sektion waehlt ihren Eintrag und fuehrt dessen komplette bestehende D3D-Materialpass-/Flush-/Fog-/Light-/Drawkette aus. Leere Passfolgen bleiben erlaubt.

Ein nativer Passpointer bezeichnet auch ueber Materialgrenzen hinweg dasselbe Objekt. Deshalb werden widerspruechliche Eingangssnapshots fuer gleiche Passadressen abgewiesen und veraenderte Passbilder an alle Aliase weitergegeben. `last_pass` bleibt ueber Sektions- und Materialwechsel erhalten; dieselbe Passidentitaet behaelt ihren originalen Auswahl-Skipvertrag. Die sichere Bankgrenze liegt bei 256 Materialien und acht Passes je Material. Diese Bank- und Aliasvorpruefungen sind Port-Sicherheitsregeln, keine nachgebauten nativen Fehlerpfade.

## 5. Eine gemeinsame Upload-/Draw-Transaktion

`Runtime::submit_mesh` verwendet die bestehende gepruefte Uploadkette fuer **ein Vertex-/Indexuploadpaar**, gibt den Vertexoffset als Indexbasis weiter und zeichnet danach alle aktiven Sektionen. Es gibt keinen erneuten Upload je Materialwechsel. Beide CPU-Ziele, Cache-/Poollisten, Scratch-/Ring-/Resizezustand, Bindungen, Deferred-Zustaende, Lichter, Zaehler, letzter Pass und alle veraenderten Materialbankbilder werden erst bei Gesamterfolg uebernommen. Ein Fehler nach einem oder mehreren fertigen Sektionsdraws erhaelt Runtime und Request vollstaendig.

Der Plan enthaelt das Uploadpaar sowie je aktiver Sektion die dekodierten Felder, Materialroute, Drawargumente und vollstaendige Passplaene. Die Ausgabe bleibt ein CPU-Modell; sie fuehrt keine GPU-Kommandos aus.

## Nachweis

`Generate-D3DMeshDraw.py` verknuepft die SHA256-geprueften vorherigen Partikel-/Poolfixtures und Materialpassfixtures. Die Rohbilder und Callbackantworten sind synthetische ABI-Fixtures, keine Live-Geometrieaufnahme. **49 Sequenzen / 145 Anfragen** decken gepackte Null-/negative Counts, Arrayflags, beide Indexbreiten, direkte und Scratchuploads, mehrere Materialien, gemeinsame Passaliase, leere Passfolgen, unsigned WORD-Grenzen und unterschiedliche Primitivecounts ab.

`Record-D3DMeshDraw.py` interpretiert den Original-Uploaduebergang, die aeussere Sektionsschleife, beide Gates, Materialargumente/Verzweigungen und Drawargumente. Die vorhandenen Originalkoerper fuer Pools, Resize, Scratch/Ringupload, Bindungen und die vollstaendige Materialpass-/Drawkette werden eingesetzt. Materialgetter und SetMaterial-Kompilierung bleiben klar markierte Antwortgrenzen. Nach jeder Anfrage werden Ergebnisse, veraenderte Requests, beide CPU-Ziele und der komplette vorhandene Runtimezustand verglichen.

Ergebnis: **3.614.789 Originalinstruktionen**, **145 Vertex-/145 Indexuploads**, **491 Drawanforderungen**, **2034 Materialpass-Durchlaeufe**. Materialroute: **93 Actor-Overrides, 72 Resolver-Fallbacks, 326 Wire-/Debug-Sektionen**. **144 Sektionen** werden durch Count null und **87** durch WORD null gefiltert. 491 Meshresolveraufrufe und 491 SetMaterial-Uebergaenge bleiben in der Originalreihenfolge sichtbar.

**16 sichere Fehlerfaelle** pruefen unter anderem kurze Arraybilder, fehlende Materialantworten, widerspruechliche Aliase, fehlende Bankeintraege und Uploadfehler. Der spaete Passfehler hat einen zusaetzlichen, original interpretierten Erfolgs-Prefix mit zwei ausgefuehrten Sektionen; die laengere Anfrage scheitert erst am spaeteren ungueltigen Pass und setzt trotzdem alles zurueck.

**18 neue Rusttests, insgesamt 825 bestandene Workspace-Tests.** Debug/Release bytegleich; Clippy mit `-D warnings`, Format- und Diffpruefung bestanden.

```powershell
python scripts/Generate-D3DMeshDraw.py
cargo run -p rc-inspect --bin rc-d3d-mesh-draw-check -- analysis/reports/d3d-mesh-draw.input.json analysis/reports/d3d-mesh-draw.json
cargo run --release -p rc-inspect --bin rc-d3d-mesh-draw-check -- analysis/reports/d3d-mesh-draw.input.json analysis/reports/d3d-mesh-draw-release.json
cargo test --workspace *> analysis/reports/d3d-mesh-draw-tests.log
python scripts/Record-D3DMeshDraw.py
```

Berichte: `analysis/reports/d3d-mesh-draw*`; Evidenzschluessel `d3d_mesh_draw_validation`. Originalexporte: `engine-particle-dynamic.c`, `engine-vertmesh-upload.asm`, `engine-vertmesh-draw.asm`, `engine-vertmesh-loop.asm`. Gemeinsame Uploadvertraege: `D3D_POOLS.md` / `D3D_PARTICLE_DRAW.md`.

Weiter offen sind Vertex-Mesh-Geometrie-/Countgenerierung, Animation, ganze Sichtbarkeits-/Setupstrecke, echte Materialresolver/-kompilierung und Wire-Shaderanlage sowie Allocator-/COM-/GPU-Ausfuehrung. Das Spiel ist noch nicht spielbar. Android wird zum Schluss geprueft.
