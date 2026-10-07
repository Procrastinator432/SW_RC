# Feste native FName-Indizes

`FName.StaticInit` (`core.dll:101504f0`) legt feste NameEntry-Objekte an und registriert sie über `Hardcode` (`10150390`) unter ihrem gespeicherten Index. `AllocateNameEntry` (`1014f880`) speichert Index an +0, Flags an +4, HashNext an +8 und den Text an +0xc. FName selbst hält einen Eintragszeiger; der Index ist vom Handle und von der Stringadresse zu unterscheiden. FName(None) wird als null repräsentiert.

384 feste Registrierungen sind aus der lokalen DLL exportiert. `scripts/Verify-HardcodedNames.py` gleicht alle Namen und Indizes zwischen dekompilierten Registrierungspaaren, Assembleroperanden und originalen PE-Stringbytes ab. Die MOV-Immediates und relativen Callziele werden zusätzlich direkt in den PE-Instruktionsbytes geprüft. Der Script schreibt mit `--write` die generierte Rust-Tabelle `hardcoded_names.rs`; ohne Schalter prüft er deren unveränderten Inhalt. Die Tabelle enthält Lücken, ihr höchster fester Index ist 936.

Für NotifyHitWall ist jetzt die feste Registrierung bewiesen: Index 353 (`0x161`), Struct-Hashbucket 97 (`353 & 127`) und Ereignismaskenbit 53 (`353-300`, Maske `0x0020000000000000`). Der zuvor belegte Names-Tabellenslot war allein noch kein Nachweis des aufgelösten Namensindex. Die Registrierung schließt diese Lücke für diesen Namen.

`name_bindings::hardcoded_name` stellt die geprüften Indizes ohne Beachtung der ASCII-Groß-/Kleinschreibung bereit. Nichtnull-Handles werden durch eigene stabile opaque Tokens repräsentiert; sie sind keine nativen Speicheradressen. Die Diagnosebindung erhält feste Indizes und vergibt für unbekannte Namen eigene Indizes oberhalb von 936. Sie lehnt Nicht-ASCII und eingebettete NUL-Zeichen ab, statt die nicht vollständig rekonstruierte native Zeichenbehandlung zu erraten.

Die native dynamische Vergabe im FName-Konstruktor (`1014ff20`) hängt von vorhandenen Einträgen, Ladefolge und wiederverwendbaren Indizes aus Available ab. Die alphabetisch vergebenen Diagnoseindizes bilden diese Registrierung nicht nach. Deshalb bleiben vollständiges natives Hashlayout und dynamische Namensparität offen.

`rc-state-link-probe <GameData> <report.json> --native-hardcoded-names` bindet die festen Indizes an die Original-Kinderlisten. Die übrigen Namen bleiben ausdrücklich diagnostisch. Der neue Bericht ist bis auf Bindungsmetadaten und Scope mit dem früheren Bericht identisch: 3430 Klassen-/State-Ownerlisten und 51 Controller-Klassensuchen unverändert. Zusätzliche Proben mit gelieferten Masken bestätigen EmptyBase bei Bit53 und MaskedOut bei Maske null. Die Masken stammen weiterhin nicht von einer tatsächlichen Runtimeinstanz.

162 Workspace-Tests, Clippy mit Warnungen als Fehler und Formatprüfung bestanden. Neue Tests prüfen festen Notify-Index, oberes Maskenwort, null für None/Leerstring, ASCII-Namensidentität, diagnosegebundene unbekannte Namen und ungültige Zeicheneingaben. Nachweise: `analysis/reports/hardcoded-names-proof.json`, `native-named-state-link.json`, `native-names-validation.json`. `scripts/Record-NativeNames.py` vergleicht die Originalberichte und aktualisiert Evidence/Wiki. Aktiver Zustand, native Overrides, Scriptausführung und Androidprüfung bleiben offen; Androidprüfung weiterhin erst zum Schluss.

## Folgearbeit: initialisierte Ereignismasken

Die Originalprobe verwendet jetzt zusätzlich den belegten InitExecution-Maskenzustand und Disable für alle51Controller-Klassen. StateNode=Class und Maske=alleBits; nachDisable353 MaskedOut. Spätere GotoState-Maskenkomposition und Enable sind als separate Helfer ergänzt; tatsächliche Zustandswahl/Callbacks bleiben offen. 164Tests bestanden. Siehe [PROBE_FRAME.md](PROBE_FRAME.md).
