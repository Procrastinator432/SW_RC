# Vertex-Mesh: kamerabhaengiger LOD und fruehe Gates

Ergaenzung 2026-10-10: Der neue vorgeschaltete Pfad in
`SCENE_PROJECTION.md` rekonstruiert jetzt Project/Deproject und berechnet das
Projektions-W aus Szenenmatrix und Actorposition. Die folgenden Angaben zur
externen Project-Antwort beschreiben weiterhin den historischen isolierten
LOD-Nachweis; dessen API und Berichte bleiben reproduzierbar.

Stand: 2026-10-10. Fuenf weitere Aufgaben umgesetzt in `crates/rc-package/src/vertmesh_lod.rs`: Bildschirmfaktoren, Distanz-/Actor-Skalierung, Integerkonvertierung/Begrenzung, fruehe Sektions-/Actor-Gates und Vertexcallback-Uebergang mit nachfolgender Indexgeometrie. Adapter: `rc-vertmesh-lod-check`. Grundlage: `Engine!UVertMeshInstance::Render`, 10560194..1056032a.

Der untersuchte Ausschnitt beginnt **nach** dem initialen Transformsetup und einer gegebenenfalls erfolgten RenderPreProcess-Antwort. Deren Ausfuehrung bleibt offen. Der vorher extern gelieferte ganzzahlige LOD-Wert wird in diesem Pfad jetzt aus Kamera-/Mesh-/Actor-Werten berechnet. Projektions-W, der auf float32 gespeicherte x87-FPTAN-Wert und der gespeicherte FVector.GetAbsMax-Wert bleiben ausdrueckliche Callbackantworten. Die originalen Koerper dieser drei Berechnungen werden hier nicht als rekonstruiert behauptet.

## 1. Bildschirmfaktoren

Originale Engine-Konstanten werden direkt aus dem PE-Bild geprueft:

| Adresse | float32-Wort | Wert |
| --- | --- | --- |
| 10676768 | 3b449ba6 | ca. 0.003 |
| 10664fb0 | 3e800000 | 0.25 |
| 1067bff4 | 3a8f5c29 | ca. 0.00109375 |
| 10668db0 | 3e99999a | ca. 0.3 |
| 10653578 | 3f800000 | 1 |
| 1067bff0 | 44048000 | 530 |

Der Vertexfaktor ist `float(VertexCount)*0.003 + 0.25`, der Viewportfaktor `float(ViewportWidth)*0.00109375 + 0.3`. Die Integerquellen werden vorzeichenbehaftet konvertiert. Einzelne Operationen bleiben float32 in der originalen SSE-Reihenfolge; es wird keine zusaetzliche Genauigkeit oder FMA eingefuehrt.

## 2. Distanz und Actor-Skalierung

Der projizierte W-Wert minus Mesh-Distanzbias wird mit dem originalen COMISS-Zweig mindestens auf eins begrenzt. Der Nenner ist in dieser Reihenfolge:

`((MeshDistanceScale * Distance) * VertexFactor) * StoredTanFov`

Actor.DrawScale wird bei negativem Vergleich als `0-Scale` berechnet. Negatives null bleibt im anderen Zweig bitgenau erhalten. Der Kandidat entsteht durch die einzelnen SSE-Schritte:

`VertexCount * (((((ActorLodBias / Denominator) * MeshLodScale) * ActorAbsMax) * AbsoluteDrawScale) * ViewportFactor * 530)`

Die Formel dient der Lesbarkeit; die Implementierung behaelt jeden originalen float32-Zwischenschritt bei. Der ausgegebene LOD-Plan enthaelt die rohen Worte fuer beide Bildschirmfaktoren, Distanz, Nenner, absoluten DrawScale, Kandidat und den vor Konvertierung ausgewaehlten Float.

## 3. Minimum, Rundung und obere Begrenzung

Zuerst wird das als float32 konvertierte Meshminimum mit dem Kandidaten verglichen. Danach verwendet CVTSS2SI unter dem hier vorausgesetzten Standard-MXCSR **round-to-nearest-even**. Eine ungueltige oder ueberlaufende Integerkonvertierung liefert wie die native SSE-Konvertierung `80000000` / i32::MIN. Der naechste Vergleich begrenzt ausschliesslich signed nach oben auf VertexCount. Ein weiterer unterer Clamp wird nicht hinzugefuegt.

Der sichere Kamerapfad weist nichtfinite Eingangsfloats sowie einen Null-/nichtfiniten Nenner und nichtfiniten Kandidaten ab. Diese Regeln sind ausdrueckliche Port-Sicherheitsgrenzen; NaN-/Inf-Arithmetik und andere MXCSR-Rundungs-/Denormalmodi werden nicht als abgedeckt behauptet. Der separate Konvertierungshelper prueft trotzdem den nativen Integer-Indefinitevertrag. Die Bedingungen werden vor jeder Modellmutation geprueft.

## 4. Fruehe Gates

Der Sektionsgate prueft nur die unteren 29 Bits des postprozessierten Counts. Ein signed negativer, aber nichtnull unterer Count besteht diesen Gate; erst die spaetere Sektionsiteration behandelt ihn als nichtpositiv. Reine obere Flags bei Count null fuehren zur fruehen Rueckgabe.

Bei vorhandenem Sektionscount wird die originale Actorreihenfolge beachtet: Actor+1a4 nichtnull aktiviert den Pfad direkt; andernfalls kann ein vorhandenes Ownerobjekt mit nichtnull Feld +2f8 aktivieren; zuletzt entscheidet das Feld +2f8 des Fallbackobjekts. Ohne aktivierenden Wert endet der Ausschnitt. Die Ergebnisse sind `enabled`, `no_sections` oder `no_actor_state`.

Bei einem fruehen Ende laufen innerhalb dieses untersuchten Ausschnitts weder Vertexcallback noch Geometrie-/Bufferupload oder Materialdraw. Runtime und Request bleiben erhalten, der berechnete LOD-/Gateplan wird dennoch zurueckgegeben. Dies beschreibt nicht die gesamte Renderfunktion: das vorausgehende Transformsetup liegt ausdruecklich ausserhalb dieser Runtimeoperation.

## 5. Vertexcallback und gemeinsame Integration

Bei aktiviertem Pfad wird die originale Instance-Schnittstelle +dc mit Actor, Scene, Meshquellzeiger, **Stride 0x20**, Countpointer und abschliessender Null sichtbar. Der Countpointer erhaelt vor dem Call den ausgewaehlten LOD-Wert. Die extern gelieferte Callbackantwort kann diesen Countpointer veraendern. Das Original behaelt aber den zuvor gesicherten LOD fuer die nachfolgende Indexgeometrie: der neue Wert wird nicht faelschlich als Geometrie-LOD verwendet.

`Runtime::submit_lod` setzt den ausgewaehlten LOD im geklonten Geometrieinput und verbindet `submit_geometry` aus `VERTMESH_INDICES.md` mit Upload und Materialdraws aus `D3D_MESH_DRAW.md`. Aktivierte Gate-/Geometriecounts muessen denselben Snapshot beschreiben. Erst bei Gesamterfolg werden Runtime und Request uebernommen. Vertexanimation und Vertexcallbackbytes bleiben geliefert; der +dc-Koerper wird noch nicht ausgefuehrt.

## Nachweis

`Generate-VertMeshLod.py` verknuepft SHA256-gepruefte Indexgeometrie- und Materialfixtures. 50 Sequenzen / 146 Anfragen pruefen verschiedene Bildschirm-/Distanz-/Skalierungswerte, negative und negative-null DrawScale-Werte, beide Gatearten, Owner-/Fallbackpfade und abweichende Countpointer-Antworten. Die Eingangsdaten sind synthetische ABI-Proben und keine Live-Kamera-/Geometrieaufnahme. Vertexcallbackdaten, Min-/Maxbereiche und reale Vertexpositionen sind weiterhin nicht als vollstaendig passende Geometrie nachgewiesen.

`Record-VertMeshLod.py` interpretiert die SSE-Arithmetik, Vergleiche, Rundung, signed Begrenzung, Gates und originalen Vertexcallbackargumente. Gespeicherte FPTAN-/GetAbsMax- und Project-Antworten sowie die Vertexcallbackantwort sind klar markierte Grenzen. Danach laeuft die vorhandene Originalpruefkette fuer Collapse-/Indexgenerierung, Sektionsmutation, Pools, Resize, Scratch-/Ringupload, Materialrouten und komplette Materialpasses/Draws. Ergebnisse, veraenderte Requests und alle Runtime-/CPU-Zustaende werden nach jeder Anfrage verglichen.

Die gemeinsame Replay-VM wurde um die benoetigten SSE-Operationen MOVAPS, CVTSI2SS, CVTSS2SI, ADDSS/SUBSS/MULSS/DIVSS und den Carry-Zweig JC erweitert. Neue Opcodeimporte erkennen auch Ziffern in Konvertierungsnamen. Sieben direkte Original-CVTSS2SI-Proben pruefen positive/negative Halbwerte, Integergrenzen und NaN-Integer-Indefinite. Die vorherige Indexgeometriepruefung mit 5.176.806 Originalinstruktionen wurde nach der VM-Erweiterung erneut bestanden, ohne historische Berichte umzuschreiben.

Ergebnis: **4.747.812 Originalinstruktionen**, **50 Sequenzen / 146 Anfragen**, **36 fruehe Rueckgaben wegen leerer Sektionen**, **30 wegen inaktivem Actorstatus**. **80 Vertexcallbacks** und **80 Vertex-/80 Indexuploads** fuehren weiter zu **67.062 erzeugten Indizes**, **22.552 Collapse-Links**, **276 Drawanforderungen** und **1.197 Materialpass-Durchlaeufen**. Die Zahl der Instruktionen ist wegen der neuen Gates nicht mit einem prozentualen Gesamtfortschritt gleichzusetzen.

**12 sichere Fehlerfaelle** pruefen Kameragrenzen, ungueltige Callbackreceiver/-pointer, widerspruechliche Sektionssnapshots, Collapse-Zyklen und spaetere Upload-/Materialfehler. Runtime und Request bleiben erhalten. **13 neue Rusttests, insgesamt 853 bestandene Workspace-Tests.** Debug/Release bytegleich; Clippy mit `-D warnings`, Format- und Diffpruefung bestanden.

```powershell
python scripts/Generate-VertMeshLod.py
cargo run -p rc-inspect --bin rc-vertmesh-lod-check -- analysis/reports/vertmesh-lod.input.json analysis/reports/vertmesh-lod.json
cargo run --release -p rc-inspect --bin rc-vertmesh-lod-check -- analysis/reports/vertmesh-lod.input.json analysis/reports/vertmesh-lod-release.json
cargo test --workspace *> analysis/reports/vertmesh-lod-tests.log
python scripts/Record-VertMeshLod.py
```

Berichte: `analysis/reports/vertmesh-lod*`; Evidenzschluessel `vertmesh_lod_validation`. Originalexport: `engine-vertmesh-lod.asm` und der vollstaendige Decompilertext in `engine-particle-dynamic.c`.

Offen bleiben Transform-/PreProcess-Ausfuehrung, Projektions-/FPTAN-/GetAbsMax-Koerper, Vertexpositionen/Animation, Quellarrayvorverarbeitung, echte Materialresolver/-kompilierung, Wire-Shaderanlage und echte COM-/GPU-Ausfuehrung. Das Spiel ist noch nicht spielbar; Android wird wie vereinbart zum Schluss geprueft.
