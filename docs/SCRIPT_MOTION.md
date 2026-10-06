# Script-Sprungphase mit PC-Bewegung verbunden

`VolumeWorld::script_motion_tick` verbindet die gegen Bytecode geprüfte PlayerWalking-Sprung-/Duck-Phase mit der bestehenden statischen Walking-/Falling-Bewegungsdiagnose. Eingabeflags und geometrische Bewegung werden nur gemeinsam nach einem vollständig erfolgreichen Tick zurückgegeben. Dies bleibt eine PC-Diagnose; eine vollständige Script-VM oder native Pawn-Zustandsmaschine ist nicht vorhanden.

## Eingabe und Zustand

ScriptBody speichert Körperzustand, bPressedJump und bWantsToCrouch. ScriptMotionInput enthält Bewegungsachsen/View sowie einen getrennten jump_event, CannotJumpNow-Rückgabesnapshot, bDuck und bCanCrouch. ViewInput.jump muss false sein, damit Script-Event und eigene physische Eingabeflanke nicht vermischt werden. jump_event wird mit dem gespeicherten bPressedJump per OR verknüpft. Körperbezogenes wants_to_crouch stammt im neuen Pfad aus ScriptBody; das gleichnamige RuntimeOptions-Feld wird hier nicht als Zustandsquelle verwendet.

Die eigene statische Bodenprüfung bestimmt vor der Script-Phase Walking oder Falling. Nach der Script-Phase wird ihre Velocity an den gemeinsamen Bewegungsadapter übergeben, dessen eigener Jump-Latch deaktiviert ist. Erfolgreiches DoJump wird dadurch genau einmal angewendet. Der gemeinsame Adapter prüft Volume und Boden erneut gegen dieselbe unveränderte statische Geometrie und führt die Walking-/Falling-Rechnung samt AABB-Kollision aus. Die im Report enthaltene input_phase ist der Snapshot vor der Bewegungsintegration; state enthält deren tatsächliches Ergebnis und die erfolgreich übernommenen Flags.

## Ducken und Fehler

Aktives Hocken bleibt unzulässig, solange der Körpergrößenwechsel fehlt. Auch eine nach der Script-Phase weiterhin gesetzte Duck-Anforderung liefert einen expliziten Fehler statt eines stehenden Bewegungskandidaten. Eine bestehende Anforderung kann bei Walking und bDuck0 gelöscht werden; ein im selben Schritt angeforderter Sprung bleibt dann wie im Script abgelehnt. Ein erfolgreicher Sprung aus dem Stehen überspringt dagegen die neu angeforderte Duck-Zuweisung wegen Falling und kann ausgeführt werden.

Ungültige Eingabekombinationen, fehlende Geometrie, unvollständige Volume-Auswahl, unsupported Wasser oder Kollisionsfehler verändern den übergebenen Körper und gespeicherte Flags nicht. Der Aufrufer bekommt dann keinen neuen Zustand. Ein frischer äußerer jump_event ist kein intern gespeichertes Betriebssystemereignis; sein Aufrufer muss bei Fehlern selbst über einen erneuten Versuch entscheiden.

## Originalkarten-Proben

`rc-mesh-probe <GameData> <map.ctm> <report.json> --script-diagnostic` aktiviert den neuen Pfad. Es werden dieselben Startpunkte und Bewegungsphasen wie im bisherigen Pawn-Diagnoseablauf verwendet. Events werden nur in first_jump und second_jump erzeugt. In Flug-/Ruhephasen wird kein weiterer Event geliefert; das ist kein nachgewiesenes physisches Held-Key-Mapping. CannotJumpNow=false entspricht dem geprüften Basisscript, bCanCrouch=true wird aus Engine.Pawn mit Herkunft geladen. Das Profil verwendet die bisherigen aufgelösten Originalwerte, currentJumpZ=defaultJumpZ475, gesund/stehend und BaseNone.

Die Ergebnisse und der Vergleich zum früheren eigenen Sprung-Latch stehen unter `analysis/collision/*-script-motion.json` und `script-motion-validation.json`. `scripts/Validate-ScriptMotion.py` prüft alle Körper-, Bewegungs-, Volume- und Modusframes, Eventanzahl, DoJump-Aufrufe, Flag-Endzustände und sämtliche unveränderten Legacy-Berichtsteile gegen `*-jump-script.json`.

## Prüfung und verbleibende Grenzen

116 Workspace-Tests und Clippy über alle Targets bestehen. Vier neue Integrationstests prüfen gespeicherten Event mit späterem Sprung, verbrauchten Luft-Event und frischen Sprung nach Landung, Duck-Reihenfolge samt explizitem Body-Transition-Fehler sowie atomische Fehler bei Volume/Geometrie/mehrdeutiger Eingabe. Unterstützungsprüfung, eigener nächster Modus, AABB-Kollision und Velocity-Rechnung bleiben Diagnosepolitik. Es gibt keine native Restzeitaufteilung, Callbacks, dynamischen Bases, Wasserbewegung, vollständige Eingabebindungen, Replikation oder Android-Verbindung.

Android-Prüfung wie gewünscht zum Schluss; kein APK-Build oder Emulatorlauf.

## Ergebnisse vom 2026-10-06

Der neue Script-Event-Pfad wurde auf drei Originalstarts über insgesamt1906Ticks geprüft: geo_01a PlayerStart0 mit280, dm_hangar PlayerStart8 mit228 und PlayerStart44 mit1398Ticks bei GravityZ-110. Je Start genauzweiEvents, zweiDoJump-Aufrufe und zwei erfolgreich begonnene Sprünge. Alle Endzustände liegen am Boden mit Velocity0, pressed_jump=false und wants_to_crouch=false. Kein Bewegungs-/Volume-Importfehler.

Sämtliche Körper-, Motion-, Volume- und Modusframes stimmen mit den vorherigen eigenen Edge-Diagnosen überein. Die Script-Flags/Phasensnapshots werden zusätzlich ausgewiesen und unterscheiden sich ausdrücklich vom alten Held-Key-Latch. Alle2340Legacy-Controllerframes und die übrigen bisherigen Berichtsteile bleiben unverändert. Der Vergleich bestätigt den integrierten statischen Diagnosepfad, keine native Gameplay-Laufzeitparität. Gespeicherte CannotJumpNow-Events und Duck-Übergangsfälle werden durch synthetische Integrationstests abgedeckt, nicht durch diese normalen Kartenabläufe.

## Folgearbeit: isolierter Größenwechsel

Ein separater Adapter prüft inzwischen die originalen Duck-/Stehmaße mit Höhenkompensation und blockiert Aufstehen bei unfreier Zielform. Die automatische Einbindung in script_motion_tick ist noch offen; dessen bisherige Größenübergangsfehler bleiben in diesem Stand bestehen. Details: [CROUCH_COLLISION.md](CROUCH_COLLISION.md).

## Folgearbeit: integrierte Duckbewegung

Der zusätzliche Pfad crouching_script_tick verwendet jetzt die native normale Dispatch-Reihenfolge mit Größenwechsel, Duckgeschwindigkeit und Aufstehversuchen vor/nach Bewegung. Die bisherigen separaten/Standing-Pfade dokumentieren weiterhin ihre damaligen Grenzen. Auto-Uncrouch-Timer bleibt explizit unsupported. Details: [CROUCH_MOTION.md](CROUCH_MOTION.md).
