# Draw-Verbraucher und begrenzte Mehrpass-Ausgabe

Stand: 2026-10-09. Fuenf Aufgaben: Primitive-Dispatch, indizierte Argumente, Statistik, temporaere Nebelfarbe und Integration der Pass-/Zustands-/Draw-Reihenfolge. Implementierung: `crates/rc-package/src/d3d_draw.rs`; Diagnoseprogramm: `rc-d3d-draw-check`.

## 1. Primitive-Dispatch

Die Originalbereiche `D3DDrv!10020cb0/10020cf0` nehmen fuenf DWORD-Argumente entgegen: Primitiveart, Start, Primitiveanzahl, Minimalvertex und Maximalvertex. Rendererzustand +484 waehlt indizierte Ausgabe. Die Enginearten 1, 2, 5, 6 und 7 ergeben D3D8-Arten 1, 2, 4, 5 und 6. Andere Arten erzeugen keinen Draw-Aufruf, durchlaufen aber weiterhin Passzustandsausgabe und Statistik.

Nichtindiziert wird VTable +118 mit `[Art, Start, Anzahl]` geplant. Nullanzahlen bleiben erhalten; der Port fuehrt hier keine zusaetzliche Fruehrueckkehr ein.

## 2. Indizierte Argumente und Mesh-Uebergabe

VTable +11c erhaelt fuenf Argumente: `[Art, Minimalvertex, Maximalvertex - Minimalvertex + 1, Start, Anzahl]`. Die Bereichsrechnung erfolgt als umlaufende 32-Bit-Arithmetik, auch bei umgekehrten Grenzen oder Ueberlauf. Die Disassembly ist hier massgeblich; eine verkuerzte Decompiler-Signatur reicht zur Rekonstruktion nicht aus. Buffergrenzen werden in diesem rohen Aufrufplan nicht validiert.

`Draw::from_section` uebernimmt die bereits geprueften Skeletal-Draw-Abschnitte mit Dreiecksliste (Engineart 5), Startindex, Dreieckszahl und Vertexgrenzen; usize-Werte werden geprueft zu DWORD konvertiert. Drei echte CloneCommando-LOD0-Abschnitte mit zusammen 3500 Dreiecken gehen als indizierte Anfragen in die Mehrpassprobe ein. Die Uebergabe beschafft keine Vertex-/Indexbuffer und keine Abschnittsmaterialien.

## 3. Native Statistik

Device +fec0 wird pro verarbeitetem Pass inkrementiert, auch bei unbekannter Primitiveart. Device +fc90 addiert danach die Primitiveanzahl. Device +fcb8 addiert bei indizierten unterstuetzten Arten die umlaufende Vertexspanne. Nichtindiziert zaehlt Art 1 die Anzahl, Art 2 zweimal, Art 5 dreimal und Art 6/7 die Anzahl plus zwei. Unbekannte Arten addieren null Vertices. Die Statistik misst keine eindeutigen Vertices. Alle drei Zaehler laufen bei 32 Bit um.

## 4. Temporaere Nebelfarbe

Native state+17 ist ein separates Aktivierungsbyte. Ist es ungleich null und das Passflag +4 enthaelt Bit 40, wird das rohe FColor-Wort an Pass+1c in den Sollcache uebernommen (Cache+34, Renderindex 12, D3D8-Zustand 34) und Dirty 1 gesetzt. Der anschliessende gemeinsame Flush entscheidet wie bisher ueber die tatsaechliche Nebelzustandsausgabe anhand des gecachten FogEnable-Wortes.

Nach dem Draw setzt `10021063` den Sollwert auf das FColor-Wort state+2f8 zurueck und setzt Dirty 1. Es gibt keinen unmittelbaren Restore-Aufruf und keinen zusaetzlichen Flush nach dem letzten Pass. Der letzte Ausgabecache kann deshalb weiterhin die temporaere Farbe enthalten, waehrend die Wiederherstellung im Sollcache wartet. Der naechste Pass verarbeitet diesen Zustand regulaer.

Der Import an D3DDrv-IAT 1006f3e8 wurde bis `Engine.dll!??BFColor@@QBEKXZ` verfolgt. Der Export liegt mit der originalen Engine-Imagebase 10300000 bei 10338670. Seine Bytes `8b 01 c3` lesen das Wort an ECX unveraendert und kehren zurueck. Hier findet keine Farbkanalkonvertierung statt.

## 5. Mehrpass-Reihenfolge

`render` verarbeitet einen begrenzten Ausschnitt mit null bis acht vorbereiteten Passplaetzen. Ein leerer Durchlauf erhaelt alle Daten. Sonst werden Eingaben vor Writes auf unterstuetzte Dirtygruppen, Kapazitaet, Adressen, Stufenzahlen und benoetigte aufgeloeste Ressourcen/Shader geprueft. Identische Passadressen ueberspringen wie im Original die erneute Passuebersetzung, erzeugen aber weiterhin Nebelverarbeitung, Flush, Draw und Statistik.

Die Reihenfolge jedes `RenderedPass` ist:

1. Passauswahl samt optionalem unmittelbarem Pixelkonstanten-Upload.
2. Nebeloverride und kompletter Zustandsplan: Render-/Stufenwerte, Matrizen, Texturen, Lichter, Shader-/Bufferbindungen.
3. Optionaler Draw-Aufruf und umlaufende Statistik.
4. Im Sollcache vorgemerkte Nebelwiederherstellung.

Mehrere Plaetze duerfen dieselbe Passadresse verwenden. Sie muessen konsistente Eingangssnapshots besitzen; LOD-/Stufenmutationen werden nach jedem Durchlauf auf alle solchen Plaetze uebernommen. Damit bleibt gemeinsam genutzter nativer Passspeicher sichtbar. Widerspruechliche Snapshots werden vor Writes abgewiesen. Die Begrenzung auf acht Passplaetze ist eine Port-API-Grenze; sie wird nicht als vollstaendiger Wertebereich des nativen Bytezaehlers behauptet.

## Nachweis und Grenzen

`Generate-D3DDraw.py` verknuepft die hashgeprueften vorherigen Passfixtures und den Skeletal-Bericht. `Record-D3DDraw.py` interpretiert originale x86-Bereiche fuer Passschleifen-Gates, vorhandene Passuebersetzung/Flush, Draw-Dispatch, FColor-Zugriff und Wiederherstellung/Statistik. Profiling mit RDTSC, Exceptionverwaltung, externe Shader-/Ressourcenbeschaffung und echte GPU-Ausfuehrung bleiben ausgeschlossen. Die geprueften Bereiche werden mit expliziten ABI-/Speicherfixtures verbunden; es wird keine vollstaendige Enginefunktion unter Windows ausgefuehrt.

```powershell
python scripts/Generate-D3DDraw.py
cargo run -p rc-inspect --bin rc-d3d-draw-check -- analysis/reports/d3d-draw.input.json analysis/reports/d3d-draw.json
cargo run --release -p rc-inspect --bin rc-d3d-draw-check -- analysis/reports/d3d-draw.input.json analysis/reports/d3d-draw-release.json
cargo test --workspace *> analysis/reports/d3d-draw-tests.log
python scripts/Record-D3DDraw.py
```

Ergebnis: **480 Draw-Faelle**, **524 Nebel-Faelle**, **195 Mehrpass-Sequenzen**, **1263569 Originalanweisungen**. Enthalten: 22 leere Sequenzen, 765 verarbeitete Paesse, 256 gemeinsam genutzte Passplaetze, 280 Identitaets-Skips, 2841 aktive Stufenuebersetzungen und 428 Nebeloverrides in Sequenzen. Zehn Primitivearten, beide Indexzweige, Null-/Ueberlaufzahlen, alle 256 Passflagbytes und vier Aktivierungsbytewerte werden abgedeckt. Drei originale Meshabschnitte liefern 3500 Dreiecke. Debug/Release sind bytegleich; 13 neue Tests ergeben **655 Workspace-Tests**. Clippy `-D warnings`, Format- und Diffpruefung bestanden.

Artefakte: `analysis/reports/d3d-draw.input.json`, `d3d-draw.json`, `d3d-draw-release.json`, `d3d-draw-validation.json`, `d3d-draw-tests.log`; Evidenzschluessel `d3d_draw_validation`. Vorherige Nachweise bleiben historische Staende mit damaligen Quellhashes und Testzahlen.

Offen bleiben echte Buffer-/Textur-/Shaderressourcen mit Lebensdauer, Materialbeschaffung pro Meshabschnitt, Live-Szenenquellen, ausfuehrendes Grafikbackend und die spielbare Engine. Dieser Stand belegt CPU-Aufrufplaene, keine GPU-/Bildgleichheit. Diffuse-Abdeckung bleibt 271/289. Android-Pruefung wie vereinbart zum Schluss.
