# PlayerWalking: Sprung- und Duck-Reihenfolge

Der neue Rust-Helfer `walking_input::walking_input_phase` bildet den Sprung-/Duck-Abschnitt von Engine.PlayerController.PlayerWalking für den lokal ausgeführten Authority-Pfad ab. Er nimmt bereits gesetzte Script-Eingabeflags und Pawn-Snapshots entgegen. Er ist separat vom geometrischen PC-Bewegungspfad; eine vollständige PlayerController- oder Script-VM-Ausführung ist noch nicht vorhanden.

## Ablauf aus dem Bytecode

1. PlayerMove setzt bDoubleJump=false. Bei bPressedJump, vorhandenem Pawn und CannotJumpNow=true wird bSaveJump=true und bPressedJump vor der Verarbeitung gelöscht; sonst bSaveJump=false.
2. ProcessMove kehrt ohne Pawn sofort zurück. Vor der Sprungverarbeitung werden OldAccel gelesen und eine abweichende NewAccel zu Pawn.Acceleration geschrieben. Diese Beschleunigungsverarbeitung liegt außerhalb des neuen Sprung-/Duck-Helfers.
3. Bei gesetztem bPressedJump wird Pawn.DoJump aufgerufen. Der Helfer nutzt dafür die zuvor abgeglichene JumpZ-/Physics-/Duck-Rechnung und wechselt bei Erfolg zu Falling.
4. Erst anschließend wird bei Physics!=Falling die Duck-Anforderung verarbeitet: bDuck==0 ruft ShouldCrouch(false) auf; andernfalls ruft bCanCrouch=true ShouldCrouch(true) auf. Ohne Erlaubnis bleibt die alte Anforderung bestehen. ShouldCrouch schreibt lediglich bWantsToCrouch.
5. PlayerMove setzt bPressedJump abschließend auf bSaveJump. Ein gesperrter DoJump-Aufruf allein speichert die Eingabe deshalb nicht.

Wird aus dem Stehen gleichzeitig Sprung und Ducken angefordert, entscheidet zuerst DoJump über den bisherigen Pawn-Zustand. Bei erfolgreichem Sprung wird die neue Duck-Anforderung wegen Falling nicht angewendet. Umgekehrt sperrt ein bereits gesetztes bWantsToCrouch den Sprung noch in dem Schritt, der anschließend durch bDuck==0 die Anforderung löscht. Ein neuer Sprung-Event ist danach erforderlich, sofern CannotJumpNow ihn nicht vorher gespeichert hat.

`pressed_jump` bezeichnet einen Script-Event, keinen gehaltenen physischen Knopf. Der Aufrufer muss Eingabebindungen und Wiederholungen selbst korrekt abbilden. Die bisherige PC-Diagnose mit eigener Eingabeflanke/Latch verwendet weiter ihren bisherigen Pfad. Base-Pawn.CannotJumpNow gibt false zurück; der True-Testfall prüft den vorhandenen Controllerzweig mit einem ausdrücklich gelieferten Rückgabesnapshot, keine im Originalspiel beobachtete Sperre.

## Statische Nachweise

| Funktion | logische Scriptbytes | serialisierte Scriptbytes | geprüfter Abschnitt |
|---|---:|---:|---|
| PlayerWalking.ProcessMove | 222 | 170 | alle15Anweisungen |
| Pawn.ShouldCrouch | 15 | 11 | beideAnweisungen |
| PlayerWalking.PlayerMove | 953 | 705 | zwölfAnweisungen ab logischem Offset774 |

PlayerMove wird vollständig strukturell gelesen; sein früherer Kamera-/Beschleunigungsabschnitt ist hier nicht vollständig gegen den Quelltext bewertet. Der überprüfte Schluss enthält die Sprungfreigabe, die Auswahl ReplicateMove/ProcessMove anhand Role<Authority4 und die abschließende Flag-Zuweisung. Die Netzwerkseite wird nicht umgesetzt. Vier zusätzliche Operatornummern wurden aus Core.u-Funktionsmetadaten bestätigt: !=Vector218, !=Int155, <Int150 und Rotator-Subtraktion317.

Der begrenzte Bytecode-Leser unterstützt nun auch Vector- und Rotator-Konstanten mit jeweils12Datenbytes. Vectorwerte müssen für die Diagnose endlich sein; Rotatorwerte bleiben signierte32Bit-Zahlen. Unbekannte Opcodes werden weiter ausdrücklich abgewiesen.

Nachweise: `analysis/reports/player-walking-bytecode.json`, `player-walking-native-operators.json` und `walking-input-bytecode-validation.json`. `scripts/Validate-WalkingInputBytecode.py` vergleicht die29geprüften Anweisungen einschließlich Ausdrucksbäumen und Sprungzielen und speichert Paket-/Payload-SHA256. Die vorherigen DoJump-Nachweise bleiben unter [JUMP_BYTECODE.md](JUMP_BYTECODE.md) dokumentiert.

## Tests und Grenzen

112 Workspace-Tests, Clippy über alle Targets und Formatprüfung bestehen. Sechs neue Sequenztests prüfen gleichzeitigen Sprung/Ducken, bisherige Duck-Anforderung und anschließendes Loslassen, gespeicherten Event mit späterem Versuch, fehlenden Pawn/Flug, Duck-Erlaubnis und Nichtteilrückgabe bei ungültiger Velocity. Zwei neue Parser-Tests prüfen Vector-/Rotator-Werte und Breiten sowie nichtendliche Vectorwerte und Größenüberlauf.

Der Helfer passt keine Kollisionskörpermaße an, führt keine Sounds oder SetPhysics-Callbacks aus und übernimmt keine Beschleunigung, Kamera, Netzwerkreplikation oder dynamische Bases. Weitere Controllerstates und Overrides sind offen. Geometrische Diagnoseabläufe bleiben unverändert; es wurde kein neuer Originalkarten-Bewegungslauf durchgeführt. Android-Prüfung wie gewünscht zum Schluss; kein APK-Build oder Emulatorlauf.

## Folgearbeit: statischer Bewegungsadapter

Die Script-Phase ist inzwischen über einen eigenen Event-/Flagzustand mit dem bestehenden PC-Bewegungsadapter verbunden. Die frühere eigene Eingabeflanke bleibt als separater Diagnosepfad erhalten. Aktive oder nach der Script-Phase weiter angeforderte Körpergrößenänderung ist noch unsupported und liefert keinen Teilzustand. Details: [SCRIPT_MOTION.md](SCRIPT_MOTION.md).
