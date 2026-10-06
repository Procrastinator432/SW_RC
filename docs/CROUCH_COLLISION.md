# Kollisionskörper beim Ducken und Aufstehen

`StaticBodyWorld::crouch_diagnostic` implementiert die aus den nativen Crouch-/UnCrouch-Funktionen belegte Höhenkompensation mit einer eigenen vollständigen statischen AABB-Prüfung. Der Größenwechsel ist separat prüfbar; die Script-Bewegungsbrücke verwendet ihn in diesem Stand noch nicht automatisch. Fortbewegung im Ducken und die native Aufrufreihenfolge im Physics-Tick bleiben offen.

## Originalwerte und native Funktionen

Der aufgelöste PlayerCommando besitzt Standing-Extent[40,40,84] und Crouch-Extent[40,40,56]. CollisionHeight und CrouchHeight stammen aus CTCharacters.CloneCommando, beide Radien aus Engine.Pawn. Die Zahlen sind Halbhöhen, keine gesamte Körperhöhe. Herkunft und Payload-Offsets stehen in `analysis/reports/crouch-defaults.json`; reproduzierbar mit `rc-crouch-defaults <GameData> <report.json>`.

Aus engine.dll wurden APawn.Crouch104819c0, UnCrouch10481c80 und execForceCrouch10485040 nach `analysis/decompiled/pawn-crouch.c` exportiert. Crouch ruft SetCollisionSize(CrouchRadius,CrouchHeight), berechnet alteHöhe-CrouchHeight und verschiebt den Mittelpunkt um diesen Betrag nach unten. UnCrouch lädt die Standardmaße, berechnet Standardhöhe-aktuelleHöhe und verschiebt nach oben. Für den Spieler sind dies jeweils28Einheiten; die Fußposition bleibt unverändert.

Beim nativen Crouch gibt es im normalen param0-Pfad eine Blockierprüfung, falls Radius oder Höhe größer werden. Beim UnCrouch wird die größere Zielform geprüft; eine Blockade oder fehlgeschlagener FarMove stellt die Duckmaße wieder her. Native Kollisionshash-/IsBlockedBy-Regeln und FarMoveActor sind noch nicht rekonstruiert. Bei Erfolg werden DrawScale/Flags angepasst und StartCrouch/EndCrouch aufgerufen. Der param1-Pfad betrifft andere Runtimebedingungen und wird hier nicht umgesetzt. ForceCrouch ruft Crouch(this,0) auf.

## Eigene statische Diagnosepolitik

CrouchProfile hält beide Maße plus Herkunft. CrouchBody hält BodyState und das aktuelle crouched-Flag. Der Adapter akzeptiert positive, endliche Maße bis1.000.000, kreisförmige XY-Halbachsen sowie kleinere Duckhöhe und nichtgrößere Duckradien. Diese Beschränkung passt zum Originalspieler, deckt aber nicht sämtliche nativen Wachstumssonderfälle ab.

Zunächst muss der aktuelle Körper mit vollständig bekannter Geometrie frei oder berührend sein. Penetration oder fehlende Geometrie ist ein Fehler ohne Kandidaten. Ein identischer angeforderter Zustand liefert Unchanged. Beim Wechsel wird der Mittelpunkt angepasst und die Zielform vollständig geprüft. Freie/berührende Ziele liefern Changed. Penetrierende Ziele liefern Blocked mit exakt erhaltenem Körper, aktueller Form und crouched-Flag; die abgewiesene Zielprüfung ist im Bericht sichtbar. Auch beim Verkleinern wird geprüft, und Berührung ist erlaubt: beides sind ausdrückliche eigene Diagnoseentscheidungen.

Velocity und grounded werden übernommen; der Helfer integriert keine Bewegung und prüft grounded nicht erneut. Der signierte Berichtswert height_adjustment ist alteHöhe-Zielhöhe: +28beimDucken und-28beimAufstehen. Sein Betrag entspricht dem nativen HeightAdjust für StartCrouch/EndCrouch; der native EndCrouch-Parameter ist positiv. Kamera, EyeHeight, DrawScale, Sounds und Script-/Physics-Ereignisse werden nicht ausgeführt.

## Prüfung

120 Workspace-Tests, Clippy über alle Targets und Formatprüfung bestehen. Vier neue Tests prüfen wiederholbaren Fuß-/Velocity-/grounded-Erhalt, verhindertes Aufstehen bei niedriger Decke, erlaubte exakte Berührung und Radiusverkleinerung sowie Fehler bei Penetration, ungültigen Maßen oder fehlender Geometrie.

`rc-mesh-probe <GameData> <map.ctm> <report.json> --crouch-diagnostic` führt zuerst den bisherigen Script-Bewegungsablauf aus und dann zehn isolierte Duck-/Aufstehzyklen an jedem vollständig erreichten Endpunkt. Es bewegt den duckenden Körper in diesen Zusatzproben nicht. Berichte: `analysis/collision/*-crouch.json`; Vergleich und Größen-/Fußprüfung: `scripts/Validate-Crouch.py` und `crouch-validation.json`.

Native Cylinder-/Encroachment-/FarMove-Parität, dynamische Actors, Callbacks, clientseitige Varianten, Crouch-Timing und duckende Fortbewegung bleiben offen. Die Script-Bewegungsbrücke weist angeforderte Größenübergänge weiter explizit ab, bis der neue Adapter mit einer geprüften Reihenfolge verbunden ist. Android-Prüfung wie gewünscht zum Schluss; kein APK-Build oder Emulatorlauf.

## Originalkarten-Ergebnisse vom2026-10-06

An den drei Endpunkten geo_01a.PlayerStart0, dm_hangar.PlayerStart8 und PlayerStart44 wurden je zehn vollständige Duck-/Aufstehzyklen ausgeführt: insgesamt60Größenwechsel. Jede Zielform frei oder berührend, jeder Wechsel Changed, Fußposition exakt erhalten und jeder Endzustand exakt gleich dem Anfangszustand. Die beiden Hangar-Endpunkte liegen in Standard- beziehungsweise geringer Schwerkraft; der isolierte Größenwechsel integriert keine Schwerkraft.

Alle1906Script-Bewegungsschritte,2340Legacy-Controllerframes und bisherigen Berichtsfelder bleiben unverändert gegenüber den Script-Motion-Berichten. Nur crouch_profile/crouch_probes/crouch_scope wurden ergänzt. Blockiertes Aufstehen wurde synthetisch unter einer niedrigen Decke geprüft; diese drei Originalendpunkte sind frei und liefern dafür keinen Original-Laufzeitnachweis. Nachweis: `analysis/collision/crouch-validation.json`.

## Folgearbeit: integrierte Duckbewegung

Der zusätzliche Pfad crouching_script_tick verwendet jetzt die native normale Dispatch-Reihenfolge mit Größenwechsel, Duckgeschwindigkeit und Aufstehversuchen vor/nach Bewegung. Die bisherigen separaten/Standing-Pfade dokumentieren weiterhin ihre damaligen Grenzen. Auto-Uncrouch-Timer bleibt explizit unsupported. Details: [CROUCH_MOTION.md](CROUCH_MOTION.md).
