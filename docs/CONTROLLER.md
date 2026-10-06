# PC-Bewegungssteuerung und originale Bewegungswerte

`rc-package::controller::control_tick` setzt Bewegungseingaben in Geschwindigkeiten um und ruft die vorhandene Körperphysik auf. Der Zustand enthält den Körper und die vorherige Sprungtastenlage. Ein Fehler erzeugt keinen neuen Zustand und verbraucht keine Eingabeflanke im übergebenen Zustand.

Die Richtung verwendet Welt-X/Y, einzelne Komponenten zwischen -1 und +1. Diagonalen werden auf Länge eins begrenzt; kleinere analoge Eingaben behalten ihre Stärke. Die horizontale Geschwindigkeit nähert sich der Zielgeschwindigkeit mit begrenzter Beschleunigung. Ohne Richtung bremst der Körper bei Bodenunterstützung bis null; in der Luft behält er seine horizontale Trägheit. Eingaben in der Luft verwenden einen eigenen Steuerungsfaktor. Die horizontale Geschwindigkeit ist zusätzlich begrenzt.

Ein Sprung beginnt nur bei neu gedrückter Taste und durch Kollisionsabfrage bestätigter Bodenunterstützung, ohne bereits aufwärts gerichtete Geschwindigkeit. Ein behaupteter `grounded`-Status genügt nicht. Gedrückthalten über eine Landung hinweg löst keinen weiteren Sprung aus. Eine in der Luft erfolglos gedrückte Taste muss für einen späteren Sprung erneut losgelassen und gedrückt werden. Decken und Landung werden von der Körperphysik behandelt.

67 Workspace-Tests und Clippy über alle Targets bestehen. Drei neue Tests prüfen Beschleunigung, diagonale Geschwindigkeitsgrenze, analoge Eingabe, Bremsen, Luftsteuerung/Trägheit, Sprungflanke, Bodenpflicht, keine Wiederholung beim Gedrückthalten und ungültige Eingaben/fehlende Geometrie ohne Änderung des ursprünglichen Zustands.

## Originalkartenprüfung

geo_01a und entry mit jeweils einem Startpunkt sowie ctf_hangar mit den ersten zwei von 40 Startpunkten: je 300 feste Schritte bei 1/60 Sekunde, insgesamt 1.200. Ablauf: 120 Schritte Fallen/Ruhe, 30 Schritte diagonale Eingabe, 30 Schritte Bremsen, 90 Schritte gehaltene Sprungtaste, 30 Schritte gelöste Taste/Ruhe. Diagnosewerte: Geschwindigkeit180, Beschleunigung600, Bremsrate900, Luftfaktor0,2, Sprunggeschwindigkeit420; Körperphysik bleibt g980/Terminal4000/Schritt24.

Alle vier Abläufe haben genau einen angenommenen Sprung, maximal180 horizontale Geschwindigkeit, keine Abfragefehler und enden mit Bodenunterstützung und Geschwindigkeit null. In der abschließenden Ruhephase beträgt die Höhendrift null. Die Ergebnisbedingungen wurden anhand der gespeicherten Frames geprüft. Berichte: `analysis/collision/*-controller-probes.json`, Zusammenfassung `analysis/collision/controller-validation.json`.

## Aufgelöste Originalwerte

Das neue Werkzeug `rc-controller-defaults` löst zwölf Eigenschaften aus Klassenvererbung und Properties-Overrides auf und erhält Ursprung, Paket und Payload-Offset. Alle zwölf sind aufgelöst. Bericht: `analysis/collision/controller-defaults.json`.

| Eigenschaft | Wert | Herkunft |
|---|---:|---|
| PlayerCommando.GroundSpeed | 450 | CloneCommando-Klassendelta |
| AccelRate | 1024 | CloneCommando-Klassendelta |
| DecelRate | 600 | Pawn-Klassendelta |
| AirControl | ca. 0,35 | Pawn-Klassendelta |
| JumpZ | 475 | CloneCommando-Klassendelta |
| MaxFallSpeed | 1200 | Pawn-Klassendelta |
| WalkSpeedRatio | 0,5 | Pawn-Klassendelta |
| BackSpeedRatio | ca. 0,8 | Properties.PlayerCommandoDefaults |
| SideSpeedRatio | ca. 0,95 | Properties.PlayerCommandoDefaults |
| PhysicsVolume.Gravity | [0,0,-1100] | PhysicsVolume-Klassendelta |
| TerminalVelocity | 12000 | PhysicsVolume-Klassendelta |
| GroundFriction | 8 | PhysicsVolume-Klassendelta |

Außerdem enthält der wiederhergestellte Actor-Quelltext die Konstanten MAXSTEPHEIGHT=35 und MINFLOORZ=0,7 (`analysis/reports/sources/System__engine.u/Actor.uc`, Zeilen657/658). Pawn.DoJump beschreibt die Bindung an Bewegungszustände und die Verwendung von JumpZ. Das bestätigt die Bedeutung der Originaleigenschaften, aber keine vollständige Parität des eigenen Controllers. MaxFallSpeed ist laut Pawn-Quelltext eine sichere Landegeschwindigkeit und eine Pfadgrenze, kein Ersatz für die TerminalVelocity des Volumens.

Die Originalwerte werden in diesem Arbeitsschritt separat auditiert und noch nicht automatisch auf den Diagnosecontroller angewendet. Aktive PhysicsVolumes, Instance-/Runtime-Änderungen und die genaue native Verwendung von Beschleunigung, Bremsrate und Bewegungsfaktoren fehlen. Camera-relative Eingabe, Geh-/Renn-/Rückwärts-/Seitwärtsfaktoren, Crouch/Ladder/Spider-Zustände und Sprunganimationen sind noch nicht implementiert. Die begrenzte Geschwindigkeitsannäherung ist eine eigene PC-Regel, keine dekompilierte Originalformel.

JNI, Snapshots und Android-Steuerung verwenden den Controller noch nicht. APK und Emulator unverändert; Android-Prüfung weiterhin auf Nutzerwunsch zum Schluss.


Aktualisierung: Die aufgelösten Bewegungswerte werden jetzt im neuen PC-Originalprofil angewendet; Yaw-relative Eingabe und Richtungs-/Gehfaktoren ergänzt. Die oben beschriebene Trennung zwischen Audits und alten Diagnoseparametern gilt für die historischen Controller-Berichte. [Aktueller Stand](MOVEMENT_PROFILE.md).
