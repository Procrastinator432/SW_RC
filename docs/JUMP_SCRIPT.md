# Sprungregeln aus dem gespeicherten Script

`rc-package::pawn_jump::jump_velocity` übernimmt Bedingungen und Geschwindigkeitszuweisung aus dem extrahierten `Engine.Pawn.DoJump` (Pawn.uc:1978–2003). Grundlage ist der gespeicherte TextBuffer; der anschließende statische Bytecode-Abgleich von DoJump, CannotJumpNow und der normalen Jump-Eingabefunktion ist inzwischen erfolgreich. Der Original-Laufzeitvergleich bleibt offen. Details: [JUMP_BYTECODE.md](JUMP_BYTECODE.md). Quellenbereiche und SHA256 stehen in `analysis/reports/jump-script-source.json`.

Die Klasse PlayerCommando erbt über CloneCommando, Republic und CTPawn von Engine.Pawn. In den extrahierten Quellen gibt es keine weiteren bool-Definitionen von DoJump oder CannotJumpNow. Engine.Pawn.CannotJumpNow gibt false zurück. Das allein beweist keine vollständige Runtime-Freigabe in allen Controllerzuständen.

| Bedingung/Zweig | Übernommene Regel |
|---|---|
| bIsCrouched oder bWantsToCrouch | Sprung abgelehnt |
| Physics außerhalb Walking/Ladder/Spider | Sprung abgelehnt |
| Walking, bIsWalking=false | XY unverändert, Z=current JumpZ |
| Walking, bIsWalking=true | XY unverändert, Z=Default.JumpZ |
| Ladder | XY unverändert, Z=0 |
| Spider | Gesamte Velocity=current JumpZ * Floor |
| Base vorhanden und kein WorldGeometry | Anschließend Base.Velocity.Z zu Z addieren |

Der Kommentar an Pawn.bIsWalking behauptet „can't jump“. Der Funktionskörper erlaubt den Sprung ausdrücklich. Die Implementierung folgt dem Funktionskörper. bCanJump und HealthLevel werden in dieser Funktion nicht geprüft; daraus folgt keine Aussage über andere Spielzustände. bUpdating ist hier ohne Wirkung. Nichtendliche Ergebniswerte liefern einen Fehler statt eines Bewegungszustands.

## Einbindung in die PC-Diagnose

Der gemeinsame Bewegungspfad nutzt den Walking-Zweig dieser Regeln. Die gemeinsame Diagnose akzeptiert weiterhin nur positive, endliche JumpZ-Werte bis1.000.000; dies ist eine Diagnosegrenze, keine Script-Bedingung. Der Aufrufer liefert jetzt current_jump_z und wants_to_crouch; das MovementProfile liefert den aufgelösten Klassenstandard von JumpZ. Die Originalkarten-Proben setzen beide Werte auf475, wants_to_crouch=false und keinen bewegten Base-Actor. Der isolierte Helfer bildet Leiter, Spider und die Geschwindigkeitsaddition einer Base ab; passende Körperbewegung und dynamische Kollision sind noch nicht verbunden.

Bodenkontakt bestimmt weiterhin nach eigener Diagnosepolitik den unterstützten Physics-Zweig. ViewInput.walking bildet den bereitgestellten bIsWalking-Snapshot ab; automatisches SetWalking ist nicht rekonstruiert. Die eigene Eingabeflanke samt Latch bleibt bestehen. Auch ein durch wants_to_crouch abgelehnter Tastendruck wird darin verbraucht; Loslassen und erneutes Drücken sind erforderlich.

Im gespeicherten PlayerController.Jump wird bPressedJump gesetzt oder die Pause aufgehoben. PlayerWalking.ProcessMove ruft bei gesetztem Flag DoJump auf, bevor es die Crouch-Anforderung aktualisiert. PlayerMove speichert das Flag bei CannotJumpNow und setzt es sonst nach dem Schritt zurück. Diese Controller-Reihenfolge, Pausen-/Netzwerklogik und vollständige Zustandsmaschine sind dokumentiert, aber nicht durch den Diagnose-Latch ersetzt oder als rekonstruiert behauptet. Aktives Hocken bleibt im gemeinsamen Pfad wegen fehlender Körpergrößenanpassung ein expliziter Fehler.

## Prüfung

100 Workspace-Tests und Clippy über alle Targets bestehen. Sechs neue Tests prüfen Standard-/Runtime-JumpZ im Geh- und Laufmodus, Duck-Sperren, ungültige Physics, Leiter-/Spider-Reihenfolge, Base-Z-Addition, nichtendliche Werte und die Einbindung mit anschließender Falling-Integration. Karte-/Regressionsnachweise stehen in `analysis/collision/jump-script-validation.json`.

Sounds, Script VM, SetPhysics-Ereignisse, Original-Landed/NotifyJumpApex, Leiter-/Spider-Bewegung, bewegte Untergründe und f32-Laufzeitparität bleiben offen. Android-Prüfung auf Nutzerwunsch zum Schluss; kein APK-Build oder Emulatorlauf in diesem Schritt.

Originalkarten-Regressionslauf: geo_01a PlayerStart0 mit280Ticks, dm_hangar PlayerStart8 mit228 und PlayerStart44 mit1398; insgesamt1906Ticks/sechsSprünge. Alle Frames und Endzustände bleiben gleich zum vorherigen Stand. Auch2340Legacy-Controller-Ticks sind identisch. Der gesamte neue JSON-Bericht stimmt nach Entfernung der zwei neuen Runtime-Felder und Normalisierung des aktualisierten Scope-Textes exakt mit dem alten Bericht überein. Reproduzierbarer Vergleich: `scripts/Validate-JumpScript.py`. Die Normalfälle setzen currentJumpZ=Default.JumpZ475 und keine Duck-Anforderung; die abweichenden Bedingungen werden durch die neuen synthetischen Tests geprüft.

Die anschließend implementierte, separate Controller-Sprung-/Duck-Phase folgt dem überprüften ProcessMove-/PlayerMove-Abschnitt. Die ursprüngliche geometrische Diagnose behält ihren eigenen Eingabe-Latch. Details: [WALKING_INPUT.md](WALKING_INPUT.md).
