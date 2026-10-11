# Vertex-Mesh RenderPreProcess und Actor-GetAbsMax

Ergaenzung 2026-10-10: `VERTMESH_FRAME.md` implementiert jetzt gepackte
Positions-/Normalendecodierung, Framepaar-Interpolation und Vertexstreamfuellung
mit Index-/Uploadanbindung. Die nachstehenden Grenzen zur Vertexfuellung beziehen
sich auf diesen historischen PreProcess-Nachweis; Sequenzwahl, Scheduling und
GetFrame-Cacheallokation bleiben auch im neuen Pfad explizite Eingaben.

Stand: 2026-10-10. Fuenf Aufgaben umgesetzt: Streamreset/UV/Revision,
aufeinanderfolgende Materialgruppen, Vertex-/Indexstream-Konstruktoren samt
Cache-ID-Fortschreibung, Actor-GetAbsMax und gemeinsame transaktionale Integration
mit Originalreplay. Rust: `vertmesh_preprocess.rs`, Adapter:
`rc-vertmesh-preprocess-check`.

## 1. Gate, Streamreset und UV-Kopie

Engine!UVertMesh::RenderPreProcess liegt bei 1055fbe0. Nichtnull untere 29 Bits
des Sektionscounts +1a4 fuehren zur Rueckgabe 0 ohne Quellenpruefung oder Mutation.
Dies gilt auch bei einem signed negativen Count. Bei Count null folgt die
Vorbereitung; reine obere Flags verhindern sie nicht.

Das Original durchlaeuft zunaechst den alten signed Streamcount ohne Nutzarbeit.
Danach setzt der Arrayhelper 103bcfb0 den Streamcount auf null und erweitert ihn
um den signed Wedgecount. Seine Speicherzuteilung bleibt eine explizite Grenze.
Die folgenden nativen REP-Stores nullen die neuen 32-Byte-Records vollstaendig.
Revision +16c steigt mit u32-Ueberlauf. Pro Wedge (Stride 12) werden die rohen
U/V-Worte +4/+8 nach Streamrecord+18/+1c kopiert. NaN-UV-Payloads und negative
Null bleiben reine Bytes; Positionen/Normalen werden dadurch noch nicht erzeugt.
Die oberen drei Countflags bleiben bei den Arrayhelperantworten erhalten.

## 2. Materialgruppen und Sektionsheader

Die Faces haben Stride acht; WORD 3 ist das Material. Ein neues Material gegenueber
dem unmittelbar vorangehenden Face erzeugt eine neue 80-Byte-Sektion. Ein spaeter
wiederkehrendes Material wird nicht mit einer frueheren Sektion zusammengefasst.
Das Original setzt Material WORD+0, Startface WORD+e, Minvertex WORD+4 auf null,
Maxvertex WORD+6 auf Wedgecount-1 und zaehlt Faces in WORD+10. Start, Maxvertex
und Laenge behalten native WORD-Trunkierung/Ueberlaeufe. Eine Gruppe aus 65.536
Faces endet deshalb mit Laenge null; null Wedges ergeben Maxvertex ffff.

`section_allocation` liefert die komplette Antwort des Sektionsallocators fuer
alle neuen Records. Unbeschriebene Bytes bleiben daraus erhalten, insbesondere
FirstIndex WORD+2, Headerpadding und +34..+37. FirstIndex wird erst bei der
anschliessenden Indexgenerierung gesetzt. Ohne Faces erfolgt kein Sektionsappend;
ein vorhandenes Byteimage bei logischem Count null bleibt unveraendert.

## 3. Konstruktoren und Cache-IDs

Der eingebettete FAnimMeshVertexStream beginnt bei +14 (vtable 1066a27c).
Seine 64-Bit-ID bei +18 ist `CacheIndex*256 + e1` modulo 2^64. Die Felder
+20/+24/+28/+2c/+30 sind nach Konstruktion null.
Der originale FRawIndexBuffer-Konstruktor 104c7c00 wird fuer Record+38 ausgefuehrt
(vtable 10665540). Die folgende ID bei +3c verwendet den naechsten CacheIndex
und Tag e2; +44/+48/+4c sind null. Jeder der beiden Konstruktoren erhoeht den
globalen 64-Bit-Zaehler, einschliesslich Low-/Highword-Ueberlauf. Beide IDs sowie
der fortgeschriebene Zaehler werden verglichen.

Der Replay interpretiert Stores, ADD und ADC der Konstruktoren. Nur der
__allmul-Helfer beantwortet den exakt definierten unteren 64-Bit-Multiplikationswert;
sein Koerper ist nicht als rekonstruiert behauptet. Vier direkte Original-ADD/ADC-
Proben pruefen Lowwordcarry, Highwordwrap und signed Overflow. Die gemeinsam
verwendete Replay-VM wurde um ADC erweitert. Der bisherige Projektionsnachweis
mit 327 Mathematikproben und 146 Mesh-Anfragen wurde danach erneut bestanden.

## 4. FVector::GetAbsMax

Engine-Importslot 106509f8 wird per PE-Importname auf Core.dll
`?GetAbsMax@FVector@@QBEMXZ` aufgeloest; der PE-Export liegt bei 1011e5b0.
Der Replay prueft beide Zuordnungen direkt in den DLL-Dateien.

Der Koerper bestimmt die Betraege ueber COMISS/JC und `0-v`, nicht ueber
Signbitmaskierung. Negative Null bleibt deshalb erhalten. Danach wird zuerst
X gegen Y, dann das Ergebnis gegen Z verglichen. Bei Gleichheit gewinnt das
bisherige Ergebnis; X=-0, Y=+0, Z=+0 liefert -0. Die float32-Ergebnisuebergabe
ueber FLD/FSTP ist fuer die endlichen Proben exakt, ohne allgemeine x87-Emulation.
Nichtfinite Actorwerte werden im sicheren Rustvertrag abgewiesen; NaN-/Inf-
Vergleichssemantik wird nicht als abgedeckt behauptet.

## 5. Integration und sichere Grenzen

`Input::prepare` erzeugt einen Vorbereitungssnapshot, ohne Inputmutation.
`Runtime::submit_preprocessed_mesh` uebernimmt diesen Snapshot und berechnet den
Actor-AbsMax aus DrawScale3D. Er ersetzt den bisherigen Camera-AbsMax-Wert und
synchronisiert Gate-/Geometriecount sowie Sektionsimage. Nach tatsaechlicher
Vorbereitung stammen Face-/Vertexthresholdarrays aus den gelieferten
Face-/Wedgequellen. Collapse- und Face-LOD-Arrays bleiben eigene Eingaben.

Danach laeuft die vorhandene Project-/LOD-/Index-/Upload-/Materialstrecke.
Erst bei Gesamterfolg wird der Request uebernommen; ihre Runtimeoperationen sind
bereits transaktional. Bei fruehen und spaeten Fehlern bleiben auch Revision,
Cachezaehler und Vorbereitungssnapshot erhalten. Die vorbereiteten UV-Records
ersetzen **nicht** die extern gelieferten animierten GetFrame-Vertexbytes.
Sektionsbytes im Vorbereitungssnapshot beschreiben den Zustand vor Indexaufbau;
die danach veraenderten kanonischen FirstIndex-Header stehen wie bisher im
untergeordneten Meshrequest (`geometry.mesh.sections`).

Vorbereitung begrenzt aktive alte Stream-/Wedge-/Facecounts auf 0..65.536,
Materialgruppen auf 256 und prueft Quellen-/Allocatorlaengen. Diese Grenzen
verhindern native ungueltige Speicherzugriffe oder unbeschraenkte Schleifen;
sie sind sichere Portfehler. Im Skipgate werden ungenutzte Quellen bewusst nicht
geprueft. Logausgabe und SEH-Registrierung/Unwind sind vom Replay ausgeschlossen.

## Nachweis

SHA256-gepruefte Szenenfixtures werden um synthetische Quellrecords und
Allocatorantworten erweitert. Kein Live-Spielmitschnitt. Der alte Camera-AbsMax
ist absichtlich NaN, damit eine fehlende Ersetzung auffaellt. Die untergeordneten
Vertexuploadbytes sind weiterhin eigenstaendige Antworten; Geometrie-/Vertex-
Min-/Maxbereiche sind damit nicht als reale zusammenpassende Spielgeometrie belegt.

**50 Vorbereitungssonden**: 46 erfolgreiche Originalvergleiche und vier sichere
Fehler. Darunter Nullcounts, Skip mit ungueltigen ungenutzten Quellen, obere
Flags, erhaltene Allocatorbytes, u64-Cachewrap, 65.536 Wedges und ein 65.536-Face-
Gruppenueberlauf. **267 AbsMax-Sonden**: 264 Originalvergleiche, alle acht
Nullvorzeichenkombinationen und drei sichere nichtfinite Fehler.

Gemeinsame Strecke: **50 Sequenzen / 146 Anfragen**, **146 Project- und 146
integrierte AbsMax-Aufrufe**, **2.758.547 Originalinstruktionen**, 12/34 fruehe
Sektions-/Actor-Gates, **100 Vertexcallbacks** und je 100 Vertex-/Indexuploads,
1.845 erzeugte Indizes, **348 Draws / 1.462 Materialpass-Durchlaeufe**.
**12 transaktionale Fehlerfaelle** pruefen Quellen, Allocation, AbsMax, Projektion,
LOD, Vertexcallback, Collapse, Unlock und Materialbank.

Ueber eigenstaendige und integrierte Vorbereitungssonden zusammen: 154
Vorbereitungslaufe, 38 Skips, 65.989 UV-Kopien, 66.370 Faces, 418 konstruierte
Sektionen, 836 Multiplikations- und 726 Arrayhelpergrenzen; 2.702.817
Vorbereitungsinstruktionen. Diese Zahl ueberlappt mit dem gemeinsamen Replay und
darf nicht dazuaddiert werden. Die eigenstaendigen AbsMax-Proben verwenden
weitere 7.213 Instruktionen. Instruktionszahlen sind keine Fortschrittsprozente.

**14 neue Rusttests, insgesamt 879 Workspace-Tests bestanden.** Debug/Release
bytegleich; Clippy alle Targets mit `-D warnings`, Format und Diff bestanden.
Berichte `analysis/reports/vertmesh-preprocess*`, Evidenz
`vertmesh_preprocess_validation`. Originalexporte: `vertmesh-preprocess`,
`vertmesh-preprocess-helpers`, `raw-index-ctor` und `vector-absmax` unter
`analysis/decompiled`.

```powershell
python scripts/Generate-VertMeshPreprocess.py
cargo run -p rc-inspect --bin rc-vertmesh-preprocess-check -- analysis/reports/vertmesh-preprocess.input.json analysis/reports/vertmesh-preprocess.json
cargo run --release -p rc-inspect --bin rc-vertmesh-preprocess-check -- analysis/reports/vertmesh-preprocess.input.json analysis/reports/vertmesh-preprocess-release.json
cargo test --workspace *> analysis/reports/vertmesh-preprocess-tests.log
python scripts/Record-VertMeshPreprocess.py
```

Offen bleiben Meshtransform, Szenenmatrixkonstruktion, FPTAN, animierte
Vertexpositionen/Normalen und GetFrame-Fuellung, echte Materialkompilierung und
COM-/GPU-Ausfuehrung. Das Spiel ist noch nicht spielbar. Androidpruefung zum Schluss.
