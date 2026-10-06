# PhysicsVolume-Auswahl und Bereichsschwerkraft

Die native Auswahl wurde aus engine.dll rekonstruiert: `ALevelInfo::GetPhysicsVolume`1043e6e0, `AVolume::Encompasses`1048df90, `execEncompasses`104ed620 und `GetDefaultPhysicsVolume`1043e5c0. Exporte: `analysis/decompiled/physics-volume-selection.c` und `default-physics-volume.c`. Der Standard-PhysicsVolume wird aus Engine.DefaultPhysicsVolume erzeugt und erhält zur Laufzeit Priorität -1.000.000. Höhere Priorität verdrängt die aktuelle Auswahl; gleiche Priorität behält die erste Auswahl. Native Sonderpfade für skeletal bases, Touch-Liste und räumlichen Hash bleiben außerhalb des statischen Diagnosemodells.

Encompasses prüft den Mittelpunkt des Actors über den Brush mit Ausdehnung null. Ein fehlender Brush bedeutet keine Zugehörigkeit. Der PointCheck-Pfad (105563f0, bereits `collision-checks.c`) wählt bei exakter Ebenengleichheit die Rückseite. CSG hängt zusätzlich an VertexCount und den beim Laden maskierten Node-Flags. Die neue Volume-Prüfung berücksichtigt diese Fälle getrennt vom bisherigen generischen Punktdiagnostikpfad. Actor-Transformationen einschließlich negativem Maßstab werden berücksichtigt; native Float-/Matrix-Parität ist nicht vollständig bestätigt.

`VolumeWorld::select` prüft die klassifizierten Volumes und liefert Einstellungen, enthaltene Kandidaten, Gleichstände und Fehler. Gleichstände zwischen speziellen Volumes bleiben unvollständig: Exportreihenfolge beweist nicht die native Touch-/Hash-Reihenfolge. Bei einer Gleichheit mit dem Standardvolume ist hingegen die Standardauswahl bekannt, da sie initial gesetzt wird. Eine geometrisch unlesbare Volume-Form darf keine scheinbar vollständige Standardauswahl liefern.

`VolumeWorld::control_tick` kann die Auswahl vor jedem PC-Steuerungsschritt neu ermitteln und Gravity/TerminalVelocity an die vorhandene Integration weitergeben. Unvollständige Auswahl liefert keinen neuen Zustand. Unterstützt ist vertikale Schwerkraft nach unten oder null. Wasser, Aufwärtsschwerkraft, horizontale Schwerkraft und nicht-null ZoneVelocity werden abgewiesen. GroundFriction wird erhalten, aber noch nicht nativ simuliert. Volume-Ereignisse, Wasserphysik, Schädigung, bewegte Volumes und Crossing-Substeps fehlen. Auswahl am Schrittanfang bedeutet, dass ein Grenzübertritt erst vor dem nächsten Tick berücksichtigt wird.

## Nachweise vom 2026-10-06

73 Workspace-Tests und Clippy über alle Targets bestehen. Drei neue Tests prüfen Standardpriorität, höhere Priorität, explizite Gleichstände, Fehler statt stiller Standardauswahl, Actor-Mittelpunkt, Ebenengleichheit, CSG mit VertexCount0, transformierte Brush-Geometrie und Anwendung der ausgewählten Schwerkraft auf die PC-Steuerung. Wasser/fehlende Geometrie liefern keinen Zustandskandidaten.

Das neue Werkzeug `rc-volume-check` lädt alle79 Originalkarten, klassifiziert421 PhysicsVolume-Actors und prüft755 PlayerStart-Mittelpunkte. Keine Lesefehler an den klassifizierten Actor-Einstellungen/Brushes. Ergebnisse:

| Auswahl | Startpunkte |
|---|---:|
| Standardvolume, vollständig | 729 |
| spezielles Volume, eindeutig | 24 |
| gleiche Priorität, Laufzeitreihenfolge unbekannt | 2 |

Die beiden unvollständigen Startpunkte sind ras_02e.PlayerStart1 (PhysicsVolume1/6/0/4, jeweils Priorität0) und yyy_35a.PlayerStart0 (PhysicsVolume0/1, Priorität0). Es wird dort keine angewandte PC-Physikoption als bestätigt ausgegeben. Deshalb beendet der Gesamtaudit sich absichtlich mit Exitcode1; dies ist keine vollständige Erfolgsmeldung.

Sechs Startpunkte in dm_hangar liegen im PhysicsVolume5 mit Gravity[0,0,-110]: PlayerStart44/48/47/49/11/33. Für alle753 vollständigen Auswahlen lassen sich die vertikalen PC-Parameter anwenden. Über alle klassifizierten Volumes existieren außerdem vier Wasserbereiche und drei Bereiche mit positiver Z-Schwerkraft; insgesamt sieben Einstellungen passen nicht zur derzeitigen PC-Physik. Weitere Volumes haben Z-Schwerkraft -50, -100 oder0. Diese werden auditiert, sind aber nicht automatisch durch die Startpunktprobe als Gameplay geprüft.

Bericht: `analysis/collision/volume-validation.json`; Zusammenfassung in `analysis/evidence.json`. Unbekannte Native-Klassen werden gesondert erfasst (198 Engine.ConvexVolume-Datenexporte). Der Audit gilt für klassifizierte gespeicherte PhysicsVolume-Actors, nicht für sämtliche dynamisch erzeugten Runtime-Objekte oder die vollständige Engine.

Die synthetische Adapterprüfung bestätigt die Anwendung einer gewählten Schwerkraft. Der Originalkorpus bestätigt Auswahl und Parameteranwendung an Startpunkten; vollständige kombinierte Originalkarten-Bewegungstrajektorien mit diesem Adapter sind noch nicht geprüft. Der ältere Mesh-/Controller-Probelauf verwendet weiterhin sein Basisprofil. JNI, Snapshot-Import und Android-Steuerung verwenden die neue Volume-Auswahl noch nicht. Android-Prüfung weiterhin auf Nutzerwunsch zum Schluss; APK und Emulator unverändert.

## Folgeprüfung: kombinierte Bewegung

Der gemeinsame Volume-Lader und die Auswahl vor jedem Controller-Schritt sind inzwischen in Originalkarten-Trajektorien verbunden. 2.340 Schritte an fünf Starts, lange Sprünge bei geringer Schwerkraft und Bereichseintritt/-austritt in geo_01a geprüft; 74 Tests/Clippy bestehen. Der oben beschriebene Stand mit nur synthetischem Adapter ist damit erweitert. [Ergebnisse und Grenzen](VOLUME_MOTION.md). Android-Prüfung weiterhin zum Schluss.
