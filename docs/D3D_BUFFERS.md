# Statische Buffer-Uebergabe und Draw-Anbindung

Stand: 2026-10-09. Fuenf Aufgaben: statische Indexbindung, Stream-Beschreibungen, Stream-Bindungen/Uploadentscheidungen, fester Vertexshader-Restore und gemeinsame Vorbereitung bis zum Mehrpass-Draw. Implementierung: `crates/rc-package/src/d3d_buffers.rs`; Diagnose: `rc-d3d-buffers-check`.

Die Referenzen sind D3DDrv-Funktionen 10020840, 10020220 und 100201c0. `analysis/decompiled/d3d-buffer-setup.asm` enthaelt die Originalinstruktionen; `d3d-buffer-setup.c` die zusaetzlichen Decompilerbefunde. Alle Ressourcen-/Shaderadressen und Geraetehandles sind hier bereits aufgeloeste Eingaben. Cachelookup, Wrapperkonstruktion, GPU-Upload und Shaderbeschaffung sind weiterhin externe Schritte.

## 1. Statischer Indexbuffer

Ein Nullquellzeiger oder ein Quellgroessenwert null folgt dem Unbind-Zweig. Dieser loescht Renderer-Wrapper (+484), Basisvertex (+488) und das Soll-Indexpaar im Deferred-Cache nur dann, wenn der bisherige Wrapper ungleich null ist. War der Wrapper bereits null, bleiben auch ein widerspruechliches Sollpaar und ein alter Basiswert unveraendert. Der uebergebene neue Basiswert spielt beim Unbind keine Rolle.

Bei einer vorhandenen Quelle mit Groesse ungleich null werden die Quellrevision und die gecachte Wrapperrevision (+10) verglichen. Eine Abweichung fordert den externen Cache-/Upload-Schritt an; der native Rueckgabewert dieser Uebergabefunktion ist dann der rohe Quellgroessenwert aus virtuellem Aufruf +10, sonst null. `IndexPlan` bildet diese Entscheidung und den Wert ab. Die konkrete Bedeutung des Groessenwertes wird nicht aus der Draw-Dreieckszahl abgeleitet.

Nach Cacheaufloesung wird Device+46a8 nach Wrapper+14 geschrieben. Renderer und Sollcache erhalten Wrapper, dessen bereits aufgeloestes Handle (+30) und den uebergebenen Basisvertex; Dirty 8 wird immer gesetzt, auch bei unveraenderten Werten. Ein Nullgeraetehandle ist zulaessig und bedeutet bei vorhandenem Wrapper keinen Wechsel zum nichtindizierten Draw.

## 2. Stream-Beschreibungen und Restplaetze

Renderer+33c enthaelt sechzehn Beschreibungen zu je fuenf DWORDs: vier vom virtuellen GetComponents-Aufruf +20 geschriebene Payloadworte und dessen Rueckgabewort. Alle aktiven Beschreibungen werden kopiert, auch bei unveraendertem Inhalt. Die API interpretiert diese Worte noch nicht als Plattform-Vertexlayout.

Vor der Uebernahme neuer Streams durchlaeuft das Original den Bereich neue Anzahl bis bisherige Byteanzahl (+4dc). Nur bei einem vorhandenen alten Renderer-Wrapper (+48c + Slot*4) werden dieser Wrapper und das Sollpaar fuer den Slot geloescht und Dirty 8 gesetzt. Bei Nullwrapper bleibt ein widerspruechliches Sollpaar erhalten. Alte Beschreibungen und Stride-Schattenbytes werden dabei nicht geloescht. Der signierte aktive Zaehler +47c wird auf die neue Anzahl gesetzt; am Ende folgt das Byte +4dc.

## 3. Aufgeloeste Stream-Bindungen

Pro aktivem Slot werden Quellrevision und Wrapperrevision verglichen. Bei Abweichung wird der rohe virtuelle Quellwert +14 zur lokalen Uploadsumme addiert; die Summe laeuft mit 32 Bit um. `StreamPlan::upload_slots` und `upload_size` halten diese Entscheidungen fest. Die benoetigten Cache-/Uploadoperationen sind Voraussetzung der gelieferten Handles, sie werden von diesem CPU-Plan nicht ausgefuehrt und nicht als Uploadbytes auf einer GPU nachgewiesen.

Jeder gebundene Wrapper erhaelt den Framewert Device+46a8 an +14, unabhaengig von der Revision. `Usage` beschreibt diese Writes fuer den externen Ressourcenbesitzer. Der Deferred-Sollcache erhaelt das volle DWORD-Paar `[Handle, Stride]` und Dirty 8. Renderer+48c speichert den Wrapper; +4cc speichert zusaetzlich nur das niedrigste Stridebyte. Die volle Stride wird nicht auf diesen Bytewert gekuerzt. Der Ausgabecache bleibt bis zum Flush unveraendert.

## 4. Fester Vertexshader-Restore

Der eigenstaendige Restore 100201c0 tut bei signiertem Streamzaehler kleiner oder gleich null nichts. Bei positivem Zaehler liefert der externe GetVertexShader-Aufruf einen aufgeloesten Wrapper: Renderer+ec erhaelt ihn, +480 wird null, sein rohes Handle/FVF an +150 geht in den Soll-Vertexshader, und Dirty 4 wird gesetzt. Auch ein Nullhandle ist ein gueltiger roher Wert. Ein benoetigter fehlender Wrapper wird vor Writes als sicherer Portfehler abgewiesen.

Die statische Streamuebergabe hat denselben Restore am Ende, wenn der uebergebene Shaderkind null und die neue Streamanzahl positiv ist. Nichtnull-Kinds erhalten die vorhandene Shaderauswahl; ein leerer Streamsatz fordert keinen Shader an.

## 5. Vorbereitung bis zum Draw

`prepare` verbindet Stream- und Indexuebergabe. Beide werden auf Kopien berechnet und erst gemeinsam uebernommen; spaete Fehler erhalten Renderer und Deferred-Cache. Diese Transaktion ist eine sichere Portregel, keine Behauptung ueber native Fehlerbehandlung.

`submit` verbindet diese Vorbereitung mit dem bereits geprueften begrenzten Mehrpass-Ablauf. Es leitet `Draw::indexed` aus dem resultierenden Renderer-Indexwrapper ab und plant danach Passauswahl, Nebeloverride, kompletten Flush, Draw und vorgemerkte Nebelwiederherstellung. Der vom Aufrufer mitgelieferte Indexmodus wird hier durch den Rendererzustand ersetzt. Die gesamte Submission wird erst nach erfolgreicher Planung uebernommen, einschliesslich Passalias-Mutationen und Statistik.

Bei null Materialpaessen findet die Buffer-Vorbereitung statt, aber kein Draw/Flush; die Sollwrites bleiben vorgemerkt. Ein spaeter Draw-Validierungsfehler nimmt auch die Buffer-Vorbereitung zurueck. Der Plan setzt bereits aufgeloeste Ressourcen voraus: externe Uploads muessen vor der Ausfuehrung der Bindungs-/Draw-Aufrufe erfolgt sein. Er ist kein ausfuehrendes Grafikbackend.

## Nachweis

`Generate-D3DBuffers.py` prueft die SHA256 der vorherigen Draw-Fixtures. Streamfaelle kombinieren alle neuen/bisherigen Anzahlen 0..16 und beide Shaderzweige; zusaetzliche Fehlerfaelle pruefen Grenzen und fehlende Ressourcen. Indexproben enthalten Nullquelle, Nullgroesse, beide bisherigen Bindungszustaende, Revisionen, Nullhandles und rohe Basiswerte. Restoreproben enthalten signierte Extremwerte, fehlende Wrapper und Null-/Nichtnullhandles. 96 Sequenzen verbinden Buffer-Vorbereitung mit den bisherigen Pass-/Draw-Kontexten.

`Record-D3DBuffers.py` wiederholt die Originalbereiche fuer Restplaetze, Beschreibungswrites, Revisionsvergleiche, umlaufende Uploadsumme, Framewrites, Bindungspaare, Stridebytes und Shader-Restore sowie die vorhandenen Pass-/Flush-/Draw-Bereiche. Externe virtuelle Getter, Cachelookup, Upload und Shaderbeschaffung werden als explizite Rueckgabefixtures eingesetzt. Es werden keine Originalressourcen alloziert und keine nativen Uploadfunktionen ausgefuehrt. Profiling/Exceptionverwaltung bleiben ausgespart. Die komplette unterstuetzte Rendererstruktur und der Deferred-Cache werden verglichen, einschliesslich unberuehrter Felder.

```powershell
python scripts/Generate-D3DBuffers.py
cargo run -p rc-inspect --bin rc-d3d-buffers-check -- analysis/reports/d3d-buffers.input.json analysis/reports/d3d-buffers.json
cargo run --release -p rc-inspect --bin rc-d3d-buffers-check -- analysis/reports/d3d-buffers.input.json analysis/reports/d3d-buffers-release.json
cargo test --workspace *> analysis/reports/d3d-buffers-tests.log
python scripts/Record-D3DBuffers.py
```

Ergebnis: **578 gueltige Streamfaelle**, **124 Indexfaelle**, **18 Restorefaelle**, **96 integrierte Sequenzen** und **14 erwartete sichere Fehler**. **863927 Originalinstruktionen**, 1547 Stream-Uploadentscheidungen, 4624 Stream-Framewrites, 272 feste Stream-Restores; 385 integrierte Materialpaesse mit 206 Draw-Aufrufen und 48 indizierten Sequenzen geprueft. Debug/Release bytegleich. Zwoelf neue Tests ergeben **667 Workspace-Tests**; Clippy `-D warnings`, Format- und Diffpruefung bestanden.

Artefakte: `analysis/reports/d3d-buffers.input.json`, `d3d-buffers.json`, `d3d-buffers-release.json`, `d3d-buffers-validation.json`, `d3d-buffers-tests.log`; Evidenzschluessel `d3d_buffers_validation`. Vorherige Berichte bleiben historische Staende mit damaligen Quellhashes und Testzahlen.

Weiter offen: eigentliche Cache-/Bufferallokation und Datenuploads, dynamische Ringbufferpfade, Deklarations-/Shadererzeugung, Materialbeschaffung pro Meshabschnitt, Ressourcenlebensdauer, ausfuehrendes Backend und Live-Spiel. Die beschriebene statische Uebergabe wird nicht als vollstaendige Bufferimplementierung behauptet. Diffuse-Abdeckung bleibt 271/289; Android zum Schluss.
