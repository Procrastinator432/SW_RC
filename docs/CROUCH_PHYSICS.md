# Duck-Physics ohne Spielereingabephase

`VolumeWorld::crouching_physics_tick` verarbeitet gespeicherte Wants-/Try-/UncrouchTime-Werte, Körpergröße und Bewegung ohne PlayerWalking.ProcessMove. `CrouchPhysicsInput` enthält diagnostische Bewegungsachsen und CanCrouch. ViewInput.jump muss false sein. Die Methode führt keinen Sprung aus und erhält das gespeicherte pressed_jump unverändert; sie verbraucht keine Script-Ereignisse und trifft keine KI-Sprungentscheidung.

Der Ablauf verwendet die bereits geprüfte performPhysics-Reihenfolge: vollständige Prüfung von Körperform, Volume und tatsächlichem statischem Bodenkontakt; Ducken vor Walking-Bewegung beziehungsweise Aufstehen vor Falling; Countdown nur bei Walking/Wants/Can/bereitsCrouched/Try; Bewegung mit aktuellem Shape und SpeedFactor; gegebenenfalls Aufstehen danach. Es gibt keine PlayerWalking-ShouldCrouch-Zuweisung, die eine KI-Anforderung durch duck0 löschen würde. Die geometrisch bestimmte Physics ersetzt weiterhin den vollständigen nativen Pawn-Physicszustand.

Der bestehende `crouching_script_tick` führt weiterhin zuerst die geprüfte Spielereingabephase aus und übergibt anschließend deren Velocity, Wants- und Pending-Event-Ergebnis an denselben Physics-Einstieg. Nach DoJump liegt positive Z-Velocity vor; die gemeinsame Bodenprüfung behandelt diesen Zustand als Falling. Das Script-Ergebnis kennzeichnet den gestarteten Sprung weiterhin ausdrücklich. Der Physics-Einstieg selbst startet keinen Sprung. Scriptphase und Physics liefern gemeinsam erst bei vollständigem Erfolg einen neuen Zustand. movement.state bleibt der Zustand vor optionalem abschließendem Größenwechsel, state der endgültige Körperzustand.

Drei neue Integrationstests prüfen:

- Expliziter KI-Wandkontakt aktiviert den Timer über die erste HitWall-Prüfung. Der Physics-Einstieg duckt ohne Spielereingabe, erhält den gespeicherten Sprungevent und führt ihn nicht aus. Der erste Duck-Tick erhält Timer0,5; zehn folgende Schritte zu0,05 lassen den f32-Timer ablaufen und richten danach auf.
- Ein durch CanCrouchWalk aktivierter Körper bewegt sich geduckt unter eine niedrige Decke. Timerablauf löscht Wants/Try trotz blockierten Aufstehversuchs. Ohne neue PlayerWalking-Phase wird später erneut versucht; Entfernen der Decke ermöglicht Aufstehen.
- Jumpinput und fehlende Geometrie liefern Fehler ohne Änderung von Körper, Flags oder Timer.

138 Workspace-Tests, Clippy und Formatprüfung bestehen. Originalkarten-Regressionen werden mit scripts/Validate-PhysicsCrouch.py gegen die bisherigen Duckbewegungsberichte verglichen. Hinzugefügte inaktive uncrouch_time0-Felder und die aktualisierte Scopebeschreibung werden ausdrücklich aus dem Vergleich entfernt; alle übrigen Werte müssen identisch sein. Die Originalkartenläufe verwenden weiterhin den Spielerpfad und belegen keine Original-KI-Laufzeitparität.

Wandkontakte werden noch nicht automatisch aus dem Bewegungsrechner in HitWall dispatcht. Native vertikale Recovery, zweiter Duckversuch, Controller-/Actor-Callbacks, dynamische Actorfilter und native Cylinder-/SSE-Parität bleiben offen. Die Bewegungsachsen sind diagnostische Vorgaben, keine rekonstruierte KI-Zielsteuerung. Das Zusammenspiel von expliziter HitWall-Aktivierung und Physics ist synthetisch geprüft. Android bleibt bis zum Schluss zurückgestellt.

## Originalkarten-Ergebnis

Die Regression besteht für geo_01a und dm_hangar:210Duckbewegungs-Ticks,1906Script-Bewegungs-Ticks und2340Legacy-Controller-Ticks sind vollständig identisch zum gespeicherten Vorstand. Nur neu hinzugefügte uncrouch_time0-Felder und die aktualisierte Scopebeschreibung werden aus dem Vergleich entfernt. Nachweis: analysis/collision/physics-crouch-validation.json; neue Rohberichte *-physics-crouch.json. Die Timeraktivierung selbst bleibt synthetisch geprüft.

## Folgearbeit: Abwärtsschritt

Der berechtigte Walking/nonhuman Auto-Crouch-Zweig besitzt jetzt einen zusätzlichen Recovery-Adapter mit Abwärtsschritt und zweitem Duckversuch. Korrektur der früheren Richtungsannahme: Native Konstante1067667c ist-35, keine Aufwärtsbewegung. Eigene statische Query mit Skin und geprüfter Zielplatzierung; MoveActor-Callbacks und automatischer Kontaktdispatch weiterhin offen.143Tests bestehen. Siehe [HIT_WALL_RECOVERY.md](HIT_WALL_RECOVERY.md).

## Folgearbeit: statische Kontaktweiterleitung

Ein zusätzlicher Diagnosepfad verbindet jetzt den eigenen vollständigen Physics-Schritt mit dem ersten berechtigten statischen Wandkontakt und der HitWall-/Recovery-Prüfung. Eingehende Velocity für den Ereignisgate, kollisionsbegrenzte Endvelocity für die Zustandsübernahme; Wallmetadaten ausdrücklich erforderlich.147Tests bestehen. Die Weiterleitung erfolgt nach dem Schritt, native innerhalb-des-Schritts-Reihenfolge bleibt offen. Siehe [AI_CONTACT.md](AI_CONTACT.md).
