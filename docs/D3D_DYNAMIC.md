# Dynamische Ringbuffer und Rendererbindungen

Stand: 2026-10-09. Implementierung: `crates/rc-package/src/d3d_dynamic.rs`; Diagnose: `rc-d3d-dynamic-check`. Fuenf Aufgaben sind im abgegrenzten direkten Uploadpfad abgeschlossen.

## 1. Vertexring planen

Originalfunktion: D3DDrv 1002d610. Das erste GetSize-Ergebnis wird bei negativem Vorzeichen umlaufend negiert. Es bestimmt die Lockgroesse und die vorzeichenbehaftete Wachstumspruefung gegen Wrapper+38. Ein zweites, getrenntes GetSize-Ergebnis erzwingt bei negativem Vorzeichen DISCARD.

Die Platzpruefung bildet umlaufend `Cursor + Stride + Lockgroesse`, vergleicht diesen Wert vorzeichenbehaftet mit der Kapazitaet und anschliessend dessen vorzeichenlosen Quotienten durch Stride mit Device+420c. Der Vertexring reserviert damit eine zusaetzliche Stride. Bei einem Ueberlauf auf Ringanfang werden Cursor+40 null und Lockflags 2000 (DISCARD); sonst gelten 1000 (NOOVERWRITE).

Der Lockoffset ist `((Cursor + Stride - 1) / Stride) * Stride` mit nativer vorzeichenbehafteter Division und umlaufender DWORD-Arithmetik. Der Rueckgabewert ist wiederum Lockoffset / Stride. Wachstum und anschliessender Ringreset erhoehen den gemeinsamen Device-Zaehler +10078 separat; ein Upload kann ihn zweimal erhoehen. Groesse null wird erhalten. Division durch null und IDIV-Quotientenueberlauf werden sicher abgewiesen.

## 2. Dynamischen Vertex-Lock fuellen

Der aktive Slot an Wrapper+44 waehlt Handle +30 oder +34; er wird durch diesen Upload nicht umgeschaltet. Der direkte Pfad plant Lock (+2c) mit Offset, Groesse, Outslot und Flags, den Quellcallback +24 mit dem Lockzeiger und Unlock (+30). Danach wird Cursor = Lockoffset + Lockgroesse gespeichert und der aktive Slot zurueckgegeben.

Ein bereitgestellter CPU-Spiegel beginnt am bereits vom Lock gelieferten Zeiger. Der GPU-Byteoffset wird nicht noch einmal auf diesen Spiegel addiert. Explizite Callbackbytes fuellen dessen Praefix; Restbytes bleiben erhalten. Die API begrenzt den Lockbereich auf 16 MiB und prueft Zielkapazitaet sowie erfolgreiche Lock-/Unlockantworten vor Writes. Fehler erhalten Ring, Device-Zaehler und Zielpuffer. Der gespeicherte Quellzeiger +3c wird erst in der spaeteren Bindefunktion gesetzt.

Die Resize-Funktion bei 1002a5f0 bleibt eine externe Grenze: Bei Wachstum werden ausdruecklich gelieferte erfolgreiche Doppelhandles, neue Kapazitaet und Cursor null uebernommen. Ihre Create-/Retry-/Eviction- und Cacheinvalidierungsaufrufe werden in diesem Block nicht ausgefuehrt oder als rekonstruiert bezeichnet. Die untersuchten Originalbefunde stehen in `d3d-dynamic-buffers.c` und `d3d-dynamic-vertex.asm`.

## 3. Dynamischen Indexring planen und fuellen

Originalfunktion: 1002d990. Die Kapazitaet liegt an +34, der Cursor an +38, der Handle an +30. Wachstum setzt Kapazitaet auf die rohe Quellgroesse und Cursor null. Erst ein vorzeichenbehaftet groesserer Wert von Cursor + Groesse erzwingt einen Ringreset. Anders als beim Vertexring darf der Indexring seine Kapazitaet exakt ausnutzen; es gibt keine Strideausrichtung und keine Mindestgroesse von zwei Bytes.

Lock/Quellcallback +14/Unlock werden im direkten Pfad analog geplant und in einen CPU-Spiegel gefuellt. Der abschliessende Cursor ist alter Lockoffset + Groesse. Der zurueckgegebene Indexoffset ist vorzeichenbehaftet Lockoffset / Breitengetter. Diese Getterantwort bleibt vom gespeicherten Erzeugungsformat an +3c getrennt. Bei Wachstum wird ein ausdruecklicher erfolgreicher Resizehandle der externen Funktion 1002a880 verwendet; deren COM-/Retrylogik bleibt offen.

## 4. Dynamischen Stream binden

Originalfunktion: 10020540. Slot null bekommt den aktiven Ringhandle und die volle Stride. Der Renderer speichert den Wrapper, aber nur das untere Stridebyte. Alle alten Slots von eins bis PreviousStreamCount-1 werden bedingungslos in Renderer und Desired-Cache geloescht, auch wenn ihr Rendererwrapper bereits null war. Ihre Stridebytes und alten Komponentenbeschreibungen bleiben erhalten.

StreamCount und PreviousStreamCount werden eins; die erste Komponentenbeschreibung wird uebernommen. Der aufgeloeste Shader wird immer installiert, HardwareVertex wird null und der Ring speichert die Quelle an +3c. Dirtybits 8 und 4 werden gesetzt. Ein weiterer ausdruecklicher GetSize-Wert wird roh zum Bytezaehler Device+100a0 addiert; negative Groessen werden fuer diese Statistik nicht absolut genommen. Shadererzeugung und Quellengetter bleiben explizite Antworten.

## 5. Dynamischen Index binden

Originalfunktion: 10020a90. Breitengetter vier waehlt Device+40c4; jeder andere Wert waehlt +40c0. Die beiden Poolwrapper werden weiterhin extern aufgeloest. Der ausgewaehlte Wrapper und sein Handle werden gebunden, der uebergebene Basisvertex bleibt separat vom zurueckgegebenen Ring-Indexoffset. Dirtybit 8 wird gesetzt, die finale rohe Quellgroesse wird umlaufend zum Zaehler +100c8 addiert. Es gibt in dieser dynamischen Bindefunktion keinen statischen Nullquellen-/Nullgroessen-Unbindpfad.

## Nachweis und Grenzen

`Generate-D3DDynamic.py` bindet die Eingaben per SHA256 an den vorherigen statischen Pipeline-Nachweis. 48 synthetische Sequenzen mit je vier wechselnden Vertex-/Indexschritten pruefen Wachstum, Wiederverwendung, negative und wiederholte Groessengetter, ungerade Strides, beide aktiven Handles, beide Indexpools, Ringlimits, Nullgroessen, Zielrestbytes, Shaderhandles null und umlaufende Zaehler. Zehn Fehlerfaelle pruefen den gemeinsamen Rollback des Diagnoseadapters, darunter ein erst nach erfolgreichem CPU-Upload fehlender Shader.

`Record-D3DDynamic.py` interpretiert die exportierten Originalinstruktionen der Ring- und Bindekoerper. GetSize/GetStride/GetIndexSize, Fuellcallback, Shaderbeschaffung und Resizehelper erhalten ausdrueckliche Antworten an ihren ABI-Grenzen. SEH, Zeitmessung, Logging und Returns werden ausgeklammert. Die Byte-VM wurde um CDQ, DIV, IDIV und zweistelliges IMUL erweitert; vorzeichenbehaftete Division rundet gegen null und prueft den Quotientenbereich. Der Fuellcallback selbst wird nicht als rekonstruierte Spielberechnung behauptet.

Verglichen werden alle modellierten Ringfelder, Device-Zaehler, geordnete Lock/Fuell/Unlock-Aufrufe, vollstaendige CPU-Zielpuffer, Rendererfelder und Deferred-Cache nach jedem Schritt. Ergebnis: **23697 interpretierte Instruktionen**, **192 Schritte** (96 Vertex / 96 Index), **4504 Callbackbytes**, **24 Vertex-/36 Index-Resizegrenzen**. Vertex: 101 Discard-Zaehlerereignisse, 19 NOOVERWRITE-Uploads. Index: 53 DISCARD- und 43 NOOVERWRITE-Uploads. Poolauswahl 72-mal klein / 24-mal 32 Bit. **15 neue Tests**, insgesamt **731 Workspace-Tests**; Debug/Release bytegleich, Clippy `-D warnings`, Format- und Diffpruefung bestanden.

```powershell
python scripts/Generate-D3DDynamic.py
cargo run -p rc-inspect --bin rc-d3d-dynamic-check -- analysis/reports/d3d-dynamic.input.json analysis/reports/d3d-dynamic.json
cargo run --release -p rc-inspect --bin rc-d3d-dynamic-check -- analysis/reports/d3d-dynamic.input.json analysis/reports/d3d-dynamic-release.json
cargo test --workspace *> analysis/reports/d3d-dynamic-tests.log
python scripts/Record-D3DDynamic.py
```

Artefakte: `analysis/reports/d3d-dynamic.input.json`, `d3d-dynamic.json`, `d3d-dynamic-release.json`, `d3d-dynamic-validation.json`, `d3d-dynamic-tests.log`; Evidenzschluessel `d3d_dynamic_validation`. Neue native Befunde: `d3d-dynamic-research.c`, `d3d-dynamic-buffers.c`, `d3d-dynamic-ring.c`, `d3d-dynamic-setup.asm`, `d3d-dynamic-vertex.asm`, `d3d-dynamic-index.asm`, `d3d-dynamic-ring.asm`.

Offen bleiben Resizehelper-/Poolkonstruktor-Ausfuehrung, der Scratchpfad Device+4114, Verbindung dynamischer Rueckgabeoffsets mit der Draw-Planung, echte GPU-/Allocatorausfuehrung, Skinningcallback, Shadererzeugung und Live-Spiel. Nichtnull-Scratchmodus wird vor Mutation sicher abgewiesen; native Fehlerpfade werden nicht ausgefuehrt. Die sicheren Porttransaktionen sind keine Behauptung identischer nativer Fehlerbehandlung. Diffuse-Abdeckung weiterhin 271/289; Android-Pruefung zum Schluss.
