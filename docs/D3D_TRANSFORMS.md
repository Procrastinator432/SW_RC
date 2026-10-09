# Matrixstufen und Transform-Aufrufplaene

Die drei Aufgaben dieses Stands erweitern die bisher nichtmatrizielle Uebergabe: rohe Texturstufenmatrizen uebernehmen, native Transformgruppen in den Ausgabefluss einordnen und den gesamten begrenzten Hardwarepass mit gemischten Matrix-/Nichtmatrixstufen verarbeiten. Grundlage sind die exportierten Originalfunktionen `D3DDrv!1001ed70` und `10028f10`. Die Rust-Implementierung befindet sich in `rc_package::d3d_transforms`.

## 1. Matrixzweig einer Texturstufe

`Transforms::stage` verarbeitet eine Stufe 0..7. Die gemeinsame Ressourcen-, Adress-, LOD-, Operations-, Koordinaten- und Bump-Uebersetzung bleibt dieselbe wie bisher. Der lokale Aufruf der gemeinsamen Uebersetzung entfernt Matrixbit 40 nur in einer Arbeitskopie; das gepackte Originalwort bleibt erhalten. Lediglich die bereits belegte LOD-Sentinel-Ersetzung darf Stufenwort 1 veraendern.

Wenn Stufenwort 4 Bit 40 gesetzt hat, uebernimmt die Funktion die niedrigen neun Bits von Wort 5 in Soll-Transformflags (natives Buehnenwort 10). Die sechzehn Woerter 6..21 werden unveraendert nach Cache+324+Stufe*40 kopiert. Das entspricht `1001f42e..1001f4bc` einschliesslich `MOVSD.REP`. Transformmaske Cache+624 erhaelt das Stufenbit; Dirtybit 40 wird gesetzt. Die nativen Vergleiche fuer Index 10 liegen ausserhalb des auf acht Stufen begrenzten Eingabemodells.

Ohne Matrixbit werden die Soll-Transformflags null; vorhandene Matrixdaten und vorherige Maskenbits bleiben erhalten. Es erfolgt keine Matrixmultiplikation, Transposition, NaN-Normalisierung oder Identitaetsersetzung. Matrix- und Bump-Woerter sind getrennt: Bump bleibt in Stufenwoertern 22..27.

Nichtnullressourcen benoetigen wie bisher explizite rohe Ressourcenwoerter +38/+3c. Fehlende Ressourcen oder Indices ab acht werden ohne Writes abgewiesen. Ein Nullressourcenzeiger hindert den nativen Matrixzweig nicht an der Uebernahme.

## 2. Transformgruppen im Ausgabefluss

`Transforms` speichert elf rohe 4x4-Matrizen und das volle 32-Bit-Maskenwort. `Transforms::flush` plant die unterstuetzten Dirtygruppen 1/2/10/20/40. Die originale Reihenfolge ist Render-/Buehnenzustaende, Transformmethoden, Texturbindungen. `Plan` legt diese Reihenfolge explizit als `before`, `transforms`, `after` ab.

Die Transformmethoden stammen aus `100298a4..10029a7a`; alle verwenden VTable-Offset 94. `TransformCall` enthaelt die numerische Transform-ID und eine Kopie der sechzehn Woerter, auf die der originale Methodenparameter zeigt. Diese Woerter sind ein Zeigerpayload, keine sechzehn separaten API-Argumente.

| Matrixslot / Maskenbit | Numerische Transform-ID | Benoetigtes Dirtybit |
| --- | --- | --- |
| 0..7 | 16..23 | 40 |
| 8 | 2 | 40 |
| 9 | 3 | 40 |
| 10 | 256 | 20 |

Ein Aufruf benoetigt sowohl Gruppenbit als auch Maskenbit. Die Geraetekapazitaet begrenzt diese Matrixausgabe nicht; sie begrenzt weiterhin Buehnen- und Texturzustandsausgaben. Auch bei Kapazitaet null koennen daher Matrixaufrufe entstehen. Die native Implementierung vergleicht Matrizen nicht mit einem Ausgabecache: eine erneut markierte identische Matrix wird erneut ausgegeben.

Am Flushende werden das gesamte Maskenwort (10029efd) und Dirtywort (10029f75) null, auch fuer Maskenbits, deren Gruppe nicht freigegeben war oder die oberhalb der elf unterstuetzten Slots liegen. Ohne erneute Markierung ist die naechste Ausgabe leer. Der Planner uebernimmt die bekannte optimistische Zustandscachepolitik; eine spaetere Backend-Anbindung muss Aufrufausfuehrung und Wiederherstellung selbst behandeln. Keine Grafik-API wird aufgerufen.

## 3. Gemeinsamer Hardwarepass mit Matrixstufen

`d3d_transforms::apply_hardware_pass` erweitert die bisherige Hardwarepipeline um `Transforms`. Sie behaelt die originale Passidentitaets-Abkuerzung vor allen Pruefungen und die Reihenfolge Praefix, aktive Stufen, unbenutzte Stufen, ColorWrite/Identitaet. Bis zu acht aktive Stufen duerfen jetzt beliebig zwischen beiden Matrixzweigen wechseln. Vorab fehlende aktive Ressourcen oder ungueltige Passadresse/Anzahl/Kapazitaet lassen Cache, Matrizen, Pass und Identitaet unveraendert.

Die Voraussetzung bleibt Pass+3a8 ungleich null. Feste Pixelshaderauswahl und deren Sonderkonstantenupload sind weiterhin ausgeschlossen. Die bisherige `d3d_pass::apply_hardware_pass` bleibt als ausdruecklich nichtmatrizielle API erhalten; ihre frueheren Ablehnungsregeln aendern sich nicht. Slots 8..10 duerfen als rohe Hostfixtures fuer die Transformausgabe bereitgestellt werden; deren echte Szenenbeschaffung wird hier nicht rekonstruiert.

## Unabhaengige Pruefung

`Generate-D3DTransforms.py` uebernimmt den SHA256-geprueften vorherigen Passbericht als Fixturequelle. Die 482 Hardwarepaesse umfassen alle 256 Matrixbitkombinationen bei acht aktiven Stufen sowie alle 226 vorherigen Passkontexte. Darunter bleiben 64 gueltige Materialpraefixe; die frueheren kuenstlichen Zaehler 254 bleiben ausgeschlossen. Ressourcen, Anfangscache, Matrixhistorie und Geraetefelder sind ausgewiesene Fixtures. Matrixpayloads enthalten rohe NaNs, Unendlichkeiten, signierte Null und unterschiedliche Einzelwoerter.

Weitere 2092 Ausgabefaelle umfassen alle 2048 Masken fuer elf Slots bei beiden Gruppenbits und jedes einzelne Slotbit bei allen vier Gruppengates 0/20/40/60. Ein hohes unbekanntes Maskenbit prueft dessen Loeschung. Kapazitaeten 0..8, vor-/nachgelagerte Render-/Texturaufrufe und leere Wiederholungen sind enthalten.

`Record-D3DTransforms.py` wiederholt separat die exportierten x86-Anweisungen: Identitaetsvergleich, Renderpraefix, alle aktiven Stufen einschliesslich REP-Kopie, Reststufen, Abschluss und Ausgabeschleifen. Die bereits vorhandene Byteadress-VM wurde um AH-Lesen, MOVSD.REP mit ABI-seitig geloeschtem Richtungsflag und Aufzeichnung des Matrixzeigerpayloads beim Offset-94-Aufruf erweitert. Die Originalspeicherwrites, kompletten unterstuetzten Soll-/Ausgabecaches, alle elf Matrizen/Masken, mutierten Stufen und geordneten Methodenplaene werden mit Rust verglichen. Profiling und Exceptionverwaltung sind ausgeschlossen. Der nativen Methode wird weiterhin ein ignorierter Fehler-HRESULT zurueckgegeben.

```powershell
python scripts/Generate-D3DTransforms.py
cargo run -p rc-inspect --bin rc-d3d-transforms-check -- analysis/reports/d3d-transforms.input.json analysis/reports/d3d-transforms.json
cargo run --release -p rc-inspect --bin rc-d3d-transforms-check -- analysis/reports/d3d-transforms.input.json analysis/reports/d3d-transforms-release.json
cargo test --workspace *> analysis/reports/d3d-transforms-tests.log
python scripts/Record-D3DTransforms.py
```

Ergebnis: **482 Hardwarepaesse**, **2092 Ausgabefaelle**, **1599207 Originalanweisungen**, **1360 Matrixkopien**, **2735 aktive Stufenuebersetzungen**, **13732 Transformaufrufe** und **40663 Render-/Buehnen-/Texturaufrufe** bestaetigt. Alle 256 Stufenmuster, 2048 Transformmasken und vier Gruppengates abgedeckt; 64 vorherige gueltige Materialpraefixe und 15 anfaengliche Identitaets-Skips enthalten. Debug/Release bytegleich. Sieben neue Tests, **622 Workspace-Tests**; Clippy `-D warnings`, Format- und Diffpruefung bestanden.

Artefakte: `analysis/reports/d3d-transforms.input.json`, `d3d-transforms.json`, `d3d-transforms-release.json`, `d3d-transforms-validation.json`, `d3d-transforms-tests.log`; Evidenzschluessel `d3d_transforms_validation`. Aeltere Berichte bleiben historische Pruefstaende mit ihren damaligen Quellhashes und Testzahlen.

Weiter offen: feste Pixelshaderauswahl/Sonderupload, Shader-/Vertex-/Index-/Streamgruppen, echte Ressourcen und Transformquellen, Backend und Live-Spiel. Der Stand plant Aufrufe und bestaetigt keinen GPU- oder Bildvergleich. Diffuse-Abdeckung bleibt 271/289; Android-Pruefung zum Schluss.


Fortsetzung `D3D_BINDINGS.md`: Der gemeinsame Ausgabepfad ergaenzt Shader-/Stream-/Indexcache und verarbeitet jetzt Gruppen 1/2/4/8/10/20/40 in Originalreihenfolge. Die Lichtgruppe 80 bleibt ausgeschlossen; Shader-/Bufferbeschaffung und Backend bleiben offen.


Fortsetzung/Korrektur `D3D_LIGHTS_AND_PASSES.md`: Lichtgruppe 80 und aufgeloeste feste Pixelshaderwahl/Sonderupload sind jetzt in einer zusaetzlichen gemeinsamen API vorhanden. Der vollstaendige native Flush-Eintritt wurde neu geprueft: Bei Dirty null kehrt er ohne Loeschen der Transformmaske zurueck. Die bisherigen Teilproben fuehrten direkt den Gruppenrumpf aus; dessen Loeschregel gilt nur nach einem von null verschiedenen urspruenglichen Dirtywort. Die oeffentlichen Transform-/Bindingplaner sind korrigiert und durch drei Regressionstests abgesichert. Historische Berichte behalten ihre damaligen Quellhashes und engeren Pruefbereiche. Ressourcen-/Containerbeschaffung und Backend bleiben offen.
