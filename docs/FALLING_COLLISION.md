# Falling-Rechenteil mit Weltkollision

`StaticBodyWorld::falling_diagnostic_tick` verbindet die rekonstruierte Falling-Rechnung mit dem vorhandenen statischen AABB-Körperlöser. Der zusätzliche Pfad wird mit `rc-mesh-probe <GameData> <map.ctm> <report.json> --falling-diagnostic` aktiviert. Bestehende Controller- und Walking-Proben behalten ihre bisherigen Regeln.

## Ablauf und Nachweisgrenzen

Pro Tick wird der Volume-Mittelpunkt neu ausgewählt. Der Aufrufer übernimmt dessen Gravity und TerminalVelocity sowie die Originalprofilwerte AccelRate, AirControl, GroundSpeed und Körpermaße. Unvollständige Volume-Auswahl oder nicht unterstützte Einstellungen liefern keinen neuen Zustand.

Die freie Rechnung beschreibt zunächst die erforderliche horizontale Vorausprobe. Ein tatsächlicher `sweep_motion` gegen klassifizierte Welt-/Mesh-Körper prüft die Strecke mit voller Körperausdehnung. Ein Kontakt schaltet den wirksamen AirControl-Wert ab und berechnet die Integration erneut. Eine unvollständige Query erzeugt einen Fehler. Der vorläufige freie Plan wird nicht als bestätigte Bewegung zurückgegeben.

Anschließend wird die kombinierte XYZ-Verschiebung durch `slide_body` geprüft. Nach Kontakten wird die Geschwindigkeit wiederholt auf die Kontaktflächen projiziert; fehlende Konvergenz, Eindringen oder erschöpftes Bewegungsbudget sind Fehler. TerminalVelocity begrenzt den resultierenden Geschwindigkeitsvektor nach der Bewegungs-/Kontaktregel. Der Frame trennt den freien `integration_plan` von der tatsächlichen `motion` mit geprüfter Position und Kontaktinformationen.

Bei abwärts gerichteter Ankunft auf einer begehbaren Fläche wird Bodenunterstützung geprüft und gegebenenfalls nach unten angebunden. Die Schräge darf dabei die projizierte Geschwindigkeit entlang der Fläche nach oben richten; ein bestätigter abwärts entstandener Bodenkontakt wird trotzdem als Landung berücksichtigt. Z-Geschwindigkeit wird auf0 gesetzt. Eine Decke begrenzt die Aufwärtsbewegung und erzeugt allein keine Bodenunterstützung.

Vorausprobe, Gleiten, Geschwindigkeitsprojektion, Landungsentscheidung und Bodenbindung sind eigene Diagnoseverfahren. Native Zylinder-/Traceflags0x286 und vollständige Actor-Filter, Hit-Daten, Zeitrückgabe nach Kollision, Landed/ModifyVelocity/NotifyJumpApex, Zwei-Wand-Korrektur und automatischer Übergang zu Walking sind nicht rekonstruiert. Die Queries gelten für den Level-Weltmodel und die klassifizierten statischen Mesh-Actors, nicht für die vollständige dynamische Engine.

Der Adapter unterstützt nur einen Substep bis zum ursprünglichen0,05-f32-Wert, vertikale Schwerkraft nach unten oder0 und ZoneVelocity0. Längere Ticks, Aufwärts-/seitliche Schwerkraft und ZoneVelocity werden abgewiesen. Die isolierte freie Rechnung besitzt einen größeren Funktionsumfang; mehrteilige kollisionsbehaftete Ticks und Volume-Grenzübertritte innerhalb eines Ticks fehlen hier noch. Wasser wird bereits von der ausgewählten Volume-Physik abgewiesen.

## Tests vom 2026-10-06

90 Workspace-Tests, Clippy über alle Targets und Formatprüfung bestehen. Fünf neue Tests prüfen:

- Exakte Übereinstimmung mit der freien Rechnung in einer leeren Welt.
- Reale Vorausprobe gegen eine Wand, Abschaltung der Luftsteuerung und Begrenzung vorhandener Geschwindigkeit beim Wandkontakt.
- Bodenlandung und anschließende vertikale Stabilität sowie Deckenkontakt ohne falsche Landung.
- Landung auf einer begehbaren Schräge trotz nach oben gerichteter Kontaktprojektion.
- Explizite Fehler für fehlende Geometrie, eindringende Ausgangslage, zu lange Ticks und nicht unterstützte Einstellungen; Eingabezustand bleibt unverändert.

## Originalkartenproben

Die Probe setzt die anfängliche Z-Geschwindigkeit auf den aufgelösten JumpZ-Wert475. Dies ist eine Diagnoseanfangsbedingung, kein durch Script ausgelöster Sprung. Sie verwendet30Ticks diagonale Beschleunigung bei Tick10 bis39, ansonsten keine Beschleunigung, und endet an der ersten bestätigten Bodenunterstützung. Der erste Startpunkt und gegebenenfalls ein weiterer mit anderer Schwerkraft werden ausgewählt. Budgets sind300 beziehungsweise720Ticks bei1/60Sekunde. Die Probe simuliert kein anschließendes Abbremsen; horizontale Geschwindigkeit darf bei der Landung verbleiben.

Android-Prüfung auf Nutzerwunsch zum Schluss; kein APK-Build, keine JNI-Verbindung und kein Emulatorlauf in diesem Schritt.

## Ergebnisse der Originalkartenproben

Drei Flugbahnen über insgesamt672Ticks enden ohne Fehler mit bestätigter Bodenunterstützung und Z-Geschwindigkeit0:

| Karte / Start | Schritte bis Bodenunterstützung | Volume / GravityZ |
|---|---:|---|
| geo_01a / PlayerStart0 | 94 | DefaultPhysicsVolume / -1100 |
| dm_hangar / PlayerStart8 | 54 | DefaultPhysicsVolume / -1100 |
| dm_hangar / PlayerStart44 | 524 | PhysicsVolume5 / -110 |

Alle Volume- und Vorausproben sind vollständig, keine Volume-Importfehler. Die ausgewählten Volumes bleiben auf diesen drei Wegen unverändert. Maximaltempo in XY beträgt etwa189,2; bei der Landung besteht weiterhin horizontale Geschwindigkeit. Die ersten beiden Bahnen haben je einen Bewegungs-Bodenkontakt. Die Bahn mit geringer Schwerkraft endet durch die kurze Bodenunterstützungsprobe und Anbindung, ohne Kontakt in der vorangehenden Bewegungsabfrage. Dies ist ein diagnostischer Landungsnachweis und kein bestätigtes natives Landed-Ereignis.

Keine der Originalkarten-Vorausproben wird auf diesen Wegen blockiert. Der blockierte Zweig und die Schrägenlandung sind durch die gezielten Geometrietests geprüft. Weitere Originalwege mit Wänden, bewegten Actors und komplexen Übergängen stehen aus.

Die bestehenden Controller-Proben bleiben Frame für Frame identisch:300Schritte auf geo_01a und2040 auf dm_hangar gegenüber den früheren Volume-Berichten. Berichte: `analysis/collision/geo_01a-falling-diagnostic.json`, `dm_hangar-falling-diagnostic.json`; Zusammenfassung `falling-collision-validation.json` im selben Ordner sowie `analysis/evidence.json`.

## Folgearbeit: gemeinsame Bewegungsdiagnose

Walking und Falling sind nun über eine eigene Tick-Steuerung mit Volume-Auswahl und Sprung-Latch verbunden. 1.906Originalkarten-Ticks mit insgesamt sechs Sprüngen, anschließendem Bremsen und stabiler Ruhe geprüft; 94Tests/Clippy bestehen. Native Script-Ereignisse, Restzeit-Übergänge und JNI bleiben offen. [Ergebnisse und Grenzen](PAWN_MOTION.md). Android-Prüfung weiterhin zum Schluss.
