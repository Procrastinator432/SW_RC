# Shader-, Stream- und Indexbindungen im gemeinsamen Aufrufplan

Drei Aufgaben ergaenzen den bislang rekonstruierten Render-/Buehnen-/Transform-/Texturpfad: nativen Shader-Ausgabecache, Stream-/Index-Ausgabecache und deren geordnete Integration. Die Implementierung `rc_package::d3d_bindings` beruht auf `D3DDrv!10028f10`, Abschnitt `10029dd9..10029ef0`. Sie plant Methodenargumente und ruft keine Grafik-API auf.

## 1. Gemeinsame Shadergruppe

`Bindings` speichert `desired` und `applied` mit Vertexshaderwort, Pixelshaderwort, sechzehn Stream-Paaren und einem Indexpaar. Diese Felder liegen nativ bei Cache+628/+62c, +630 und +6b0; ihre Ausgabecaches bei den jeweils um a34 hoeheren Adressen.

Dirtybit 4 steuert beide Shaderwoerter. Bei Abweichung wird zuerst das Vertexshaderwort im Ausgabecache aktualisiert und ein Aufruf fuer VTable-Offset 130 mit einem Rohargument geplant. Der Pixelshadervergleich und Offset 160 folgen erst nach der Streamschleife. Gleiche Woerter unterdruecken den Aufruf; auch Null oder hohe unbekannte Bits werden unveraendert weitergegeben. Das Vertexwort kann ein nativer Handle/FVF-Wert sein; hier findet keine Dekodierung oder Programmbeschaffung statt.

Wie bei den bisher rekonstruierten Gruppen schreibt das Original den optimistischen Ausgabecache vor dem Methodenaufruf und ignoriert dessen HRESULT. `applied` ist daher keine bestaetigte GPU-Ausfuehrung. Die Gegenprobe verwendet weiterhin den Fehler-HRESULT 80004005, ohne die originalen Cachewrites zu unterdruecken.

## 2. Streams und Indexbuffer

Dirtybit 8 steuert Stream- und Indexpaare. Die Streamkapazitaet stammt nativ aus Geraet+4210, unabhaengig von der Texturstufenkapazitaet +41e8. Die native Schleife behandelt sie signiert, begrenzt nach oben auf 16 und laeuft bei negativen Werten nicht. Die Rust-API erhaelt deshalb einen `i32` und verwendet die entsprechende Grenze 0..16.

Jeder Stream besteht aus Bufferhandle und Stride. Aendert sich mindestens ein Wort, werden beide Ausgabewoerter uebernommen und Offset 14c mit `(Slot, Buffer, Stride)` geplant. Ein Nullhandle wird nicht ausgesondert; auch eine reine Strideaenderung erzeugt einen Aufruf. Streams oberhalb der Grenze bleiben unveraendert im Ausgabecache.

Nach dem Pixelshader wird das Indexpaar aus Bufferhandle und Basisvertex verglichen. Eine Abweichung in einem der beiden Woerter uebernimmt beide und plant Offset 154 mit `(Buffer, Basisvertex)`. Dieser Vergleich erfolgt unabhaengig von der Streamkapazitaet, auch bei null oder negativer Kapazitaet. Basisvertex bleibt ein rohes u32-Wort. Bufferinhalt, Groesse, Vertexlayout und Drawaufruf werden nicht rekonstruiert.

Die exakte Reihenfolge von `Bindings::tail` ist:

1. Vertexshader, sofern Dirty 4 und geaendert.
2. Streams in aufsteigender Slotreihenfolge, sofern Dirty 8 und geaendert.
3. Pixelshader, sofern Dirty 4 und geaendert.
4. Indexpaar, sofern Dirty 8 und geaendert.

Die Methode erhaelt die Dirtymaske als Eingabe und loescht sie nicht selbst; dadurch laesst sich dieser originale Teilabschnitt separat pruefen. Ein erneut markierter unveraenderter Durchlauf ist leer. Bei spaeterer erweiterter Kapazitaet und erneuter Markierung werden zuvor zurueckgestellte Streams ausgegeben.

## 3. Integrierte Ausgabe

`Deferred` fasst die vorhandenen `Cache`-/`Transforms`-Zustaende und `Bindings` zusammen. `Deferred::flush` unterstuetzt jetzt alle Gruppenbits 1/2/4/8/10/20/40 und erzeugt `Plan` mit dem bisherigen geordneten Zustandsplan und anschliessenden Bindingaufrufen:

`states.before -> states.transforms -> states.after -> bindings`

Das entspricht Render-/Buehnenzustaenden, Matrizen, Texturen und dem obigen Shader-/Stream-/Indexabschnitt. Die vorhandenen Nebel-/Stencil-Gates, Matrixmasken, Kapazitaeten und Vergleichsregeln bleiben erhalten. Am Ende sind Dirtywort und gesamte Transformmaske null. Die intern getrennte Planung liefert dieselben finalen Cachewerte und Methodenargumente wie der originale zusammenhaengende Ausgabepfad.

Die Lichtgruppe 80 und unbekannte Dirtybits sind vor jeder Mutation ausgeschlossen. Texturstufenkapazitaeten ueber acht werden ebenfalls vorher abgewiesen; die signierte Streamkapazitaet ist davon unabhaengig. Alte Teil-APIs behalten ihre engeren Vertraege. Ein spaeteres Backend muss den Plan in dieser Reihenfolge ausfuehren und Geraeteverlust/Fehler selbst behandeln.

## Unabhaengige Originalbefehlspruefung

`Generate-D3DBindings.py` prueft SHA256 von vorherigem Matrixbericht und dessen Eingaben. 482 bereits gegen Originalanweisungen gepruefte Hardwarepass-Sollstaende dienen als Ausgangspunkt der kombinierten Ausgabe. Darunter sind 64 gueltige Materialpraefixe. Diese Staende werden nicht als neu erfasste Live-Szenen dargestellt; Shader-/Bufferbindungen sind explizite zusaetzliche Fixtures. 128 weitere Faelle decken jede Dirtymaske 0..7f ab, insgesamt 610 kombinierte Faelle.

704 separate Bindingfaelle kombinieren die vier Gates 0/4/8/c mit 22 signierten Streamkapazitaeten (einschliesslich i32-Minimum/Maximum, -1, 0, 1..16 und Werten ueber 16) und acht Mustern fuer geaenderte/equal/null Shader- und Bufferwoerter. Stride-/Basisvertexaenderungen, alle sechzehn Streamslots, leere Wiederholungen und erneute Ausgabe mit erweiterter Kapazitaet sind enthalten.

`Record-D3DBindings.py` fuehrt separat die exportierten x86-Anweisungen fuer den Bindingabschnitt und alle bereits unterstuetzten Ausgabeschleifen aus. Die vorhandene Byteadress-VM wurde fuer die vier neuen Methodenoffsets um deren exakte Argumentanzahlen erweitert. Native Rohwrites, Soll-/Ausgabecaches, Matrixdaten/-masken und geordnete Aufrufe werden mit Rust verglichen. Matrixmasken werden erst nach dem Bindingabschnitt nativ geloescht; Profiling/Statistik und Exceptionverwaltung sind ausgeschlossen. Keine native GPU-Ausfuehrung erfolgt.

```powershell
python scripts/Generate-D3DBindings.py
cargo run -p rc-inspect --bin rc-d3d-bindings-check -- analysis/reports/d3d-bindings.input.json analysis/reports/d3d-bindings.json
cargo run --release -p rc-inspect --bin rc-d3d-bindings-check -- analysis/reports/d3d-bindings.input.json analysis/reports/d3d-bindings-release.json
cargo test --workspace *> analysis/reports/d3d-bindings-tests.log
python scripts/Record-D3DBindings.py
```

Ergebnis: **704 Bindingfaelle**, **610 kombinierte Ausgaben**, **1490159 Originalanweisungen**, **18066 Shader-/Stream-/Indexaufrufe**, **2884 Transformaufrufe** und **42192 Render-/Buehnen-/Texturaufrufe** bestaetigt, insgesamt **63142 Methodenaufrufe**. Alle 128 Dirtymasken, vier Bindinggates, 22 signierte Streamkapazitaeten, 16 Streamslots und Texturkapazitaeten 0..8 geprueft; 64 vorherige gueltige Materialpraefixe enthalten. Wiederholungen leer, zurueckgestellte Bindungen bei erneuter Markierung/erweiterter Kapazitaet korrekt ausgegeben. Debug/Release bytegleich. Sieben neue Tests, **629 Workspace-Tests**; Clippy `-D warnings`, Format- und Diffpruefung bestanden.

Artefakte: `analysis/reports/d3d-bindings.input.json`, `d3d-bindings.json`, `d3d-bindings-release.json`, `d3d-bindings-validation.json`, `d3d-bindings-tests.log`; Evidenzschluessel `d3d_bindings_validation`. Fruehere Berichte bleiben historische Pruefstaende mit ihren jeweiligen Quellhashes und Testzahlen.

Offen bleiben Lichtausgabegruppe 80, feste Pixelshaderauswahl/Sonderkonstantenupload, Shader-/Buffer-/Ressourcenbeschaffung und Lebensdauer, echte Szenenquellen, Draw-/Backend-Anbindung und spielbare Engine. Die fertige Bindingplanung bestaetigt keinen GPU- oder Bildvergleich. Diffuse-Abdeckung bleibt 271/289; Android-Pruefung zum Schluss.


Fortsetzung/Korrektur `D3D_LIGHTS_AND_PASSES.md`: Lichtgruppe 80 und aufgeloeste feste Pixelshaderwahl/Sonderupload sind jetzt in einer zusaetzlichen gemeinsamen API vorhanden. Der vollstaendige native Flush-Eintritt wurde neu geprueft: Bei Dirty null kehrt er ohne Loeschen der Transformmaske zurueck. Die bisherigen Teilproben fuehrten direkt den Gruppenrumpf aus; dessen Loeschregel gilt nur nach einem von null verschiedenen urspruenglichen Dirtywort. Die oeffentlichen Transform-/Bindingplaner sind korrigiert und durch drei Regressionstests abgesichert. Historische Berichte behalten ihre damaligen Quellhashes und engeren Pruefbereiche. Ressourcen-/Containerbeschaffung und Backend bleiben offen.
