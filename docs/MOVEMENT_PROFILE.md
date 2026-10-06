# Originalprofil und blickrichtungsabhängige PC-Steuerung

`MovementProfile::read` löst 14 Eigenschaften aus Originalpaketen und Properties-Overrides auf, erhält sämtliche Herkunftsdaten und erstellt daraus die Optionen für den bestehenden PC-Controller. Fehlende, falsch typisierte oder nicht unterstützte Parameter erzeugen einen Fehler; es gibt keine stillen Ersatzwerte. Schwerkraft muss für die derzeitige PC-Integration ausschließlich nach unten gerichtet sein.

Angewendet: GroundSpeed450, AccelRate1024, DecelRate600, AirControl~0,35, JumpZ475, WalkSpeedRatio0,5, BackSpeedRatio~0,8, SideSpeedRatio~0,95, Körperradius40/Halbhöhe84, Gravity[0,0,-1100] und TerminalVelocity12000. MaxFallSpeed1200 und GroundFriction8 bleiben als Herkunftsdaten erhalten, werden aber nicht in die eigene Formel eingesetzt. MaxFallSpeed ist keine Terminalgeschwindigkeit.

MAXSTEPHEIGHT35 und MINFLOORZ0,7 stammen aus dem wiederhergestellten Actor-Quelltext (Zeilen657/658); sie werden als Schrittgrenze und begehbare Normalengrenze verwendet. Sicherheitsabstand0,5, Bodenabfrage2 und acht Iterationen bleiben eigene Integrationsparameter. Die Schwerkraft wird aus dem Basisdefault von Engine.PhysicsVolume übernommen. Der tatsächlich am jeweiligen Punkt aktive PhysicsVolume wird noch nicht ermittelt, ebenso fehlen Instance-/Runtime-Änderungen.

`ViewInput::world_input` erhält Vorwärts-/Seitwärtseingabe, Unreal-Yaw, Gehmodus und Sprungtaste. Zuerst wird die lokale Eingabe auf maximale Länge eins begrenzt, danach werden Rückwärts-/Seitwärtsfaktor und optional der Gehfaktor angewendet. Die anschließende Drehung verwendet quantisierten Yaw ((yaw>>2)&16383) und ignoriert Blick-Pitch, damit Gehen horizontal bleibt. Die f64-Drehung ist eine eigene numerische Umsetzung; Originalformel und f32-Parität sind nicht bestätigt.

Diese Gewichtungsreihenfolge ist eine eigene PC-Regel. Die Originalwerte belegen noch nicht, dass alle Bewegungsmodi der Original-Engine dieselbe Formel verwenden. Richtungswechsel und Gehmodus ändern die Zielgeschwindigkeit; die tatsächliche Geschwindigkeit nähert sich ihr über die vorhandene Beschleunigungs-/Bremsregel.

## Nachweise vom 2026-10-06

70 Workspace-Tests und Clippy über alle Targets bestehen. Drei neue Tests prüfen Yaw-Drehung, volle Umdrehungen und Winkelquantisierung, Rückwärts-/Seitwärts-/Gehfaktoren und diagonale Begrenzung; die tatsächlichen Zielgeschwindigkeiten werden außerdem über mehrere Körperphysikschritte geprüft. Das Profil lehnt fehlende Werte, nicht vertikale Schwerkraft und ungültige Terminalgeschwindigkeit ab.

Auf geo_01a und entry wurde je ein Startpunkt geprüft, auf ctf_hangar die ersten zwei von 40. Je300 feste Schritte bei1/60 Sekunde, insgesamt1200: Fallen/Ruhe, diagonal in Blickrichtung bewegen, abbremsen, gehaltene Sprungtaste und anschließende Ruhe. Yaw stammt aus der jeweiligen Original-PlayerStart-Rotation. Alle vier Abläufe haben genau einen Sprung und enden ohne Query-Fehler bodengebunden mit Geschwindigkeit null. Die Höhendrift in der abschließenden Ruhephase beträgt null. Maximale Geschwindigkeit in diesen diagonalen Abläufen: ca.438,894204458, unter der Grundgrenze450.

Zusätzlich sind Vorwärts-, Rückwärts-, Seitwärts- und Geheingaben bei Yaw0 und16384 geprüft. Zielgeschwindigkeiten:

| Modus | Zielgeschwindigkeit |
|---|---:|
| Vorwärts | 450 |
| Rückwärts | ca.360,000005364 |
| Seitwärts | ca.427,499994636 |
| Gehen vorwärts | 225 |

Die Abweichungen bei Rückwärts/Seitwärts stammen aus den original gespeicherten f32-Faktoren. Diese Zahlen sind Zielwerte der eigenen Steuerungsregel, keine Messung der Original-Engine.

Berichte: `analysis/collision/*-profile-probes.json`; Zusammenfassung `analysis/collision/profile-validation.json` und `analysis/evidence.json`. In denselben großen Diagnoseberichten bleiben die älteren direkten Physics-/Stufenproben mit ihren bisherigen Diagnoseparametern getrennt erhalten; nur die Controller-Proben verwenden das neue Originalprofil.

Offen: aktive Volumes, native Beschleunigungs-/Brems-/Richtungsformeln, bewegte Geometrie, Spawn-Auswahl, Crouch/Ladder/Spider, Animationen und JNI-/Szenen-/Android-Integration. APK und Emulator bleiben unverändert; Android-Prüfung auf Nutzerwunsch zum Schluss.


Aktualisierung: Die statische Volume-Auswahl und ein PC-Steuerungsadapter sind jetzt umgesetzt. Die oben genannten Basisprofil-Trajektorien bleiben historische Nachweise; kombinierte Originalkarten-Trajektorien mit wechselnden Volumes sind noch offen. [Aktueller Stand](PHYSICS_VOLUMES.md).
