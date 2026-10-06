# Erste Auto-Crouch-Prüfung bei Wandkontakt

`StaticBodyWorld::hit_wall_auto_crouch_first_attempt` verbindet die geprüften Bedingungen in APawn.processHitWall104900b0 mit dem vorhandenen CanCrouchWalk-Adapter. Der Aufrufer übergibt einen tatsächlichen Wandkontakt sowie Controller-, Physics- und Event-Snapshots. Der Adapter wird noch nicht automatisch aus jedem Kontakt des Bewegungsrechners aufgerufen.

Die native Reihenfolge bleibt erhalten: kein Wallactor oder ausgeschlossene Wallklasse beendet den Zweig; DirectHitWall oder fehlender Controller überspringt die Controllerprüfung. Im Controllerzweig beendet exakt Velocity0 die Verarbeitung. Danach wird Controller.Destination minus Pawn.Location normalisiert. Bei Walking werden Zielrichtung und Wandnormale auf XY abgeflacht und nochmals normalisiert. Dot > MinHitWall beendet den Zweig, Gleichheit bleibt erlaubt. Erst danach wird das übergebene NotifyHitWall-Ergebnis geprüft. PhysicsWalking, nicht menschlich gesteuert, CanCrouch und nicht bereits Crouched erlauben den ersten CanCrouchWalk-Versuch.

Der Script-Kommentar an Controller.MinHitWall nennt Velocity.Normal. Die exportierte native Funktion benutzt dagegen Destination minus Location. Der Rust-Adapter folgt dem Funktionskörper. Das Prüfsegment beginnt bei Location und endet eine aktuelle CollisionRadius-Länge entlang der berechneten Zielrichtung. Wegen der notwendigen IsCrouched=false-Bedingung liefert das geprüfte Stehprofil den Radius. CanCrouchWalk verschiebt dieses Segment anschließend um CrouchHeight-currentHeight nach unten.

Ein erfolgreicher erster Versuch liefert Armed und den Zustand mit Wants/Try/Timer0,5; Position, Geschwindigkeit und tatsächliche Körperform bleiben erhalten. Bei Blockade liefert der Adapter RecoveryRequired samt erstem Queryreport. Er führt weder die native folgende vertikale MoveActor-Operation noch den zweiten CanCrouchWalk-Versuch aus. Die Rückgabe bedeutet deshalb keine vollständige HitWall-Verarbeitung. Die übrigen Skip-Entscheidungen beschreiben ebenfalls nur diesen Auto-Crouch-Zweig; sie ersetzen keine Actor-/Controller-HitWall-Ereignisse.

HitWallContext enthält das Ergebnis der ausgeschlossenen Klassenprüfung und NotifyHitWall ausdrücklich als Snapshots. Die konkrete Wallklasse wird nicht aus einem unspezifischen PrivateStaticClass-Symbol geraten. DirectHitWall entspricht hier dem übergebenen Ergebnis der nativen Bitprüfung; ein vollständiger nativer Propertyoffset-Abgleich steht aus. Native rsqrtss-Näherung, f32-Reihenfolge und Event-Seiteneffekte sind nicht nachgebildet. Die Diagnose verwendet f64-Normalisierung und weist nichtendliche aktive Eingaben ab. Beide CanCrouchWalk-Queries verwenden weiterhin eigene statische AABB-Hüllen ohne native Masken/Actorfilter.

135 Workspace-Tests, Clippy und Formatprüfung bestehen. Vier neue Tests prüfen:

- Zielrichtung und XY-Projektion trotz entgegengesetzter Velocity; Probe um Stehradius und unveränderte Körperform.
- Elf Skip-Bedingungen einschließlich HumanControlled, NotifyHandled, Falling, bereits Crouched und fehlendem Controller; fehlende Geometrie wird in diesen Fällen nicht abgefragt.
- Exakte Winkelgleichheit und Ziel gleich Location.
- Blockierter Erstversuch als RecoveryRequired sowie unbekannte Geometrie/ungültige Richtung als atomische Fehler.

Keine neuen Originalkartenläufe oder Android-Prüfung in diesem Schritt. Für vollständige KI-Bewegung fehlt außerdem ein Physics-Einstieg ohne die PlayerWalking-Eingabephase: Der menschliche duck0-Pfad löscht die Duck-Anforderung und darf nicht ungeprüft auf eine KI-Anforderung angewendet werden. Automatische Kontaktdispatches, native Recovery-/Callback-Reihenfolge und KI-Controllerzustände bleiben offen. Nachweis: analysis/reports/hit-wall-validation.json.

## Folgearbeit: gemeinsamer Physics-Einstieg

Ein separater Physics-Einstieg ohne PlayerWalking-Phase erhält jetzt die gespeicherte KI-Duckanforderung und den Pending-Sprungevent. Der Spielerpfad delegiert nach seiner Scriptphase an dieselbe Implementierung. Explizites HitWall-Arming bis Timerablauf/Blockade/Retry synthetisch geprüft,138Tests bestehen. Originalkarten210Duck-/1906Script-/2340Legacy-Ticks unverändert bis auf additive inaktive Timerfelder und Scopebeschreibung. Native Recovery und automatischer Kontaktdispatch bleiben offen. Siehe [CROUCH_PHYSICS.md](CROUCH_PHYSICS.md).

## Folgearbeit: Abwärtsschritt

Der berechtigte Walking/nonhuman Auto-Crouch-Zweig besitzt jetzt einen zusätzlichen Recovery-Adapter mit Abwärtsschritt und zweitem Duckversuch. Korrektur der früheren Richtungsannahme: Native Konstante1067667c ist-35, keine Aufwärtsbewegung. Eigene statische Query mit Skin und geprüfter Zielplatzierung; MoveActor-Callbacks und automatischer Kontaktdispatch weiterhin offen.143Tests bestehen. Siehe [HIT_WALL_RECOVERY.md](HIT_WALL_RECOVERY.md).

## Folgearbeit: native Wandklassen

Der bisher unspezifische native Klassenvergleich ist als APawn/Unterklassen-Ausschluss identifiziert. Die CLI-Diagnose leitet ihn jetzt aus den tatsächlichen statischen Actor-Klassen und der Originalhierarchie ab; Notifyfalse bleibt Diagnosevorgabe und World.BSP eine explizite statische Weltpolicy. Controller.IsAPlayerController-Vtableslots der nativen Basisklassen direkt ausPE bestätigt, abgeleitete Runtime-Overrides weiterhin Snapshot. 742 statische Actor-Klassenbindungen geprüft;424KI-Diagnose-Ticks und sämtliche bisherigen Reportwerte unverändert, ausgenommen zusätzliche Klassenmetadaten und Scopebeschreibung.148Tests bestehen. Siehe [HIT_WALL_METADATA.md](HIT_WALL_METADATA.md).
