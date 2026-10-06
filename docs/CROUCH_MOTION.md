# Bewegung im Ducken mit Physics-Reihenfolge

`VolumeWorld::crouching_script_tick` verbindet die geprüfte Script-Sprung-/Duck-Phase, den Größenadapter und die Walking-/Falling-Rechnung. Es ist ein zusätzlicher PC-Diagnosepfad mit explizitem Duckzustand. Die bisherigen Standing-/Edge-Pfade bleiben für ihre dokumentierten Regressionen bestehen.

## Belegte Reihenfolge

APawn.performPhysics104955f0 wurde aus engine.dll nach `analysis/decompiled/pawn-physics-dispatch.c` exportiert. Im normalen Pfad ergibt sich:

1. Bei PHYS_Walking, bWantsToCrouch und bCanCrouch wird vor startNewPhysics Crouch aufgerufen, falls bIsCrouched noch false ist.
2. Bei anderer Physics und bIsCrouched wird vor startNewPhysics UnCrouch versucht.
3. startNewPhysics führt die Bewegung aus.
4. Danach wird bei weiterhin geducktem Körper UnCrouch versucht, wenn Physics nicht Walking oder bWantsToCrouch=false ist.

Damit nutzt der Walking-Schritt beim Loslassen noch Duckmaße und Duck-SpeedFactor; erst anschließend wird aufgerichtet. Ein blockierter Aufstehversuch lässt den Körper geduckt, während die Anforderung bereits false sein kann. Der nächste Schritt versucht es nach der Bewegung erneut. DoJump prüft zuvor den tatsächlichen Duckzustand: Ein in diesem Schritt abgelehnter Sprung wird nicht automatisch nach erfolgreichem Aufstehen ausgeführt.

Der native zusätzliche Zweig bTryToUncrouch ist jetzt als Countdown integriert: Nur Walking + Wants + Can + bereits Crouched + Try dekrementiert Pawn+0x650 um dt. Der else-if überspringt den Countdown beim ersten Größenwechsel. Bei Ergebnis <=0 werden Wants und Try gelöscht (Maske0xfffffff5), der Timer bleibt bei seinem Ergebnis, ohne Clamp. Die Bewegung erfolgt noch geduckt, der Aufstehversuch danach. KillZ/FellOutOfWorld, native Physikdispatch-Sondermodi, Animation-/PostTouch-/Controller-Callbacks und Zeitaufteilung sind weiterhin außerhalb dieses Pfads.

## Rust-Adapter

CrouchingScriptBody enthält ScriptBody plus tatsächliches crouched und try_to_uncrouch. CrouchMotionOptions liefert Runtimefaktoren und das aufgelöste CrouchProfile. Der Steh-Extent muss zum MovementProfile passen; Zustand und Shape werden vollständig geprüft. Die eigene statische Bodenprüfung liefert Walking/Falling für die Script-Phase. Script und Physics verwenden denselben bCanCrouch-Snapshot.

Die Script-Phase verarbeitet gespeicherten Event und Duck-Anforderung. Anschließend werden vor der Bewegung nötige Größenwechsel geprüft. Der gemeinsame Bewegungsrechner erhält den tatsächlichen Extent und Crouch-SpeedFactor; sein eigener Jump-Latch bleibt deaktiviert. Danach folgen gegebenenfalls Aufstehversuche. Erst wenn alle Queries und die Bewegung vollständig erfolgreich sind, werden Körper, Duckzustand und Scriptflags gemeinsam zurückgegeben. Blocked ist ein gültiger unveränderter Größenentscheid; unbekannte Geometrie bleibt ein Fehler ohne Teilzustand.

Der Report trennt before_movement, movement und after_movement. movement.state ist der Zustand vor dem möglichen abschließenden Aufrichten; state ist der tatsächlich zurückgegebene Endzustand. Runtime.crouched/wants_to_crouch werden aus dem gespeicherten Körper-/Scriptzustand gesetzt. Das Profil für die Rechenphase benutzt die ursprünglichen Skalare und das aktuelle Shape; Propertyherkunft wird separat mit dem Originalprofil protokolliert.

Native Cylinder-/Encroachment-/FarMove-Toleranzen, DrawScale-/EyeHeight-/Soundereignisse, dynamische Bases und SSE-f32-Parität bleiben offen. Volume wird vor der Scriptphase sowie im gemeinsamen Bewegungsadapter am aktuellen Mittelpunkt geprüft; eine Änderung durch abschließendes Aufrichten wird im nächsten Tick ausgewählt, ohne native Volume-Callbacks.

## Prüfung

127 Workspace-Tests, Clippy über alle Targets und Formatprüfung bestehen. Die bisherigen vier Integrationstests prüfen Duck-Speedcap und Aufrichten nach dem Bewegungsschritt, niedrige Decke/erneuter Versuch/späteren frischen Sprung, Aufrichten vor Falling sowie atomische Fehler; der frühere Timer-Rejection-Test prüft jetzt einen nichtendlichen Timer. Das kleine Testprofil erreicht beim Ducken5 statt Stehtempo10; der Release-Schritt bleibt bei5, der folgende Stehschritt beschleunigt auf6.

Drei zusätzliche Timer-Integrationstests prüfen: erster Duck-Schritt ohne Countdown, f32-Subtraktion und negative Restzeit nach Ablauf, Duckform während des Ablauf-Bewegungsschritts, erneutes Ducken bei weiterhin gehaltenem Duckinput; unveränderter Timer bei fehlendem Try/Can/Wants oder Falling; Ablauf unter niedriger Decke mit gelöschten Flags trotz Blockade und erfolgreicher Wiederholung nach Entfernen der Decke. UncrouchTime wird als endlicher f32-Snapshot übergeben. Seine automatische Initialisierung durch andere Enginefunktionen ist noch nicht rekonstruiert. Inaktive Snapshots bleiben unverändert. Bei ungültigem dt/Timer oder Geometriefehler entsteht kein Teilzustand. Die f32-Subtraktion ist keine Aussage über vollständige x87-/SSE-Laufzeitparität.

`rc-mesh-probe <GameData> <map.ctm> <report.json> --crouch-motion-diagnostic` führt zuerst die bisherigen Script-Abläufe aus und startet anschließend an deren Endpunkten30Ticks Vorwärtsbewegung im Ducken, Bremsen bis Stillstand, einen Release-Schritt und30Ticks Ruhe im Stehen. Yaw0 und diese Eingaben sind Diagnosevorgaben. bCanCrouch und CrouchSpeedRatio werden aus Originaldefaults geladen; Größen sind40/84 beziehungsweise40/56, GroundSpeed450 und Duck-Ratio0,5. Auto-Uncrouch-Timer ist deaktiviert. Budget1200Ticks, dt1/60.

Berichte: `analysis/collision/*-crouch-motion.json`. `scripts/Validate-CrouchMotion.py` vergleicht sämtliche bisherigen Berichtsteile, prüft Endzustände, zählt Crouch-/Falling-/Shape-Ticks und schreibt `crouch-motion-validation.json`. Blockiertes Aufstehen und Timerfälle sind synthetisch geprüft; die Originalkartenabläufe bestätigen statische Diagnosebewegung, keine vollständige Original-Gameplay-Laufzeitparität.

Android-Prüfung weiterhin zum Schluss; kein APK-Build oder Emulatorlauf.


## Originalkarten-Ergebnisse

Die reproduzierbare Validierung besteht für 210 neue Ticks: geo_01a/PlayerStart0 74, dm_hangar/PlayerStart8 74 und dm_hangar/PlayerStart44 62. Jeder Ablauf führt zwei Größenwechsel aus und endet stehend, am Boden, mit Velocity0 und gelöschten Scriptflags. Die letzten30 Ruhe-Ticks zeigen jeweils exakt keinen Höhendrift. Keine Falling-Ticks oder blockierten Aufstehversuche in diesen drei Originalkartenabläufen; diese Fälle bleiben durch synthetische Integrationstests abgedeckt.

Die beiden normalen Abläufe erreichen Ducktempo225 (450*0,5) und benötigen13 Brems-Ticks. PlayerStart44 bleibt in PhysicsVolume5 mit GravityZ-110, erreicht maximal145,0667 und hat zum Ende der30 Bewegungs-Ticks bereits Velocity0 bei einem Kontakt mit der schrägen Fläche von StaticMeshActor131. Der eigene statische Solver begrenzt dort die Bewegung; der einzelne folgende Brems-Tick belegt keine native Bremsparität oder freie Beschleunigung bis225.

Alle bisherigen Berichtsfelder sind gegenüber den Script-Motion-Berichten vollständig identisch, einschließlich1906 Script-Ticks und2340 Legacy-Controller-Ticks. Nachweis: `analysis/collision/crouch-motion-validation.json`. Insgesamt sechs integrierte Größenwechsel. Diese Messwerte ergänzen die isolierten60 Größenwechsel des früheren Crouch-Berichts.

## Folgearbeit: Timeraktivierung

CanCrouchWalk ist jetzt als expliziter statischer Adapter rekonstruiert und aktiviert Wants/Try sowie Timer0,5. Die gefundene processHitWall-Aufrufbedingung gilt für nicht menschlich gesteuerte Figuren. Controller-/KI-Anbindung und native Tracefilter bleiben offen. Siehe [AUTO_CROUCH.md](AUTO_CROUCH.md).131Tests bestehen; frühere Kartenberichte bleiben historische Nachweise.

## Folgearbeit: gemeinsamer Physics-Einstieg

Ein separater Physics-Einstieg ohne PlayerWalking-Phase erhält jetzt die gespeicherte KI-Duckanforderung und den Pending-Sprungevent. Der Spielerpfad delegiert nach seiner Scriptphase an dieselbe Implementierung. Explizites HitWall-Arming bis Timerablauf/Blockade/Retry synthetisch geprüft,138Tests bestehen. Originalkarten210Duck-/1906Script-/2340Legacy-Ticks unverändert bis auf additive inaktive Timerfelder und Scopebeschreibung. Native Recovery und automatischer Kontaktdispatch bleiben offen. Siehe [CROUCH_PHYSICS.md](CROUCH_PHYSICS.md).
