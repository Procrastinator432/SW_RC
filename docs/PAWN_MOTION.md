# Gemeinsame PC-Diagnose fÃ¼r Bodenbewegung und Flug

`VolumeWorld::pawn_diagnostic_tick` verbindet die rekonstruierten Walking-/Falling-Rechenteile mit den bisherigen statischen Kollisionsadaptern. Die Auswahl des Bewegungszweigs und die Verarbeitung der Sprungeingabe sind eigene Diagnosepolitik, keine rekonstruierte native Pawn-/Script-Zustandsmaschine. Android-Steuerung und Ã¤ltere Controller-Proben verwenden diesen neuen Pfad noch nicht.

Vor jedem Tick werden Volume-Einstellungen am KÃ¶rpermittelpunkt und tatsÃ¤chliche BodenunterstÃ¼tzung neu geprÃ¼ft. Ein gespeichertes grounded-Flag allein entscheidet nicht. Bei bestÃ¤tigter UnterstÃ¼tzung und nichtpositiver Z-Geschwindigkeit arbeitet der Walking-Zweig; andernfalls Falling. Der nÃ¤chste Modus wird aus dem geprÃ¼ften Endzustand bestimmt. Nach einer Landung wirkt Bodenreibung damit im folgenden Tick. Bei Kantenabgang verwendet der ausgehende Walking-Tick noch den eigenen KÃ¶rperlÃ¶ser; ab dem nÃ¤chsten Tick arbeitet die Falling-Rechnung. Es gibt keine native Restzeit-Aufteilung innerhalb eines Ticks.

Eine neue Sprungeingabe bei bestÃ¤tigtem Boden setzt Z-Geschwindigkeit auf den aufgelÃ¶sten JumpZ-Wert und wÃ¤hlt Falling. Gehaltene Eingabe bleibt Ã¼ber die Landung hinweg verbraucht. Loslassen und erneutes DrÃ¼cken sind fÃ¼r einen weiteren Sprung erforderlich. Eine in der Luft neu gedrÃ¼ckte Taste wird ebenfalls verbraucht; sie ist nicht automatisch nach der Landung wirksam. Die Eingabe wird erst nach einem erfolgreichen vollstÃ¤ndigen Schritt gespeichert; Fehler verÃ¤ndern weder Zustand noch Sprung-Latch.

Die Eingabe wird nach eigener Yaw-/Normierungsregel in Beschleunigung Ã¼bersetzt. Die rekonstruierte SpeedFactor-Rechnung und die Volume-Bodenreibung wirken im Walking-Zweig; native Luftsteuerungsrechnung und echte eigene Vorausprobe im Falling-Zweig. MaximumDesiredSpeed, Waffen- und Gesundheitsfaktoren werden ausdrÃ¼cklich vom Aufrufer geliefert. Hocken wird in diesem gemeinsamen Pfad abgewiesen, weil die zum Stehprofil gehÃ¶renden KÃ¶rpermaÃŸe noch nicht dynamisch angepasst werden.

## Tests vom 2026-10-06

94 Workspace-Tests und Clippy Ã¼ber alle Targets bestehen. Vier neue Tests prÃ¼fen:

- Abheben, Landung und Wechsel zur Bodenbewegung; gehaltene Taste startet keinen weiteren Sprung, Loslassen und neues DrÃ¼cken erlauben ihn.
- In der Luft gedrÃ¼ckte Taste bleibt bei Landung verbraucht; im folgenden Walking-Tick wirkt die ausgewÃ¤hlte Bodenreibung bis zum Stillstand.
- Kantenabgang mit anschlieÃŸendem Falling-Zweig und Luftbeschleunigung.
- Nicht unterstÃ¼tzte Volumes, Hocken und fehlende Geometrie liefern keinen Teilzustand und verbrauchen die Sprungeingabe nicht.

## Originalkarten-Diagnoseablauf

`rc-mesh-probe <GameData> <map.ctm> <report.json> --pawn-diagnostic` aktiviert den zusÃ¤tzlichen Pfad. Der erste Start und gegebenenfalls ein weiterer mit abweichender Schwerkraft werden geprÃ¼ft. Die Probe startet mit Geschwindigkeit0, wartet auf Bodenkontakt, bewegt30Ticks diagonal und bremst bis zum Stillstand. Dann folgen ein Sprung mit gehaltenem Knopf Ã¼ber die Landung und30weitereTicks, ein Tick Loslassen, ein zweiter Sprung und schlieÃŸlich30Ticks bestÃ¤tigter Ruhe. Bei normaler Schwerkraft betrÃ¤gt das Budget600Ticks, bei zunÃ¤chst geringer Schwerkraft1800; dt ist1/60Sekunde. UnvollstÃ¤ndiger Ablauf oder eine andere Anzahl als zwei SprÃ¼nge erzeugt einen expliziten Fehler.

Die Probe setzt MaximumDesiredSpeed=GroundSpeed, Waffenfaktor1 und gesund/stehend voraus. CrouchSpeedRatio und WoundedSpeedRatio werden mit Herkunft aus dem Katalog aufgelÃ¶st, obwohl Hocken und Verwundung auf diesen Wegen nicht aktiviert werden. Der Ablauf ist eine vorgegebene Diagnoseeingabe, keine Mission oder Original-Spielersteuerung.

## Verbleibende Grenzen

Native Script-Sprungfreigaben, zusÃ¤tzliche Runtimeflags, automatischer Gehmodus, ModifyVelocity/Landed/NotifyJumpApex und native ZustandsÃ¼bergÃ¤nge fehlen. Gerade der Sprung setzt hier eine eigene Eingabeflanke voraus; seine Berechtigung und Reihenfolge gegenÃ¼ber anderen Original-Events sind nicht rekonstruiert. AABB-Kollision, Support-Snap und Velocity-Projektion bleiben eigene Regeln. Dynamische Actors, Wasser, bewegte Volumes, Ã„nderungen innerhalb eines Ticks, lÃ¤ngere kollisionsbehaftete Substeps und SSE-f32-ParitÃ¤t sind offen.

Android-PrÃ¼fung auf Nutzerwunsch zum Schluss; kein APK-Build, keine JNI-Verbindung und kein Emulatorlauf in diesem Schritt.

## Ergebnisse auf Originalkarten

1.906Ticks an drei Originalstartpunkten, jeweils genau zwei erfolgreich begonnene Sprünge, Landungen, Bremsen und bestätigte Ruhe. Keine Volume-Import- oder Bewegungsfehler; sämtliche Volume-Auswahlen vollständig:

| Karte / Start | gesamte Ticks | Bremsphase | Flugmodus-Ticks | Bodenmodus-Ticks |
|---|---:|---:|---:|---:|
| geo_01a / PlayerStart0 | 280 | 24 | 165 | 115 |
| dm_hangar / PlayerStart8 | 228 | 24 | 113 | 115 |
| dm_hangar / PlayerStart44 | 1398 | 240 | 1067 | 331 |

Alle drei Endzustände liegen am Boden mit Geschwindigkeit0, freigegebener Sprungeingabe und Höhendrift0 in den letzten30Ticks. Die Hangar-Probe mit geringer Schwerkraft bleibt im PhysicsVolume5 mit GravityZ-110; ihre zwei Flugphasen umfassen jeweils517Ticks einschließlich des Abhebeticks. In den Standardbereichen sind es jeweils51Ticks. Die30Ticks mit weiter gehaltener Taste nach der ersten Landung erzeugen keinen zusätzlichen Sprung. In geo_01a wird während der anfänglichen Fallphase PhysicsVolume1 bei Tick25 betreten und bei Tick33 verlassen; beide Bereiche haben GravityZ-1100.

Die vorherigen Controller-Trajektorien wurden im selben Lauf erneut ausgeführt: Alle2340Frames und Endzustände bleiben identisch mit den bestehenden Volume-Berichten. Berichte: `analysis/collision/geo_01a-pawn-diagnostic.json`, `dm_hangar-pawn-diagnostic.json`; Zusammenfassung `pawn-motion-validation.json` im selben Ordner und `analysis/evidence.json`.

Diese Abläufe bestätigen die gemeinsame Diagnose gegen gespeicherte Originalgeometrie. Sie bestätigen keine native Script-Sprungfreigabe oder vollständige Original-Zustandsmaschine; Gehmodus-/Gesundheitsbedingungen des nativen Sprungpfads werden noch nicht geprüft.

## Folgearbeit: gespeicherte Script-Sprungregeln

Die obigen Ergebnisse mit94Tests dokumentieren den früheren Stand. Der Walking-Sprungzweig nutzt jetzt die Bedingungen und Velocity-Zuweisung des extrahierten Engine.Pawn.DoJump: wants_to_crouch sperrt; Gehmodus verwendet Default.JumpZ, sonst current JumpZ. Der Aufrufer liefert den aktuellen Wert getrennt. Im gespeicherten Funktionskörper gibt es keine Gehmodus- oder Gesundheitssperre. Eingabeflanke, geometrische Moduswahl und Ereignisverarbeitung bleiben eigene Diagnose bzw. offen. Details und Grenzen: [JUMP_SCRIPT.md](JUMP_SCRIPT.md).
