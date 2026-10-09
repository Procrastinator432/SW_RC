# Erfasste statische Quelldaten bis zum Draw-Plan

Stand: 2026-10-09. Implementierung: `crates/rc-package/src/d3d_static.rs` und Upload-Abbildung in `d3d_resource.rs`; Diagnose: `rc-d3d-static-check`.

## 1. Wrapper und Uploadfelder verbinden

`Cache::upload_resource` liest den nativen Handle an +30, Revision an +10 und die typabhaengige Kapazitaet (Vertex +38, Index +34). Besitzer und konkrete Wrapper-VTable werden geprueft. `store_upload` uebernimmt nur die vom jeweiligen Upload geschriebenen Felder und den Frame an +14. Vertex +3c erhaelt die Quelle; Index +3c bleibt erhalten. Links, Schluessel, Padding, zweiter Vertexhandle und sonstige Bytes bleiben unveraendert.

Der Cache gehoert zum UD3DRenderDevice. Das fuer Create/Lock verwendete COM-Geraet ist ein anderer Zeiger und wird weiterhin ausdruecklich angegeben.

## 2. Vertexquellen aufloesen

Der 64-Bit-Schluessel wird direkt aus Quelle+4/+8 des Snapshots gelesen. Ein Treffer verwendet den vorhandenen Wrapper; nur bei einem Fehlschlag wird das bereitgestellte Allokationsabbild als Vertexwrapper initialisiert und in beide Listen eingefuegt. Die externe Revisionsantwort wird gegen die aktuelle Wrapperrevision verglichen.

Bei geaenderter Revision werden Skin-Metadaten erfasst, Groesse bestimmt, der statische Upload geplant und Quelldaten in den bereitgestellten CPU-Lockspiegel kopiert. Der neue Handle, Kapazitaet, Revision und Quellzeiger werden im Cache gespeichert. Bei gleicher Revision entfallen Skin-Metadaten, Nutzdaten, Lockziel und Uploadantworten. Die Komponentenbeschreibung behaelt die zehn vom nativen Getter ungeschriebenen Bytes; Stride bleibt 32.

Slots werden in Reihenfolge verarbeitet. Zwei Slots mit demselben Schluessel sehen denselben Wrapper; der zweite Slot sieht bereits die vom ersten Upload geschriebene Revision. Eine unbenutzte Allokationsantwort wird bei einem Treffer ignoriert. Zwei Quellen duerfen denselben CPU-Lockspiegel nacheinander fuellen.

## 3. Indexquellen aufloesen

Nullquelle und Nullgroesse werden vor Ressourcenlookup und Allokation behandelt. Bei Nichtnullgroesse wird ein vorhandener Indexwrapper gesucht oder ein bereitgestelltes Indexabbild initialisiert. Die aeussere Revision entscheidet, ob der Indexupload aufgerufen wird. Dessen eigene Revisionsabfrage bleibt getrennt: Sie kann die Datenfuellung ueberspringen, obwohl der aeussere Vergleich einen Uploadaufruf verlangt hat.

Bei diesem inneren Treffer werden weder Quelldaten noch Lockziel benoetigt; eine erforderliche Buffererzeugung und abschliessende Revisionsschreibung bleiben erhalten. Der Rueckgabewert der Bindefunktion zaehlt weiterhin die rohe Quellgroesse beim aeusseren Unterschied, nicht die tatsaechlich kopierte Bytezahl. 16- und 32-Bit-Indizes bleiben unterscheidbar; Wiederverwendung folgt der nativen Kapazitaet und ignoriert einen Breitenwechsel.

Ein neuer Wrapper hat Revision null. Liefert die Quelle ebenfalls null, wird schon der aeussere Upload uebersprungen und der Handle bleibt null. Das ist das rekonstruierte Originalverhalten. Indexmodus wird spaeter aus dem vorhandenen Wrapper abgeleitet, auch bei Nullhandle.

## 4. Statische Bindung vorbereiten

`Resources::resolve_sources` liefert aus Cache und Quelldaten die bisher extern vorgegebenen Stream-/Indexwerte. `prepare` verbindet dies mit Komponentenfeldern, bedingtem Stream-Restplatzloeschen, Frameverwendung, Basisvertex und festem Vertexshader-Restore. Die Uploadentscheidung der Bindung verwendet die Revision vor dem Upload, waehrend der Cache schon die abschliessende Revision enthaelt.

CPU-Zielpuffer werden anhand der ausdruecklichen Lockadresse aufgeloest. Maximal 32 Spiegel mit jeweils 16 MiB und insgesamt 64 MiB sind zugelassen. Zielrestbytes bleiben erhalten. Diese Zielobjekte sind benannte CPU-Abbilder, keine allgemeine Simulation ueberlappender Prozessadressen.

## 5. Bis zum Mehrpass-Draw verbinden

`submit` reicht die aufgeloesten Werte an die bereits gepruefte Buffer-/Pass-/Draw-Planung weiter. Ressourcenbilder, CPU-Ziele, Rendererzustand, Deferred-Cache, Passmutation und Zaehler werden gemeinsam erst nach Erfolg uebernommen. Ein Fehler im zweiten Upload, fehlender fixer Shader oder eine ungueltige Passliste erhaelt alle Eingaben.

Diese Transaktion ist die sichere Port-Semantik. Geplante Create/Release/Lock/Unlock- und Draw-Aufrufe werden nicht tatsaechlich ausgefuehrt. Ihre externen Antworten, Allokationsbilder, Besitzer-Skinningbytes und Shaderhandles bleiben Eingaben. Die Uploadbefehle liegen in Streamreihenfolge, danach beim Index, gefolgt vom Submissionplan vor.

## Pruefung

Die Referenz sind D3DDrv GetCachedResource (10015040), Wrapperkonstruktoren (1002a140/1002a320/1002a7d0), Vertex-/Indexuploads (1002a380/1002c270), Stream-/Indexbindung (10020220/10020840), Pass-/Drawfunktionen und die bereits exportierten Originalgetter/Kopien aus Engine.dll. Es wurde kein neues Live-Speicherabbild gewonnen.

`Record-D3DStatic.py` komponiert die originalen Instruktionskoerper: Cache und Konstruktoren laufen im Listenmodell; Uploadkoerper in ihren ABI-Fixturerahmen. Deren vollstaendige 64-Byte-Wrapperbilder werden in das Listenmodell zurueckgetragen. Komponentengetter, Groessen und Kopien laufen mit den Engine-Instruktionen; Bindung und Draw mit den resultierenden Werten in einem Rendererrahmen. Die Referenzberechnung verwendet keinen Rust-Aufruf. Dieser Nachweis ist kein ununterbrochener originaler Binaer-Callstack; externe Callbackantworten und die Verbindungen zwischen den ABI-Rahmen sind ausdruecklich modelliert.

24 Sequenzen mit 96 vollstaendigen Submission-Schritten vergleichen Cachekoepfe, saemtliche Wrapperbytes, CPU-Zielpuffer samt Restbytes, aufgeloeste Werte, Uploadplaene, Bindung, Passmutation, Drawplaene und Zaehler. Enthalten sind kollidierende 64-Bit-Schluessel, wiederholte Frames, Wrapperaliasing, geteilte Lockziele, gespeicherte/delegierte Skindaten, Bufferresize/-reuse, Retry/Eviction, Nullgroessen, Revisionshits und Revision null.

Ergebnis: **440280 interpretierte Originalinstruktionen**, **68 Cachefehlschlaege / 172 Treffer**, **5696 kopierte Bytes**, **288 Draw-Passschritte**. 96 Vertex-Uploadaufrufe und 84 aeussere Skips; 40 Index-Uploadaufrufe mit zehn inneren Skips und 20 aeusseren Skips. 24 Vertex- und zehn Index-Reuses. Zehn Fehlerfaelle pruefen den vollstaendigen Rollback ohne Ausfuehrung nativer Fehlerpfade. 15 neue Unit-Tests ergeben **716 Workspace-Tests**. Debug/Release bytegleich; Clippy `-D warnings`, Format- und Diffpruefung bestanden.

```powershell
python scripts/Generate-D3DStatic.py
cargo run -p rc-inspect --bin rc-d3d-static-check -- analysis/reports/d3d-static.input.json analysis/reports/d3d-static.json
cargo run --release -p rc-inspect --bin rc-d3d-static-check -- analysis/reports/d3d-static.input.json analysis/reports/d3d-static-release.json
cargo test --workspace *> analysis/reports/d3d-static-tests.log
python scripts/Record-D3DStatic.py
```

Artefakte: `analysis/reports/d3d-static.input.json`, `d3d-static.json`, `d3d-static-release.json`, `d3d-static-validation.json`, `d3d-static-tests.log`; Evidenzschluessel `d3d_static_validation`. Der Recorder schreibt den kompakten Nachweis; er wird beim Abschluss dem bestehenden Evidenzledger hinzugefuegt. Historische Nachweise behalten ihre damaligen Quellhashes und Testzahlen.

Offen: echte GPU-/Allocatorausfuehrung, dynamische Buffer, Ausfuehrung des Besitzer-Skinningcallbacks, Shadererzeugung, reale Szenenbeschaffung aus Spielpaketen und Live-Spiel. Die Verbindung dieses Blocks gilt fuer erfasste statische Quellen und explizit beantwortete externe Aufrufe. Diffuse-Abdeckung bleibt 271/289. Android-Pruefung wie vereinbart zum Schluss.
