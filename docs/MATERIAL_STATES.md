# Material-, Sampler- und Snapshot-Zustandsuebergabe

Drei weitere Aufgaben rekonstruieren den uebergeordneten HardwareShader-Aufruf, die rohen Sampler-Zustandswrites und einen sicheren Leser fuer deren originale x86-Speicherfelder. Die neue Anbindung kann freigegebene Materialuebergaben und ausgewaehlte Ressourcen-/Adress-/Filterwoerter nachbilden. Eine Grafik-API wird noch nicht aufgerufen.

## 1. Materialfreigabe, Rueckfall und gepackter Renderpass

`rc-package::hardware_material::set` folgt `D3DDrv!1000fff0`. Die Geraetewerte `+4218` (VS) und `+4220` (PS) werden in dieser Reihenfolge auf ungleich null geprueft. Nicht vorhandene Unterstuetzung ersetzt einen optionalen Meldungstext und schreibt bei vorhandenem Ausgabezeiger die Rueckfallreferenz `shader+28`; der Setup-Callback wird nicht aufgerufen. Auch `ffffffff` gilt bei den originalen unsigned Vergleichen als vorhanden.

Sind beide Faehigkeiten vorhanden, wird vor dem externen Aufruf `1000dbe0` Rendererflag `+9fd0 |= 20` gesetzt. Rueckgabewert -1 haengt exakt zwei Leerzeichen und `Failed to set hardware shader.` an einen optionalen bisherigen Text an, schreibt die optionale Rueckfallreferenz und beendet die Uebergabe ohne eigene Passwrites. Bereits erfolgte Callback-/Rendererwrites bleiben erhalten. Bei Erfolg bleiben Text und Rueckfallausgabe unberuehrt.

Erfolg uebernimmt ausschliesslich folgende Felder in den ersten Pass aus `state+304`:

| Quelle | Ziel |
| --- | --- |
| Bits 2..4 aus `shader+92c` | Bits 0..2 von Passwort `+4` |
| Bits 0..1 aus `shader+92c` | Bits 3..4 desselben Passworts |
| Byte `shader+930` | Passbyte `+8` (Alpha-Referenz) |
| Byte `shader+931` | Nullerweitertes Passwort `+14` (Source-Blend) |
| Byte `shader+932` | Nullerweitertes Passwort `+18` (Destination-Blend) |
| Niedrigstes Byte des Setup-Ergebnisses | Passbyte `+9` |
| Konstante 1 | Byte `state+324` |

Andere Bits und Bytes bleiben erhalten. Die Passstruktur ist groesser; das Modell besitzt nur ihren benoetigten 28-Byte-Praefix. Es interpretiert die einzelnen Bool-/Blend-Enums noch nicht als konkrete GPU-Einstellungen. Auch negative Setupwerte ausser -1 werden wie im Original ueber AL verengt; das normale Texture-Setup liefert 0..8. Zusaetzliche sichere Callbackfehler bleiben Rust-Fehler mit erhaltenen vorherigen Writes, ohne eine native Fehlerbehandlung fuer ungueltige Speicherzugriffe zu behaupten.

## 2. Ressourcen-/Adress-/Filterwoerter

`rc-package::hardware_sampler::bind` folgt `100044b0` ab dem Ergebnis der externen Ressourcenbeschaffung `1001a420`. Nullmaterial loescht allein Buehnenwort 0. Fuer Nichtnullmaterial wird die uebergebene Ressourcenadresse dorthin geschrieben; Bit 0 aus dem rohen Materialbyte `+7c` setzt Rendererflag 1. Die Assembly bestaetigt diesen Byteoffset ausdruecklich.

Bei Nichtnullressource und deren Nichtnullwort `+3c` erhalten beide niedrigen Nibbles des Buehnenworts 2 das niedrige Nibble von Geraetewort `+466c`. Andernfalls bestimmt Materialbyte `+65` das erste und Byte `+66` das zweite Nibble: Byte 0 schreibt 1, Byte 1 schreibt 3; alle anderen Bytewerte erhalten die bisherigen Bits. Das dritte Nibble erhaelt immer das niedrige Nibble von `+466c`. Uebrige Bits, Rendererflags und Buehnenwoerter bleiben erhalten.

Damit ist der zuvor komplett externe rohe Adress-/Filterteil separat umgesetzt. Die Ressourcenbeschaffung/-lebensdauer, Texture-Modifier und spaetere Umsetzung der gepackten Woerter durch Direct3D 8 bleiben offen. Begriffe wie Nearest/Bilinear werden keinem dieser Werte ohne weiteren Befund zugewiesen. Die bisherigen CPU-Meshbilder behalten ihre explizite Nearest/Repeat-Sampler-Fixture.

## 3. Sicherer Snapshot-Leser und unabhaengige Gegenpruefung

`rc-package::hardware_state_snapshot::{material,sampler}` liest dieselben Felder aus `shader_snapshot::Memory`, einschliesslich Renderer->Geraet und Renderer->State->Pass. Es akzeptiert nur vorhandene, nicht ueberlaufende Bereiche; fehlende Felder liefern einen Fehler ohne Zustandsschreibvorgang. Materialerfassung ist vollstaendig/eager: auch ein nativer frueher Faehigkeitsabbruch benoetigt fuer diese Capture-API einen vollstaendigen stabilen Snapshot. Samplererfassung liest fuer Nullmaterial weder Renderer/Geraet noch die angegebene Ressourcenadresse. Die Ressource ist ein ausdruecklicher Hostwert aus dem externen Cache-Lookup, kein aus dem Material erfundener Zeiger.

`Generate-MaterialState.py` erzeugt synthetische, versetzte Sparse-Abbilder: 512 Materialkontexte (alle 32 niedrigen Flagmuster, alle vier VS-/PS-Faehigkeitskombinationen und Setupwerte -1/0/3/-2), 3072 nichtleere Samplerkontexte (alle 256 U-Bytes, V-Bytes 0/1/2/255 und drei Ressourcenpfade), 16 Nullmaterialien sowie sechs defekte Material- und zwei defekte Samplerabbilder. Meldungs-/Rueckfallzeiger sind optional; ein ausdruecklicher Setup-Stub veraendert in ausgewaehlten Proben den Host, um deren Erhaltung zu pruefen. Initiale Pass-/Buehnenwoerter und Geraetewerte sind Fixtures.

`rc-material-state-check` erfasst diese Abbilder ueber die neue API und fuehrt die Rust-Zustandsuebergaenge aus. `Record-MaterialState.py` benutzt separat ein Byteadress-Woerterbuch und einen begrenzten x86-Anweisungsinterpreter fuer die exportierte Originalroutine. Bei Materialfaellen laeuft die gesamte `1000fff0`-Anweisungsfolge mit denselben ausdruecklichen Setup-Stub-Effekten. Meldungstexte werden aus dem Original-PE gelesen. Bei Samplern beginnt die Wiederholung nach dem externen Cache-Lookup, fuer Nullmaterial am Funktionseingang. Es wird keine neu geschriebene Rust-Formel als Referenz ausgefuehrt.

Ergebnis: **16016 originale Materialanweisungen / 16896 Host-Ausgabebytes**, **119016 originale Sampleranweisungen / 86464 Buehnenwoerter**, acht erwartete Erfassungsfehler. Die erfassten Rohfelder, Callbackanzahl, Status, Meldungstexte, Rueckfallwerte, Pass-/Buehnenwoerter und Rendererflags stimmen ueberein. Debug-/Release-Berichte bytegleich. Sechs neue Tests; **602 Workspace-Tests**, Clippy `-D warnings`, Format und Diffpruefung bestanden.

Artefakte: `analysis/decompiled/hardware-material-state.{c,asm}`, `hardware-state-constructors.{c,asm}`, `analysis/reports/material-state{.input,,-release,-validation}.json`, `material-state-tests.log`. Der exportierte Engine-Konstruktor bestaetigt auch die bisher verwendeten Materialzaehler VS=-2 / PS=-3; die Byteflagzuordnung zu Unreal-Propertynamen wird daraus nicht abgeleitet.

```powershell
python scripts/Generate-MaterialState.py
cargo run -p rc-inspect --bin rc-material-state-check -- analysis/reports/material-state.input.json analysis/reports/material-state.json
cargo run --release -p rc-inspect --bin rc-material-state-check -- analysis/reports/material-state.input.json analysis/reports/material-state-release.json
cargo test --workspace *> analysis/reports/material-state-tests.log
python scripts/Record-MaterialState.py
```

Naechste offene Verbindung: Verbraucher der gepackten Pass-/Buehnenzustaende in der Direct3D-8-Ausgabe, belegte Zuordnung von Unreal-Property-/Klassenvorgaben zu den nativen Flagbits und echte Ressourcen-/Wrapperanbindung. Neue Zustandsproben ersetzen keine Bild- oder Live-Spielpruefung. Allgemeine Diffuse-Abdeckung bleibt 271/289; Live-Szene und spielbare Engine offen. Android-Pruefung zum Schluss.

Weiterfuehrung/Korrektur 2026-10-09: [Direct3D-Zustandsuebergabe](D3D_STATE_HANDOFF.md) rekonstruiert nun Renderpraefix, nichtmatrizielle Buehnenuebersetzung und geordnete Cache-Aufrufplaene. Das bisherige rohe Label `filter` fuer Geraetewort +466c betrifft hier Adressierungswerte (IDs 13/14/25); echte Min-/Mag-/Mip-Filter bleiben offen. Rohwertberichte unveraendert, keine Grafik-API aufgerufen.


Fortsetzung `D3D_PASS_COMPLETION.md`: -2 ist im SetMaterial-Callback-Test absichtlich kein nativer Fehlerwert und wird zu Byte 254. Das ist keine gueltige maximale Acht-Stufen-Uebergabe; die neue begrenzte Hardwarepipeline weist solche Zaehler vor Mutation ab. Alle 64 erfolgreichen Kontexte mit Stufenzahl 0 oder 3 werden dort als Praefixinputs weitergeprueft.
