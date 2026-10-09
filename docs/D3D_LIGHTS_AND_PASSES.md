# Lichtgruppe, Pixelshaderwahl und beide Passzweige

Dieser Stand bearbeitet fuenf Aufgaben: rohe Lichtpayloads, deren separaten Aktivierungscache, die gemeinsame Ausgabe aller niedrigen Dirtybits, aufgeloeste feste Pixelshaderwahl mit Sonderkonstanten und die integrierte Passuebergabe fuer Hardware- und Festfunktionszweig. `rc_package::d3d_complete` ergaenzt die bisherigen Zustandsplaner. Grundlage bleiben `D3DDrv!1001ed70` und `10028f10`; es erfolgt keine GPU-Ausfuehrung.

## 1. Rohe Lichtdaten

`Lights::words` speichert acht native 26-Wort-Strukturen von Cache+6d8 mit Abstand 68. Bei Dirtybit 80 und einem von null verschiedenen Aktivierungswort plant `Lights::tail` Offset b0 mit Slotindex und einer Kopie der 26 Woerter des originalen Zeigerpayloads. Floatfelder, Typwoerter, NaNs und signierte Null bleiben roh. Lichttypen oder physikalische Parameter werden nicht neu interpretiert.

Das Original vergleicht diese Payloads nicht mit einer vorherigen Ausgabe. Auch unveraenderte Daten werden bei erneut markierter Lichtgruppe wieder ausgegeben, sofern das Licht aktiv ist. Bei Aktivierungswort null entfaellt der Payloadaufruf. Dies entspricht `10029cb8..10029d05` und wird getrennt von der Aktivierung geprueft.

## 2. Aktivierungscache

`Lights::enabled` entspricht Cache+a18, `applied_enabled` Cache+144c. Nach einem gegebenenfalls geplanten Datenaufruf vergleicht die native Schleife das Aktivierungswort und aktualisiert bei Abweichung den optimistischen Ausgabecache vor Offset b8 mit `(Slot, Rohwert)`. Nichtnullwerte werden nicht auf eins normalisiert. Ein deaktiviertes Licht kann somit einen Aktivierungsaufruf mit null erzeugen, ohne Datenaufruf.

Die Kapazitaet stammt aus Geraet+41f4. Die Schleife verwendet eine signierte Grenze, maximal acht; negative Werte ergeben keinen Durchlauf. Sie ist unabhaengig von Textur- und Streamkapazitaet. Nicht durchlaufene Slots behalten ihre Ausgabehistorie. Daten- und Aktivierungsaufrufe bleiben pro Slot in der Reihenfolge b0, dann gegebenenfalls b8; Slots steigen aufwaerts.

## 3. Gemeinsame Ausgabe und korrigierter Eintrittstest

`Complete` fasst `Deferred` und `Lights` zusammen. `Complete::flush` unterstuetzt alle Dirtybits 1/2/4/8/10/20/40/80. Die Reihenfolge im ausgegebenen `Plan` ist:

`states.before -> states.transforms -> states.after -> lights -> bindings`

Dies entspricht Render-/Buehnenzustaenden, Matrizen, Texturen, Lichtern und dem Vertexshader-/Stream-/Pixelshader-/Indexabschnitt. Payloadstrukturen enthalten Kopien der Zeigerdaten; skalare `Call::arguments` bleiben echte Methodenargumente ohne Geraetezeiger. HRESULTs werden wie in den bisherigen Planern nicht als bestaetigte GPU-Ausfuehrung interpretiert; das Original ignoriert sie und behaelt seine Cachewrites.

Der vollstaendige native Eintritt `10028f2c..10028f36` stellt eine zusaetzliche Bedingung: **Bei Dirtywort null erfolgt sofortiger Ruecksprung, und die Transformmaske bleibt unveraendert.** Die vorherigen Gegenproben starteten direkt im Gruppenrumpf und konnten diesen Eintrittsfall nicht belegen. Die neue Gegenprobe fuehrt deshalb auch diese Originalanweisungen aus. Die bisherigen oeffentlichen Transform-/Bindingplaner wurden entsprechend korrigiert.

Bei jedem von null verschiedenen unterstuetzten Dirtywort wird der Rumpf dagegen ausgefuehrt und am Ende die gesamte Transformmaske geloescht, auch wenn ausschliesslich Shader-, Buffer- oder Lichtbits gesetzt waren. Interne `flush_body`-Methoden erhalten diese Unterscheidung, wenn ein aeusserer Planer bereits die urspruengliche Nichtnullmaske geprueft und danach Gruppenbits ausgeblendet hat. Drei neue Regressionstests pruefen Null-Dirty mit bestehender Matrixmaske und Nichtnull-Dirty mit einer anderen Gruppe. Ein ausdruecklicher Null-Dirty-Originalfall verwendet Maskenwort deadbeef.

Dirtybits oberhalb ff und Texturstufenkapazitaeten oberhalb acht werden vor Mutation abgewiesen. Die unabhaengigen signierten Stream-/Lichtkapazitaeten werden nativ begrenzt. Dieser sichere Eingabevertrag bleibt enger als beliebige ungueltige native Speicherzustaende.

## 4. Aufgeloeste Pixelshaderwahl und Sonderupload

`select_pixel` rekonstruiert die Zustandswrites von `1001ee08..1001efb7`. `ShaderChoice::hardware` bildet Pass+3a8 ab: bei Nichtnullhardwarezweig bleiben Pixelshader-Sollwort und Dirtybit unveraendert, und es gibt keinen Sonderupload.

Im festen Zweig ergibt Passwort 0 gleich null ein Pixelshader-Sollwort null und Dirtybit 4, ohne Ressourcenlookup. Jeder Nichtnullkind benoetigt den bereits aufgeloesten DWORD-Handle aus `GetPixelShader`-Ergebnis+4; auch ein aufgeloester Handle null wird roh uebernommen. Fehlende Aufloesung wird vor Writes als Fehler gemeldet. Lookup, Shaderkompilierung und Ressourcenlebensdauer bleiben externe Aufgaben.

Nur Kind 1 plant zusaetzlich den unmittelbaren Offset-16c-Aufruf: Startregister 0, Anzahl 3, Payload drei Planes `(1,0,0,0)`, `(0,1,0,0)`, `(0,0,1,0)`. Die DLL-Konstante an 1006f9a0 wird direkt im Original-PE als 3f800000 verifiziert. Dieser Aufruf steht vor der Passzustandsuebersetzung und vor dem spaeteren Pixelshader-Binding; erneute Auswahl von Kind 1 plant ihn erneut.

Die native statische Containerallokation `10001650` wird nicht nachgebildet. Die Gegenprobe setzt deren erledigtes Allokationsbit und prueft sowohl den noch nicht initialisierten Konstantenspeicher (10084700 null, originale XORPS/MOVSS-Initialisierung wird ausgefuehrt) als auch den bereits initialisierten Speicher mit den kanonischen Planes. Nichtnullshaderzweige werden nach dem externen Lookup bei `1001ee2d` mit explizitem Ergebniszeiger fortgesetzt. Der Originalupload `1001ef42..1001ef5d` wird ausgefuehrt und sein Zeigerpayload aufgezeichnet; kein GPU-Aufruf erfolgt.

## 5. Integrierte Passuebergabe

`apply_pass` verarbeitet beide Zweige mit `Pass`, `Device`, aufgeloesten acht Ressourcenpaaren und `ShaderChoice`. Der originale Vergleich der Passadresse bleibt zuerst; gleiche Adresse ueberspringt Auswahl, Upload und Zustandsschreibungen sogar bei inzwischen ungueltigem Inhalt. Neue Paesse werden vor Writes auf Adresse, Stufenzahl/Kapazitaet, Ressourcen und benoetigten festen Shaderhandle geprueft.

Die Reihenfolge ist Auswahl/Sonderuploadplan, gemeinsamer Renderpraefix, alle aktiven Matrix-/Nichtmatrixstufen, unbenutzte Stufen und ColorWrite/Identitaet. Im festen Zweig werden die bislang ausgesparten festen Operations-/Argumentfelder gesetzt; im Hardwarezweig bleiben sie wie bisher erhalten. Matrixworte, LOD-Sentinel, Ressourcenhandlewahl und Reststufenunterschiede behalten ihre belegten Regeln. `PassPlan::immediate` muss vor einem anschliessenden `Complete::flush`-Plan ausgefuehrt werden. Aufrufplanung bleibt von tatsaechlicher Backend-Ausfuehrung getrennt.

## Pruefung und Artefakte

`Generate-D3DComplete.py` prueft SHA256 der vorherigen Binding- und Pass-Eingaben. Die neuen Fixtures enthalten 308 Lichtfaelle mit allen 256 Aktivierungsmustern, 64 Shaderauswahlen mit vier Kinds, Null-/Nichtnull-/fehlenden Handles sowie beiden Initialisierungszustaenden, 482 Passuebergaben mit Matrix-/Nichtmatrixstufen und 256 gesonderte Ausgaben fuer alle Dirtymasken 0..ff. 64 gueltige Materialpraefixe bleiben enthalten; kuenstliche Stufenzaehler 254 bleiben ausgeschlossen. Die ersten 256 synthetischen Passfaelle variieren die Shaderkinds ausdruecklich, die danach uebernommenen Materialpraefixe bleiben unveraendert.

`Record-D3DComplete.py` wiederholt die Originalanweisungen fuer Eintritt, Auswahl/Konstanteninitialisierung, beide Stufenzweige, Reststufen, Abschluss und alle niedrigen Ausgabegruppen. Die Byteadress-VM wurde um die benoetigten skalaren XMM-Transfers, XORPS-Nullsetzung, NOP und Payloadaufzeichnung fuer b0/b8/16c erweitert. Verglichen werden komplette unterstuetzte Soll-/Ausgabefelder, Matrizen/Masken, Lichtstrukturen/Aktivierungen, mutierte Passdaten und geordnete Methodenargumente. Sechs fehlende Shaderaufloesungen sind erwartete sichere Rust-Fehler, keine ausgefuehrten nativen Nullzeigerzugriffe. Profiling, Statistik und Exceptionverwaltung bleiben ausgeschlossen.

```powershell
python scripts/Generate-D3DComplete.py
cargo run -p rc-inspect --bin rc-d3d-complete-check -- analysis/reports/d3d-complete.input.json analysis/reports/d3d-complete.json
cargo run --release -p rc-inspect --bin rc-d3d-complete-check -- analysis/reports/d3d-complete.input.json analysis/reports/d3d-complete-release.json
cargo test --workspace *> analysis/reports/d3d-complete-tests.log
python scripts/Record-D3DComplete.py
```

Ergebnis: **308 Lichtfaelle**, **64 Shaderauswahlen** (sechs erwartete fehlende Handlefehler), **482 Passuebergaben**, **256 Ausgabefaelle**, **2652316 Originalanweisungen** und **103385 geplante Methodenaufrufe** bestaetigt. Enthalten sind alle 256 Lichtaktivierungsmuster/Dirtymasken, 2735 aktive Stufenuebersetzungen, 64 vorherige gueltige Materialpraefixe, 38 unmittelbare Sonderuploads und 740 Null-Dirty-Eintritts-Skips. Debug/Release bytegleich. Zehn neue Funktions- und drei Regressionstests, **642 Workspace-Tests**; Clippy `-D warnings`, Format- und Diffpruefung bestanden.

Artefakte: `analysis/reports/d3d-complete.input.json`, `d3d-complete.json`, `d3d-complete-release.json`, `d3d-complete-validation.json`, `d3d-complete-tests.log`; Evidenzschluessel `d3d_complete_validation`. Fruehere Berichte bleiben historische Pruefstaende mit ihren damaligen Quellhashes, Testzahlen und ausgeschlossenen Funktionseinstiegen.

Weiter offen: echte Shader-/Buffer-/Texturressourcen und deren Lebensdauer, statische Container-/Shaderbeschaffung, native Szenenlicht-/Transformquellen, Draw-/Backend-Anbindung und spielbare Engine. Vollstaendige niedrige Zustandsgruppen bedeuten keine vollstaendige Engine oder GPU-/Bildgleichheit. Diffuse-Abdeckung bleibt 271/289; Android-Pruefung zum Schluss.
