# Native Wandklassen- und Controllerbedingungen

Die Wandklassenprüfung in processHitWall104900b0 ist eindeutig identifiziert: Assembler10490104 vergleicht die Actor-Klasse mit1075a270 und verfolgt bei Nichtübereinstimmung deren Parent an+0x30. Der Export an1075a270 heißt PrivateStaticClass@APawn. Somit überspringt dieser native Pfad **Pawn und alle Pawn-Unterklassen**. Die bisher unspezifische PrivateStaticClass-Bezeichnung darf jetzt konkret zugeordnet werden.

`WallEventSnapshot::from_actor_class` verwendet die aufgelöste serialisierte Klassenhierarchie des Catalog. Nicht-Actors und fehlende Klassen ergeben einen Fehler; Pawn-Unterklassen erhalten excluded_wall_class=true, andere bekannte Actors false. NotifyHandled bleibt ein ausdrücklich übergebenes Event-Ergebnis. Der neue Unit-Test prüft Pawn, PlayerCommando-Unterklasse, StaticMeshActor und fehlende Klasse sowie die Erhaltung eines true-Notify-Snapshots.

Die CLI-Kontaktproben führen nun für jeden klassifizierten statischen Actor dessen tatsächlichen Klassennamen mit. Die Wandklasse wird anhand dieses Namens aufgelöst, statt für alle Objekte pauschal false zu setzen. Der Bericht protokolliert Objekt, Klasse, berechneten Ausschluss und Herkunft der weiterhin synthetischen Notifyfalse-Antwort. Fehlt eine Klassenbindung, schlägt die Diagnose ausdrücklich fehl. World.BSP verwendet weiterhin die explizite Policy eines statischen Weltobjekts ohne Pawn-Klasse; eine native Laufzeit-Actorbindung des gesamten BSP-Modells wird daraus nicht abgeleitet.

APawn.IsHumanControlled10480780 prüft Controller+0x41c auf Vorhandensein und ruft dessen virtuellen Slot+0x1d0 auf. Die exportierte Controller-Vtable10658650 enthält dort1031a880 AActor.IsAPlayerController mit Bytes33c0c3 (return0). Die PlayerController-Vtable10663620 enthält103247d0 APlayerController.IsAPlayerController mit Bytesb801000000c3 (return1). Diese Slots und Funktionsbytes werden direkt aus den PE-Sektionen geprüft. Kein Controller ergibt false. Der Scriptkörper formuliert denselben gewöhnlichen Fall als PlayerController-Cast; der native Code verwendet jedoch einen virtuellen Test. Abgeleitete Klassen können diesen überschreiben, weshalb die Runtime-Human-Bedingung weiterhin als explizites Ergebnis übergeben wird. Possession/Controller-Lebenszyklus und virtuelle Aufrufausführung sind nicht implementiert.

148 Workspace-Tests, Clippy und Formatprüfung bestehen. Native Identitäts-/Slotprüfung: scripts/Verify-HitWallMetadata.py -> analysis/reports/hit-wall-class-controller-proof.json. Die Originalkartenregression vergleicht neue *-ai-wall-metadata.json gegen die vorherigen *-ai-duck-target.json und entfernt ausschließlich hinzugefügte wall_metadata und aktualisierte Scopebeschreibung. Sämtliche übrigen Werte müssen identisch sein. scripts/Validate-AiWallMetadata.py schreibt ai-wall-metadata-validation.json.

ControllerPresent, Destination, DirectHitWall, MinHitWall und Notify-Ergebnisse bleiben Zustands-/Diagnosevorgaben. Der ursprüngliche Native-Bitoffset von bDirectHitWall wird hier nicht zusätzlich als Propertylayout rekonstruiert. Native Kontaktzeitpunkte, dynamische Actorfilter, Events, KI-Navigation und Android-Anbindung bleiben offen. Android-Prüfung zum Schluss.

## Originalkarten-Ergebnis

742 statische Actor-Klassenbindungen geprüft;424KI-Diagnose-Ticks und sämtliche bisherigen Reportwerte unverändert, ausgenommen zusätzliche Klassenmetadaten und Scopebeschreibung.

| Karte | Statische Klassenbindungen | KI-Ticks | Weiterleitungen | Pawn-Ausschlüsse |
|---|---:|---:|---:|---:|
| geo_01a | 284 | 189 | 1 | 0 |
| dm_hangar | 458 | 235 | 2 | 0 |

Die hier gelieferten statischen Klassen lösen keine Pawn-Ausschlüsse aus; die native Pawn-/Unterklassenbedingung ist im neuen Unit-Test geprüft. Kontaktentscheidungen bleiben dieselben blockierten Fälle wie im vorherigen Zielbericht; kein zusätzlicher erfolgreicher Ducknachweis.
