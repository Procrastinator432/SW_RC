# Statische Buffer-Lebenszyklen

Stand: 2026-10-09. Fuenf Aufgaben: Vertexbuffer-Flags/Wiederverwendung, Vertex-Allokationsversuche, Vertex-Lock/Fuellen/Unlock, Indexbuffer-Groesse/Format/Wiederverwendung und revisionsabhaengiger Indexupload. Implementierung `crates/rc-package/src/d3d_upload.rs`; Diagnose `rc-d3d-upload-check`.

Referenzen: D3DDrv 1002a380 und 1002c270, exportiert als `analysis/decompiled/d3d-static-upload.asm` und `d3d-static-index-upload.asm`. Die zugehoerigen C-Befunde stehen in `d3d-buffer-setup.c`. Die nativen Wrapper speichern Handle an +30, Revision an +10 sowie Kapazitaet beim Vertexbuffer an +38 und beim Indexbuffer an +34.

## 1. Vertexbuffer-Flags und Wiederverwendung

Der Vertexpfad speichert zuerst den Quellzeiger an Wrapper+3c. Die Quelle liefert die rohe Groesse ueber virtuell +14. Usage beginnt mit 8; ein Nichtnullwert von Quelle+18 waehlt 208 und Lockflags 2000. Quelle+1c fordert zusaetzlich Usagebit 100 an, wenn Device+4118 ungleich null ist. Device+40dc gleich null fuegt Usagebit 10 hinzu. Alle Bedingungen verwenden rohe Null-/Nichtnullwerte.

Ein vorhandenes Handle mit exakt gleicher Kapazitaet wird wiederverwendet. Geaenderte Usageanforderungen bewirken hier keine Neuallokation. Bei anderer Groesse wird ein vorhandenes Handle per Release (+8) freigegeben. Der rohe Groessenwert bleibt erhalten, auch null oder gesetztes hoechstes Bit; die CPU-Diagnose alloziert dafuer keinen entsprechenden Hostspeicher.

## 2. Vertex-Allokation und Fallback

CreateVertexBuffer liegt an Geraete-VTable +5c und erhaelt `[Groesse, Usage, 0, Pool, Wrapperadresse+30]`. Der Schleifenindex beginnt bei null und bleibt kleiner drei. Pool ist bei Index zwei der Wert 2, sonst 0. Nichtnegative HRESULT-Werte gelten als Erfolg, einschliesslich positiver Werte.

Nach einem fehlgeschlagenen Versuch gilt:

- Device+4120 ungleich null: Index um zwei erhoehen; keine Eviction. Es entstehen Versuche mit Poolwerten 0 und 2.
- Device+4120 gleich null: Geraetemethode +14 mit Argument null ausfuehren und Index um eins erhoehen. Es entstehen Versuche mit Poolwerten 0, 0 und 2.

Die Antworten auf Create/Eviction sind explizite `Responses`. Der erfolgreiche Create schreibt das gelieferte Handle an die native Ausgabestelle. Die API verlangt dabei ein Nichtnullhandle. Fehlende Antworten, erschoepfte Versuche und Fehler im Evictionpfad werden sicher abgewiesen. Native Fehlerformatierung, GError-Logging und eine moegliche Fortsetzung nach solchen Meldungen werden nicht emuliert.

## 3. Vertex-Lock, Fuellen und Abschluss

Auch beim wiederverwendeten Buffer folgt immer Lock (+2c) mit `[0, Groesse, Lock-Ausgabestelle, Lockflags]`. Danach wird der Quellcallback +24 mit dem gelieferten Lockzeiger geplant, anschliessend Buffer-Unlock (+30). Am Ende speichert das Original die Groesse an +38 und den abschliessend gelesenen Quellrevisionswert an +10.

`Command` erhaelt Empfaenger, VTable-Offset und geordnete Argumente. Das Modell fuehrt keine COM-Methode und keinen Quellcallback aus. Es behauptet deshalb weder kopierte Nutzdaten noch einen gelungenen realen GPU-Upload. Die Lock-Ausgabestelle und der zurueckgegebene Zielzeiger sind ausdrueckliche Diagnoseeingaben. Die abschliessende Revision ist ebenfalls ein eigener Quellwert.

## 4. Indexbuffer-Groesse, Format und Wiederverwendung

Die Indexgroesse aus Quelle+10 wird mit einem signierten Vergleich auf mindestens zwei angehoben. Werte mit gesetztem hoechstem Bit ergeben deshalb ebenfalls zwei. Bei vorhandenen Handles entscheidet nur diese resultierende Groesse ueber Wiederverwendung; Indexbreite und Usage spielen dabei keine Rolle. Auf dem Wiederverwendungspfad fragt das Original die Breite nicht ab. `Plan::usage` und `format` bleiben dort `None`.

Bei Neuallokation wird ein vorhandenes Handle freigegeben. Quelle+18 gleich vier waehlt Formatwert 66, jeder andere Wert 65. Usage ist bei Device+40dc ungleich null der Wert 8, sonst 18. CreateIndexBuffer (+60) erhaelt `[resultierende Groesse, Usage, Format, 1, Wrapperadresse+30]`. Dieser Pfad hat keinen Vertex-artigen Retry-/Evictionzyklus.

## 5. Revisionsabhaengiger Indexupload

Nach eventueller Neuallokation vergleicht das Original die gecachte Revision mit einem frisch gelesenen Quellwert. Nur bei Abweichung folgen Lock (+2c) mit `[0, resultierende Groesse, Ausgabestelle, 0]`, Quellcallback +14 mit dem Lockzeiger und Unlock (+30). Bei gleicher Revision entfallen alle drei Aufrufe, selbst nach einer Neuallokation. Diese seltene Kombination bleibt ausdruecklich erhalten.

Unabhaengig vom Uploadgate schreibt der Abschluss die resultierende Groesse an +34 und eine nochmals gelesene Quellrevision an +10. `IndexSource` hat deshalb getrennte `revision_before` und `revision_after`; eine Aenderung zwischen beiden wird nicht weggekuerzt. Das Vertex-spezifische Quellzeigerfeld +3c bleibt beim Indexpfad unveraendert.

Beide APIs planen auf einer Kopie der Ressourcenstruktur und uebernehmen sie nur bei erfolgreicher Planung. Sichere Fehler erhalten die Eingabestruktur. Diese Transaktion ist eine Portregel; sie beschreibt weder native Teilwrites nach Fehlern noch einen Rollback bereits ausgefuehrter GPU-Aufrufe. Die ausgegebenen Befehle duerfen erst nach erfolgreicher Planung ausgefuehrt werden, sofern spaeter ein Backend angeschlossen wird.

## Nachweis und Artefakte

`Generate-D3DUpload.py` verknuepft den vorherigen Buffer-Fixturestand per SHA256. Die neuen Proben kombinieren alle 32 Vertexflag-/Geraetegates, drei Wiederverwendungszustaende und drei Retryantwortmuster. Indexproben kombinieren acht rohe Groessen, fuenf Breiten, zwei Geraetegates, drei Wiederverwendungszustaende und beide Revisionsvergleiche. 32 Sequenzen mit je vier Schritten pruefen fortlaufende Handle-/Kapazitaets-/Revisionsaenderungen. Zusaetzliche Fehlereingaben pruefen sichere Abweisung.

`Record-D3DUpload.py` interpretiert die Originalbereiche fuer Flags, Groessenvergleiche, Reuse/Release, Schleifensteuerung, Argument-Pushes, HRESULT-Gates und Abschlusswrites. An COM- und Gettergrenzen werden explizite Antworten eingesetzt; die tatsaechlich gepushten Empfaenger/Argumente werden unabhaengig aufgezeichnet. SETZ DL wird mit der geprueften Originalinstruktion und dem vorherigen nativen Vergleich ausgefuehrt. Native Prologe, Exceptionverwaltung und Fehlerlogging bleiben ausgespart. Die sicheren Portfehler werden auf erhaltenen Eingabestand geprueft, nicht als native Fehlergleichheit bezeichnet.

```powershell
python scripts/Generate-D3DUpload.py
cargo run -p rc-inspect --bin rc-d3d-upload-check -- analysis/reports/d3d-upload.input.json analysis/reports/d3d-upload.json
cargo run --release -p rc-inspect --bin rc-d3d-upload-check -- analysis/reports/d3d-upload.input.json analysis/reports/d3d-upload-release.json
cargo test --workspace *> analysis/reports/d3d-upload-tests.log
python scripts/Record-D3DUpload.py
```

Ergebnis: **288 Vertexfaelle**, **480 Indexfaelle**, **32 Sequenzen / 128 weitere Schritte**, **14 sichere Fehler**. **58364 Originalinstruktionen** und **3198 geordnete Aufrufe** bestaetigt. Die Zaehler einschliesslich Sequenzschritten enthalten 460 Vertex-Createversuche, davon 124 mit Pool 2, 123 Evictions, 99 Vertex- und 207 Index-Wiederverwendungen sowie 296 tatsaechlich geplante Index-Fuellfolgen. Debug/Release bytegleich; zehn neue Tests ergeben **677 Workspace-Tests**. Clippy `-D warnings`, Format- und Diffpruefung bestanden.

Berichte: `analysis/reports/d3d-upload.input.json`, `d3d-upload.json`, `d3d-upload-release.json`, `d3d-upload-validation.json`, `d3d-upload-tests.log`; Evidenzschluessel `d3d_upload_validation`. Vorherige Nachweise behalten ihre historischen Testzahlen und Quellhashes.

Weiter offen: echte Quellcallback-Nutzdaten, Backend-Ausfuehrung von Create/Release/Lock/Unlock und Datentransfer, Ressourcenlookup und Lebensdauer, Verbindung dieses Lebenszyklus mit der gesamten Material-/Draw-Szene, dynamische Ringbuffer, Shadererzeugung und Live-Spiel. Die neue Aufrufplanung ist kein Nachweis real ausgefuehrter GPU-Uploads. Diffuse-Abdeckung bleibt 271/289; Android wie vereinbart zum Schluss.
