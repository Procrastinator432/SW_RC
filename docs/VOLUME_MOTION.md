# PhysicsVolumes in PC-Bewegungsproben

Der Kartenimporter verwendet nun denselben Volume-Lader für den Korpusaudit und die Controller-Trajektorien. Vor jedem Schritt wählt `VolumeWorld::control_tick` das PhysicsVolume am aktuellen Körpermittelpunkt und übernimmt dessen Gravity und TerminalVelocity. Jede gespeicherte Controller-Frame enthält die Auswahl einschließlich Kandidaten, Priorität und Fehlern. Unvollständige Auswahl erzeugt keinen neuen Bewegungszustand.

## Prüfung vom 2026-10-06

74 Workspace-Tests und Clippy über alle Targets bestehen. Der neue Test `crossings_change_gravity_on_the_following_tick` bewegt einen Körper aus einem Bereich mit geringer Schwerkraft heraus und wieder hinein. Die jeweils neue Schwerkraft wirkt im folgenden Tick. Grenzübertritte werden innerhalb eines Ticks noch nicht in Teilschritte zerlegt.

Fünf Originalstartpunkte wurden über insgesamt 2.340 Controller-Schritte bei 1/60 Sekunde geprüft, jeweils mit genau einem Sprung. Alle Volume-Auswahlen sind vollständig, keine Import- oder Controller-Fehler, alle Endzustände am Boden mit Geschwindigkeit null:

| Karte / Start | Schritte | Auswahl / Schwerkraft Z |
|---|---:|---|
| dm_hangar / PlayerStart8 | 300 | DefaultPhysicsVolume / -1100 |
| dm_hangar / PlayerStart9 | 300 | DefaultPhysicsVolume / -1100 |
| dm_hangar / PlayerStart44 | 720 | PhysicsVolume5 / -110 |
| dm_hangar / PlayerStart48 | 720 | PhysicsVolume5 / -110 |
| geo_01a / PlayerStart0 | 300 | Standard → PhysicsVolume1 → Standard / jeweils -1100 |

In geo_01a erfolgt die Auswahl von PhysicsVolume1 ab Tick25 und die Rückkehr zum Standard ab Tick33 (nullbasierte Indizes). Dies bestätigt einen realen Bereichseintritt und -austritt, aber keinen Wechsel der Schwerkraft: beide Bereiche haben hier denselben Wert. Unterschiedliche Schwerkraft bei einem Grenzübertritt ist bislang synthetisch geprüft.

Die beiden Abläufe mit geringer Schwerkraft sind nach 300 Schritten noch in der Luft. Die verlängerten Proben landen bei Tick700 und bremsen anschließend ab. Die letzten 20 Frames sind am Boden; ihre Höhe ändert sich während des Abbremsens auf der Landefläche noch um etwa0,34 Einheiten. Die Endgeschwindigkeit ist null. Die übrigen drei Abläufe haben in den letzten60Frames keine Höhendrift. In dm_hangar bleiben die ausgewählten Volumes während dieser Wege gleich.

Berichte: `analysis/collision/dm_hangar-volume-motion-long.json`, `geo_01a-volume-motion.json`; Zusammenfassung: `volume-motion-validation.json` im selben Ordner und `analysis/evidence.json`. Der frühere 300-Schritte-Bericht `dm_hangar-volume-motion.json` bleibt als Nachweis des noch laufenden Flugs erhalten. Die verlängerte Probe verwendet nach dem Loslassen der Sprungtaste 450 statt30 weitere Schritte; ihre Beschreibungsmetadaten wurden entsprechend berichtigt.

Der erneute Gesamtaudit mit gemeinsamem Lader behält 79 Karten, 421 Volumes und 755 Startmittelpunkte: 753 vollständige Auswahlen, zwei unaufgelöste Prioritätsgleichstände. Exitcode1 bleibt deshalb erwartet; der Audit ist keine vollständige Erfolgsmeldung.

## Grenzen

Dies sind PC-Diagnosen mit statischer Welt- und Mesh-Kollision sowie gespeicherten Volume-Posen. Native Bewegungsformeln, GroundFriction, Wasser, Aufwärtsschwerkraft, ZoneVelocity, Volume-Ereignisse, bewegte Volumes, Laufzeitreihenfolge bei Prioritätsgleichstand und Crossing-Substeps bleiben offen. Die älteren direkten Physics-/Stresstests in den Berichten behalten ihre Diagnoseparameter; die Controller-Trajektorien verwenden Originalprofil und Volume-Auswahl. JNI, Szenen-Snapshot und Android-Steuerung sind noch nicht verbunden. Die Android-Prüfung erfolgt auf Nutzerwunsch zum Schluss; APK und Emulator wurden in diesem Arbeitsschritt nicht geprüft.
