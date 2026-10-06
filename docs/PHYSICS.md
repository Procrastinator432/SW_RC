# PC-Schwerkraft und Bodenbindung

`rc-package::physics::body_tick` integriert einen begrenzten Zeitschritt und liefert einen neuen Körperzustand, ohne den Eingabezustand zu verändern. Position, Geschwindigkeit und Bodenstatus bilden den Zustand; die Bodenlage wird aus Kollisionsabfragen neu ermittelt. Ein vom Aufrufer gesetztes `grounded` wird nicht als Kollisionsbeweis übernommen.

Zunächst wird die horizontale Verschiebung gegen die Hüllen begrenzt. Nach dieser Bewegung prüft der Solver die Bodenunterstützung: Verlässt der Körper eine Kante, wirkt bereits in diesem Schritt die Schwerkraft. Ohne Unterstützung wird die vertikale Geschwindigkeit mit einer semi-impliziten Integration aktualisiert und nach unten begrenzt. Die vertikale Verschiebung wird separat gegen die vollständige Körperausdehnung geprüft. Decken und andere Kontakte begrenzen die Geschwindigkeit. Eine nahe begehbare Bodenfläche bindet den Körper mit einer ausschließlich abwärts gerichteten Korrektur und einem konfigurierbaren Abstand; die neue Position wird nochmals auf Eindringen geprüft.

Zeitschritte müssen größer als null und höchstens 0,05 Sekunden sein. Der Aufrufer muss größere Zeitabschnitte aufteilen. Fehler, anfängliches Eindringen oder ein ausgeschöpftes Bewegungsiterationslimit liefern keinen neuen Zustand. Es gibt keine teilweise bestätigte Bewegung. Die Eingabegeschwindigkeit wird vom Aufrufer bereitgestellt; Beschleunigung durch Steuerung, Reibung, Sprünge und Schrittüberwindung fehlen.

## Nachweise vom 2026-10-06

62 Workspace-Tests und Clippy über alle Targets bestehen. Drei neue Tests prüfen Fallen/Landen auf dünner Geometrie, dauerhaft stabilen Bodenstand, horizontale Bewegung nach Landung, sofortiges Fallen beim Kantenabgang, Deckenberührung, maximale Fallgeschwindigkeit, ungültige Zeitintervalle und fehlende Kollisionsdaten ohne Veränderung des Eingabezustands.

Originalkarten wurden mit 180 Schritten je Startpunkt geprüft: 120 Schritte Fallen/Ruhe, 30 Schritte horizontaler Geschwindigkeitsvorgabe und 30 Schritte Ruhe. Feste Schrittweite 1/60 Sekunde, AABB [40,40,84], Schwerkraft 980, maximale Fallgeschwindigkeit 4000, Sicherheitsabstand 0,5, Bodenabfrage 2 Einheiten und minimale Normalen-Z-Komponente 0,7. Dies sind selbst gewählte Diagnoseparameter; die ursprünglichen Defaults oder die native Integrationsregel wurden dafür nicht rekonstruiert.

| Karte | Geprüfte Startpunkte | Schritte | Erster Bodenstatus | Höhendrift in abschließender Ruhephase |
|---|---:|---:|---|---:|
| geo_01a | 1 | 180 | Schritt 67 | 0 |
| entry | 1 | 180 | Schritt 1 | 0 |
| ctf_hangar | erste 4 von 40 | 720 | Schritt 12 | 0 |

Alle sechs Abläufe enden auf dem Boden mit Geschwindigkeit null, ohne Query-Fehler. In geo_01a endet die horizontale Bewegung 60 Einheiten neben der Ausgangs-X-Koordinate; die Körpermitte bleibt bei Z=-2603,49951171875. entry ist bereits zu Beginn bodenberührend. Die Prüfung umfasst insgesamt 1.080 Schritte, keine vollständige Karten- oder Gameplay-Abdeckung. Die Ergebnisbedingungen wurden zusätzlich anhand der gespeicherten Frames geprüft.

Berichte: `analysis/collision/*-physics-probes.json`; Zusammenfassung: `analysis/collision/physics-validation.json` und `analysis/evidence.json`. Frühere Platzierungs- und Bewegungsberichte bleiben historische Nachweise.

Offen sind native Walking/Falling-Parität, volumenabhängige Schwerkraft, Reibung, Beschleunigung, Sprünge, Stufensteigen, dynamische Geometrie und Spawn-Auswahl. Schrägflächen sind noch nicht als vollständiges Laufmodell geprüft. Die Trennung horizontaler und vertikaler Bewegung ist eine eigene Integrationsentscheidung und darf nicht als Originalalgorithmus dargestellt werden. JNI, Snapshots und Android-Steuerung verwenden die PC-Physik noch nicht. Die Android-Prüfung bleibt auf Nutzerwunsch bis zum Schluss verschoben; APK und Emulator bleiben unverändert.


Aktualisierung: Konservatives Stufensteigen ist jetzt als eigene PC-Regel umgesetzt. Die frühere Aufzählung offener Stufenlogik beschreibt den Stand vor dieser Erweiterung. Native Parameter-/Algorithmusparität bleibt offen. [Details und neue Nachweise](STEP_UP.md).


Aktualisierung: Eine eigene PC-Eingaberegel mit Beschleunigung, Bremsen und Sprungsteuerung liegt jetzt vor. Die native Verwendung der aufgelösten Bewegungswerte bleibt offen. [Details](CONTROLLER.md).
