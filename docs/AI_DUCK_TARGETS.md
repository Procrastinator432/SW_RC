# Zielsuche nach Duckdurchgängen

`rc-mesh-probe <GameData> <map.ctm> <report.json> --ai-duck-target-diagnostic` erweitert die bisherige KI-Kontaktprobe. Die grundlegende Probe mit acht Richtungen bleibt verfügbar. Der neue Modus prüft32Yaw-Richtungen in2048-Schritten über1500Einheiten. Die Bewegung hat maximal300Vorwärts-Ticks und anschließend90Idle-Ticks bei dt1/60.

Pro Richtung darf nur der erste geometrische Treffer Kandidat werden. Walkable-/Decken-/nicht entgegen der Bewegung gerichtete Treffer werden verworfen; hinter diesen Treffern liegende Wände werden nicht als erreichbare Ziele behandelt. Der eigene Solver schätzt die Kontaktposition aus Hit.Fraction minus Skin/Approach und berechnet dort einen separaten CanCrouchWalk-Report entlang einer Radiuslänge. Eine Armed-Vorhersage wird vor blockierten Kandidaten bevorzugt; innerhalb der gleichen Gruppe entscheidet die geringere Distanz. Gibt es keine Armed-Vorhersage, bleibt ein blockierter Kontakt als Diagnoseziel möglich.

Die Vorhersage führt keine Bewegung aus und aktiviert keinen Timer im laufenden Körperzustand. Sie verändert weder Startposition noch Physics. Ihre Höhenannahme ist die horizontale Ausgangshöhe, ohne Bodenverlauf/StepUp/Navigation. Erst die tatsächliche AI-Kontaktweiterleitung während der anschließenden Bewegung darf als Kontakt- oder Duckaktivierung gezählt werden. Vorhersage, ausgewähltes Ziel und sämtliche Bewegungs-/Dispatchframes sind getrennt protokolliert.

Controller-/Wall-Snapshots bleiben die ausdrücklich synthetischen Vorgaben der bisherigen Diagnose (nonhuman, Controller vorhanden, MinHitWall0, Excluded/Notifyfalse). Original-Spielermaße und aufgelöste Ratios sind keine rekonstruierte konkrete KI-Klasse. Native Kontaktzeitpunkte, dynamische Actors, Navigation und Callbackantworten fehlen weiterhin. Ein fehlendes Ziel, ausbleibender Kontakt oder abweichende Vorhersage sind ausgewiesene Ergebnisse und kein erfolgreicher Ducknachweis.

147 Workspace-Tests, Clippy und Formatprüfung bestehen. Die Erweiterung betrifft die CLI-Zielauswahl und Budgets; die Physics-/Kontaktarithmetik bleibt unverändert. Berichte: analysis/collision/*-ai-duck-target.json. Prüfung: `scripts/Validate-AiContact.py --duck-targets` gegen die vollständigen bisherigen Scriptberichte; Zusammenfassung ai-duck-target-validation.json. Android weiterhin zum Schluss.

## Gemessene Ergebnisse

424 neue Diagnose-Ticks, 3 tatsächliche Kontaktweiterleitungen, 0 tatsächliche Timeraktivierungen und 0 Armed-Suchvorhersagen.1906Script- und2340Legacy-Ticks samt sämtlichen bisherigen Berichtsfeldern unverändert.

| Karte | Start | Ticks | Outcome | Suchprognosen | Ausgeführte Entscheidungen | Größenwechsel |
|---|---|---:|---|---|---|---:|
| geo_01a | PlayerStart0 | 189 | ContactStanding | ShapeBlocked:31 | Unresolved:1 | 0 |
| dm_hangar | PlayerStart8 | 118 | ContactStanding | ShapeBlocked:25 | Unresolved:1 | 0 |
| dm_hangar | PlayerStart44 | 117 | ContactStanding | ShapeBlocked:9 | Unresolved:1 | 0 |

Die erweiterten Originalkartenläufe belegen weiterhin keine erfolgreiche Duckaktivierung. Die ausgewählten Wege bleiben blockiert. Die synthetischen Integrationstests sind weiterhin der Nachweis für erfolgreiche Kontaktaktivierung/Duckbewegung/Timerablauf. Die Suche ist auf diese drei Endpunkte und32horizontale Richtungen beschränkt; daraus folgt keine Aussage über sämtliche Originalkarten-Duckstellen.

## Folgearbeit: native Wandklassen

Der bisher unspezifische native Klassenvergleich ist als APawn/Unterklassen-Ausschluss identifiziert. Die CLI-Diagnose leitet ihn jetzt aus den tatsächlichen statischen Actor-Klassen und der Originalhierarchie ab; Notifyfalse bleibt Diagnosevorgabe und World.BSP eine explizite statische Weltpolicy. Controller.IsAPlayerController-Vtableslots der nativen Basisklassen direkt ausPE bestätigt, abgeleitete Runtime-Overrides weiterhin Snapshot. 742 statische Actor-Klassenbindungen geprüft;424KI-Diagnose-Ticks und sämtliche bisherigen Reportwerte unverändert, ausgenommen zusätzliche Klassenmetadaten und Scopebeschreibung.148Tests bestehen. Siehe [HIT_WALL_METADATA.md](HIT_WALL_METADATA.md).
