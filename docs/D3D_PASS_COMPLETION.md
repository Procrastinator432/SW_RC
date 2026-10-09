# Passabschluss und mehrstufige Hardware-Uebergabe

Drei Aufgaben verbinden die bisherigen einzelnen Zustandsuebersetzungen: unbenutzte Texturstufen abschalten, ColorWrite samt Passidentitaet abschliessen und einen begrenzten Hardwarepass ueber alle aktiven Stufen verarbeiten. Grundlage sind die bereits exportierten Originalanweisungen aus `D3DDrv!1001ed70`. Die Ausgabe bleibt ein numerischer Direct3D-8-Aufrufplan ohne GPU-Ausfuehrung.

## 1. Unbenutzte Stufen

`Cache::disable_unused` entspricht `1001f59f..1001f68e`. Die Schleife beginnt beim aktiven Stufenzaehlbyte Pass+9 und endet vor der Geraetekapazitaet +41e8. Beide Werte sind in der Rust-API auf 0..8 begrenzt; eine aktive Anzahl oberhalb der Kapazitaet ergibt wie beim Original eine leere Restschleife.

Jede Reststufe verliert ihren Ressourcenzeiger und die Solltexturbindung. Soll-Koordinatenindex wird der Stufenindex, Soll-Transformflags werden null. Im festen Funktionspfad wird zusaetzlich das gepackte Wort 2 mit `old & ffc21fff | 21000` angepasst; ColorOp und AlphaOp werden 1. Bei vorhandenem HardwareShader bleiben diese Operationsfelder und das gepackte Wort unveraendert. Andere Felder, die Matrixhistorie und Stufen ausserhalb der Schleife bleiben erhalten. Dirtybits 2/10 werden gesetzt. Es wird ausdruecklich kein pauschales Nullsetzen einer ganzen Stufe verwendet.

## 2. ColorWrite und Identitaet

`Cache::finish_pass` entspricht `1001f68e..1001f6ae`: Passwort +20 wird unveraendert in natives Sollwort 30 geschrieben (API-ID 168), Renderdirty gesetzt und Passadresse nach Rendererzustand+300 uebernommen. Auch unbekannte hohe ColorWrite-Bits bleiben roh erhalten.

Am Funktionsanfang `1001ed8e..1001edab` vergleicht das Original diese Adresse mit dem angeforderten Passzeiger. Gleiche Adresse ueberspringt den gesamten Uebergabepfad, auch wenn Speicherinhalt oder Geraeteargumente inzwischen anders sind. `apply_hardware_pass` bildet genau diese Reihenfolge ab: der Identitaetsvergleich findet vor der Eingabepruefung statt. Die Adresse ist ein Identitaetswort im Snapshot, kein von Rust dereferenzierter Zeiger. Der Aufrufer muss die originale Lebensdauer und Invalidierung selbst verwalten.

## 3. Zusammenhaengender Hardwarepass

`rc_package::d3d_pass::apply_hardware_pass` erhaelt Cache, letzte Passadresse, `Pass`, acht bereits aufgeloeste optionale Ressourcenpaare +38/+3c und `Device` mit Cullargument, LOD-Bias und Kapazitaet. `Pass` enthaelt 28 Praefixbytes, das separate rohe ColorWrite-Wort und acht gepackte 28-Wort-Stufen. Die nicht benoetigte Luecke Pass+1c wird nicht interpretiert.

Die API setzt voraus, dass das originale HardwareShader-Feld Pass+3a8 ungleich null ist. Dieser Originalzweig ueberspringt die externe Pixelshaderbeschaffung. Nach dem Identitaetstest folgen gemeinsamer Renderpraefix, alle aktiven nichtmatriziellen Stufen in Originalreihenfolge, Reststufen und Abschluss. Ein anschliessendes `Cache::flush` plant die Render-/Buehnen-/Texturmethoden. Die aktive Stufenzahl begrenzt die Uebersetzung; die Geraetekapazitaet begrenzt Reststufen und Ausgabeschleifen. Diese zwei Grenzen werden nicht zusammengezogen.

Nichtnullpassadresse, Anzahl/Kapazitaet bis acht, Matrixbit und vorhandene Ressourcenfelder werden vor den Writes geprueft. Bei ungueltigen Eingaben bleiben Pass, Cache und Identitaet unveraendert. Diese sichere Vorpruefung ist eine Rust-Grenze, keine Behauptung zum Fehlerverhalten nativer ungueltiger Zeiger oder uebergrosser Zaehler. Bei gleichem Passzeiger gilt zuvor weiterhin der originale Skip.

Die vorherige Materialprobe enthielt absichtlich Setup-Rueckgaben -1, 0, 3 und -2. Nur -1 gilt dort nativ als Fehler; -2 wird als Byte 254 serialisiert. Die bisherige Praefixpruefung konnte solche Bytes behandeln, da sie keine Stufenschleife ausfuehrte. Ein vollstaendiger begrenzter Pass weist 254 dagegen zurueck. Deshalb werden hier alle 64 erfolgreichen Materialpraefixe mit Stufenzahl 0 oder 3 uebernommen; die 32 kuenstlichen 254-Faelle werden nicht als gueltige Mehrstufenpaesse behauptet. Ein eigener Test belegt ihre Ablehnung ohne Mutation.

## Verifikation und Artefakte

`Generate-D3DPass.py` prueft den SHA256 des vorherigen Eingabeberichts. 162 systematische Kontexte bilden alle Kombinationen aus aktiven Stufen 0..8, Kapazitaet 0..8 und beiden Reststufenzweigen ab. Dazu kommen die 64 gueltigen Materialpraefixe, insgesamt 226. Initialcache, Ressourcenpaare, ColorWrite, Devicefelder und gepackte Stufendaten bleiben ausgewiesene Fixtures; dies erfasst keine Live-Szene.

`rc-d3d-pass-check` prueft Reststufenschleife und Hardwarepass getrennt, zeichnet mutierte Passdaten/Soll-/Ausgabecaches/geordnete Aufrufargumente auf und versucht anschliessend denselben Zeiger mit ungueltig geaenderter Anzahl und Kapazitaet erneut. `Record-D3DPass.py` fuehrt separat die exportierten x86-Anweisungen fuer Identitaetsvergleich, Praefix, jede aktive Stufe, Restschleife, Abschluss und die bekannten Ausgabeschleifen aus. Der Identitaetsbranch wird vor Profiling gestoppt; Zeitmessung und Exceptionprolog sind nicht Gegenstand der Uebersetzung. Die Byteadress-VM wird aus den bisherigen Pruefskripten nur bis vor deren Reportauswertung geladen. Native CPU-Cachewrites und numerische Methodenargumente werden verglichen; kein nativer GPU-Aufruf erfolgt.

```powershell
python scripts/Generate-D3DPass.py
cargo run -p rc-inspect --bin rc-d3d-pass-check -- analysis/reports/d3d-pass.input.json analysis/reports/d3d-pass.json
cargo run --release -p rc-inspect --bin rc-d3d-pass-check -- analysis/reports/d3d-pass.input.json analysis/reports/d3d-pass-release.json
cargo test --workspace *> analysis/reports/d3d-pass-tests.log
python scripts/Record-D3DPass.py
```

Ergebnis: **226 Kontexte**, **552547 Originalanweisungen** (181932 Reststufen/Abschluss/Ausgabe, 370615 Hardwarepipeline/Identitaet/Ausgabe), **23491 geordnete Methodenaufrufe**, **101248 gepackte Stufenwoerter**, **687 aktive Stufenuebersetzungen**. 15 anfaengliche und 226 wiederholte Identitaets-Skips bestaetigt; 64 vorherige gueltige Materialpraefixe enthalten. Debug/Release bytegleich. Sieben neue Tests, **615 Workspace-Tests**; Clippy `-D warnings`, Format- und Diffpruefung bestanden.

Berichte: `analysis/reports/d3d-pass.input.json`, `d3d-pass.json`, `d3d-pass-release.json`, `d3d-pass-validation.json`, `d3d-pass-tests.log`; Evidenzschluessel `d3d_pass_validation`. Die frueheren Berichte bleiben historische Pruefstaende mit ihren damaligen Quellhashes und Testzahlen.

Offen bleiben Matrixstufen, feste Pixelshaderauswahl und Sonderkonstantenupload, Ressourcenbeschaffung/-lebensdauer, Backend, Live-Szenen und spielbare Engine. Die Pipeline bildet den begrenzten Hardwarezweig ab, nicht die ganze Funktion fuer jedes Material. Android-Pruefung weiterhin zum Schluss; Diffuse-Abdeckung unveraendert 271/289.


Fortsetzung `D3D_TRANSFORMS.md`: Eine zusaetzliche API uebernimmt jetzt rohe Matrixstufen, elf Transformslots und native Transformmasken im geordneten Render-/Buehnen-/Transform-/Texturplan. Der bisherige nichtmatrizielle API-Vertrag bleibt erhalten. Feste Shaderauswahl, Ressourcen-/Transformquellen und Backend bleiben offen.
