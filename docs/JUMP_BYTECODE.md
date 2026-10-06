# Bytecode-Abgleich der Sprungregeln

Der neue begrenzte Rust-Leser `rc-package::script::read_function` liest die drei Sprungfunktionen direkt aus Engine.u. Bedingungen, Geschwindigkeitszuweisungen, Aufrufreihenfolge und Sprungziele stimmen mit den gespeicherten Script-Texten überein. Der reproduzierbare Vergleich prüft sämtliche20Anweisungen von Pawn.DoJump, beide Anweisungen von Pawn.CannotJumpNow und fünf Anweisungen von PlayerController.Jump.

| Funktion | logische Bytecode-Bytes | serialisierte Bytecode-Bytes |
|---|---:|---:|
| Pawn.DoJump | 299 | 234 |
| Pawn.CannotJumpNow | 4 | 4 |
| PlayerController.Jump | 44 | 34 |

Objekt- und Namensverweise benötigen im logischen Bytecode vierBytes, werden im Paket aber als CompactIndex gespeichert. Sprungziele beziehen sich auf logische Offsets. Der Leser speichert beide Offsetarten und verlangt für absolute Jumps ein Ziel an einer obersten Anweisungsgrenze. Kurze boolesche Auswertungen enthalten Skip-Längen einschließlich EndFunctionParms; Context enthält die Länge des Ergebnis-Ausdrucks. Beide werden für den unterstützten Bereich geprüft.

## Nachgewiesene Struktur

- Eintritt nur ohne bIsCrouched/bWantsToCrouch und mit Physics Walking1, Ladder11 oder Spider9.
- Spider setzt den ganzen Vektor auf JumpZ*Floor; Ladder setzt Z0; Walking wählt DefaultVariable JumpZ bei bIsWalking und InstanceVariable JumpZ sonst.
- Base!=None und !Base.bWorldGeometry führt zur nachfolgenden Z-Addition von Base.Velocity.
- PlayOwnedCue mit Argument24, PlayJumpSoundOnCurrentMaterial, SetPhysics mit Argument2 und anschließende Rückgabe true; abgelehnte Eintrittsbedingung springt zur Rückgabe false. Der Authority4-Zweig enthält keine ausführbare Aktion.
- CannotJumpNow gibt false zurück. Die normale PlayerController.Jump-Funktion ruft bei Level.Pauser==PlayerReplicationInfo SetPause(false) auf und setzt sonst bPressedJump=true.

Operatornummern stammen zusätzlich aus den serialisierten Core.u-UFunctions: !129, &&130, ||132, !=Object119, ==Object114, ==Int154, +=Float184 und Float*Vector213. Actor.SetPhysics besitzt in Engine.u NativeIndex3970. Die Enum-Zahlen werden mit den gespeicherten EPhysics-, ENetRole- und EPawnAudioEvent-Definitionen verglichen. Der BoolVariable-Handler in core.dll bestätigt, dass der folgende Variablen-Ausdruck ausgewertet wird; DefaultVariable greift auf Klassendefaults zu.

## Werkzeuge und Nachweise

Die Serialisierung ist aus UStruct::Serialize10123630, SerializeExpr10120d00 und UFunction::Serialize10124310 abgeleitet. Vorhandene `class-serializers.c`, neuer `function-serializer.c` und `jump-bytecode-handlers.c` liegen unter `analysis/decompiled`. Der Leser unterstützt ausdrücklich nur SWRC-Version159/Licensee1, Functions ohne StateFrame und ohne eigene TaggedProperties. FriendlyName kann bei Operatoren ein Symbol statt des Exportnamens sein. Unbekannte Opcodes, Überläufe, unvollständige Ausdrücke und überschüssige Payload-Bytes sind Fehler. Dies ist ein begrenzter Leser, keine vollständige Script-Dekompilierung oder VM.

Beispielbefehle aus dem Projektverzeichnis:

```powershell
cargo run -p rc-inspect --bin rc-script-dump -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\Engine.u' analysis/reports/jump-bytecode.json Pawn.DoJump Pawn.CannotJumpNow PlayerController.Jump
```

Zusätzlich werden die acht genannten Core.u-Operatoren und Actor.SetPhysics mit demselben Werkzeug gelesen; Berichte: `jump-native-operators.json` und `jump-setphysics.json`. `scripts/Validate-JumpBytecode.py` vergleicht die vollständigen geprüften Ausdrucksbäume und Anweisungspositionen, prüft NativeIndices sowie Enum-Werte und schreibt `analysis/reports/jump-bytecode-validation.json` einschließlich Paket-/Funktionspayload-SHA256.

104 Workspace-Tests, Clippy über alle Targets und Formatprüfung bestehen. Vier neue Tests decken kompakte Referenzen/logische Sprungziele, sämtliche Trunkierungen eines Funktionspayloads, unbekannte Tokens, zusätzliche Bytes, Operator-FriendlyName, Bool-Ausdrucksbaum, native erweiterte Nummer, Skip-Länge und Rekursionstiefe ab. Keine Änderung an Bewegungsrechnung oder Diagnoseabläufen in diesem Schritt; die zuvor geprüften Originalkartentrajektorien wurden deshalb nicht erneut ausgeführt.

## Verbleibende Grenzen

Der Abgleich bestätigt die statische Struktur dieser drei Funktionen und die Zuordnung der benutzten nativen Aufrufe. Er bestätigt keine Ausführung der vollständigen Script-VM, keine native Operator-/Sound-/SetPhysics-Laufzeitparität, keine bewegten Bases oder vollständige Controllerzustände. PlayerWalking.ProcessMove/PlayerMove und weitere Klassen-/State-Overrides sind noch nicht umfassend gegen Bytecode geprüft. Die eigene Eingabeflanke und Moduswahl bleiben als Diagnosepolitik bestehen. Android-Prüfung wie gewünscht zum Schluss; kein APK-Build oder Emulatorlauf.

## Folgearbeit: Controller-Sprung-/Duck-Abschnitt

ProcessMove und ShouldCrouch sind inzwischen vollständig, der Sprung-Schluss von PlayerMove ab Offset774 ist gesondert gegen den Bytecode geprüft. Ein separater Rust-Helfer übernimmt diese Sprung-/Duck-Reihenfolge für den Authority-Pfad; die vollständige Zustandsmaschine und Kollisionskörperanpassung bleiben offen. Details: [WALKING_INPUT.md](WALKING_INPUT.md).
