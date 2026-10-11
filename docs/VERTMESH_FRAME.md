# Vertex-Mesh GetFrame: gepackte Frames bis zum Vertexupload

Stand: 2026-10-10. Fuenf Aufgaben umgesetzt: Positionsdecode, Normalendecode,
Framepaar-Interpolation, stridierte Positions-/Wedgeausgabe und Integration mit
Indexaufbau/Upload/Materialdraw samt Originalreplay. Modul `vertmesh_frame.rs`,
Adapter `rc-vertmesh-frame-check`.

## Originalumfang

`UVertMeshInstance::GetFrame` liegt bei Engine!1055f080. Der direkte PE-Wert
am Instance-vtable 1067bd88 +dc zeigt auf diese Funktion. Export unter
`analysis/decompiled/vertmesh-getframe.{c,asm}`. Der native signed Countclamp
1055f09e..1055f0c5, Countpointer-/Instance+94-Writeback, Decode-/Interpolations-
schleifen 1055f2b0..1055f599, Ausgabe 1055f59c..1055f646 und Flags +60/+61
werden interpretiert. Weitere Instruktionen in der Exportdatei gehoeren zu
nachfolgenden Funktionen und werden nicht als GetFrame ausgefuehrt.

Cachearray-Allokation/-Invalidierung, GetSequence, Animationszeit, Frameindex-
Modulo, MXCSR-Wechsel des Schedulers, SEH und finaler Unwind bleiben ausserhalb
dieses Nachweises. Die API erhaelt einen Cache-Snapshot und bereits ausgewaehlte
gepackte Framepaare mit gespeichertem Interpolationsfaktor. Sie behauptet damit
noch keine vollstaendige Animation aus einem laufenden Actor/Animationskanal.

## 1. Gepackte Positionen

Ein u32 enthaelt signed X/Y/Z mit 11/11/10 Bits. Die nativen Shiftfolgen sind:

`X = ((word << 21) as i32) >> 21`

`Y = ((word << 10) as i32) >> 21`

`Z = (word as i32) >> 22`

Die Werte werden mit CVTSI2SS nach float32 konvertiert. X/Y liegen bei
-1024..1023, Z bei -512..511. Das rohe Wort ist kein IEEE-Float; auch Muster
wie 7fc00000 werden als gepackte Integerposition interpretiert.

## 2. Gepackte Normalen

Normalen verwenden drei unsigned 10-Bit-Felder. Die obersten zwei Bits sind
ungenutzt. Je Komponente wird 512 subtrahiert, sodass -512..511 entsteht.
Die Konstante bei 10671f1c ist direkt aus dem Engine-PE als 44000000 geprueft.
Es gibt hier weder Division durch 512 noch Normalisierung auf Laenge eins.

Das Original nutzt FILD, FSUB und FSTP zwischen Integerfeldern und float32-
Zwischenspeichern. Die Replay-VM unterstuetzt ausdruecklich nur diese exakte
Teilmenge: Integer 0..1023, Subtraktion der Konstanten 512 und Floatstore.
Alle Ergebnisse sind exakt darstellbare kleine Integer; eine Emulation beliebiger
x87-Genauigkeit oder FPTAN wird daraus nicht abgeleitet. Unreachable FADD-Zweige
fuer negative unsigned Feldwerte werden nicht stillschweigend simuliert.

## 3. Framepaar-Interpolation und Cache

Der ausgegebene Count ist signed `min(requested_count, vertex_count)`.
Nur die ersten Count-Positionen und -Normalen des Caches werden ersetzt;
der Rest bleibt bitgenau erhalten, auch wenn ungenutzte Worte NaN-Muster tragen.
Instance+94 und Countpointer erhalten diesen geklemmten Count. Beide Flagbytes
+60/+61 werden auf eins gesetzt, auch bei Count null.

Bei vorhandenem naechstem Frame und positivem Faktor laufen getrennte
float32-Schritte `(B-A)*factor + A`, ohne FMA oder Normalisierung. Faktor null,
negative Null und negative Faktoren verwenden den ersten Frame direkt und
lesen keine naechsten Framearrays. `next=None` bezeichnet den direkten Pfad ohne
Animationssequenz. Der explizite Faktorenvertrag laesst auch endliche Werte
groesser eins zu; dies belegt keine solchen Faktoren aus dem originalen Scheduler.

## 4. Stridierte Ausgabe

Genau Mode 1 kopiert Count Positionen zu je zwoelf Bytes. Wedges werden dabei
nicht ausgewertet, Normalen im Cache aber trotzdem berechnet. Alle anderen
Modewerte iterieren die Wedgezuordnungen. Mapwerte kleiner Count kopieren XYZ
und Normalen (24 Bytes); andere Mapwerte lassen den gesamten Record unveraendert.
Der Zielzeiger steigt in beiden Faellen um Stride. UV-Worte bei +18/+1c und
Padding werden im ueblichen 32-Byte-Layout erhalten.

`Input::build` liefert Count, Cache, vollstaendiges Byteimage und Interpolations-
pfad, ohne den Input zu veraendern. Sichere Grenzen: Vertex-/Requestcounts und
Wedges maximal 65.536, keine negativen Requestcounts, Cachelaengen gleich
Vertexcount, aktive Framearrays ausreichend, endlicher Faktor und endliche
Interpolationsergebnisse. Aktive Zielrecords verlangen Stride >=12 bzw.24,
maximal 4096, ausreichendes Byteimage und insgesamt maximal 16 MiB. Ueberlappende
Records und native ungueltige Speicherzugriffe werden als Portfehler abgewiesen.

## 5. Verbindung zu Geometrie und Draws

`Runtime::submit_frame` verbindet den vollen Meshmodus 0 / Stride 32 mit der
vorhandenen `vertmesh_indices::Request`. Requestcount muss dem vor Callback
gesicherten Geometrie-LOD entsprechen; die Wedgezuordnungen muessen den
urspruenglichen WORDs der Geometriequelle entsprechen. Der geklemmte Callback-
Count ersetzt diesen Geometrie-LOD nicht. Dies wird auch bei Request 5 und nur
vier Basisvertices geprueft.

Die erzeugten Bytes ersetzen VertexSource-Payload, beide Sizeantworten und
Stride. Danach laufen Collapse-/Indexgenerierung, Sektionsheadermutation,
Scratch-/Ringupload, Materialrouting und komplette Drawpasses. Cache, Zielimage,
Request und Runtime werden nur bei Gesamterfolg uebernommen. Bei spaeterem
Index-, Unlock- oder Materialfehler bleibt auch der Framecache unveraendert.

Dieser Adapter beginnt beim bereits ausgewaehlten Framepaar/Geometrie-LOD.
Er ist noch nicht mit Actor-Zeitsteuerung oder dem ganzen Render/GetFrame-
Aufrufbaum verbunden. Die bisherige PreProcess-/Project-/Camera-API bleibt
erhalten; eine vollstaendige Spielintegration wird nicht behauptet.

## Nachweis und Grenzen

Der Generator verwendet SHA256-gepruefte vorbereitete Sektions-/UV-Snapshots
aus dem vorherigen Bericht und synthetische gepackte Framepaare. Eine dort
bereits ausgefuehrte Zweipass-Materialantwort wird fuer die neuen aktiven
Sektionen verwendet; ihre Passaliase bleiben konsistent. Gelieferte Scratch-/
Lockzielimages werden fuer die neuen 160-Byte-Vertexpayloads ausreichend gross
angelegt. Keine Live-Spiel-/GPU-Aufnahme und kein aus Originalassets abgespielter
Animationsclip. Materialien, Pool-/Allocator- und COM-Antworten bleiben Grenzen.

**265 eigenstaendige Frameproben** vergleichen Clamp, direkte und interpolierte
Frames, alle Ausgabemodi, verschiedene Strides, Cachetails, Flagbytes, Bitgrenzen,
Wedge-Skips und UV-/Paddingerhaltung. **Neun sichere Framefehler** pruefen Counts,
Cache-/Framelaengen, Faktoren, Stride, Zielimage und Interpolationsueberlauf.

**50 Sequenzen / 146 gemeinsame Anfragen**, **678.349 Originalinstruktionen**,
je 146 Vertex-/Indexuploads, 2.628 erzeugte Indizes, **438 Draws / 876 Passes**.
**Acht transaktionale Fehlerfaelle** pruefen Layout-/Count-/Quellkonflikte und
spaete Index-/Unlock-/Materialfehler mit unveraendertem Cache, Request und Runtime.

Ueber eigenstaendige und gemeinsame Framepruefungen zusammen: **411 Frameaufrufe**,
1.650 direkt decodierte und 1.077 interpolierte Vertices, 732 Positionskopien,
1.003 Wedgekopien und 595 Wedge-Skips; **319.472 Frameinstruktionen**. Diese Zahl
ueberlappt mit dem gemeinsamen Replay und darf nicht dazuaddiert werden.
Der vorherige PreProcess-/GetAbsMax-/Projectnachweis mit 2.758.547 gemeinsamen
Instruktionen wurde nach der x87-Teilmengenerweiterung erneut bestanden.

**14 neue Rusttests, insgesamt 893 Workspace-Tests bestanden.** Debug/Release
bytegleich; Clippy alle Targets mit `-D warnings`, Format und Diff bestanden.
Berichte `analysis/reports/vertmesh-frame*`, Evidenz `vertmesh_frame_validation`.

```powershell
python scripts/Generate-VertMeshFrame.py
cargo run -p rc-inspect --bin rc-vertmesh-frame-check -- analysis/reports/vertmesh-frame.input.json analysis/reports/vertmesh-frame.json
cargo run --release -p rc-inspect --bin rc-vertmesh-frame-check -- analysis/reports/vertmesh-frame.input.json analysis/reports/vertmesh-frame-release.json
cargo test --workspace *> analysis/reports/vertmesh-frame-tests.log
python scripts/Record-VertMeshFrame.py
```

Offen bleiben GetFrame-Cacheallokation/-Invalidierung, Sequenzwahl und Zeit-/
Framescheduling, Meshtransform, Szenenmatrixkonstruktion, FPTAN sowie echte
Materialkompilierung und GPU-Ausfuehrung. Noch nicht spielbar; Android zum Schluss.
