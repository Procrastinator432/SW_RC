# Konservatives Stufensteigen am PC

`PhysicsOptions::step_height` aktiviert einen separaten Aufwärts-/Vorwärts-/Abwärtsversuch in `body_tick`; null schaltet ihn ab. Der Versuch beginnt nur bei nachgewiesener Bodenunterstützung, ohne aufwärts gerichtete Geschwindigkeit und nach einem horizontalen Hinderniskontakt. Der vom Aufrufer gesetzte Bodenstatus genügt nicht.

Der vollständige Körper muss den Aufwärtsweg frei durchlaufen können. Danach wird die horizontale Verschiebung in angehobener Lage geprüft. Nur ein besserer Fortschritt in der angeforderten Richtung rechtfertigt den Versuch. Eine vollständige Abwärtsabfrage muss eine begehbare Fläche finden. Die Höhendifferenz wird zwischen den beiden Boden-Kontaktlagen gemessen, sodass ein vorhandener Körperabstand zur Ausgangsfläche nicht die zulässige Stufenhöhe vergrößert. Eigene Toleranzen: minimaler positiver Höhengewinn 1e-4, obere Höhengrenze mit 1e-8 Zuschlag. Die endgültige Position wird nochmals auf Eindringen geprüft.

Ein geometrisch abgelehnter Versuch fällt auf die reguläre begrenzte Bewegung zurück. Ein Query-Fehler liefert keinen neuen Körperzustand. Ausgeschöpfte Iterationen im alternativen Aufstiegsversuch verwerfen diesen Kandidaten; ein ausgeschöpfter regulärer Bewegungspfad bleibt ein Fehler. Erfolgreiche Versuche werden mit Höhengewinn und Bodenabfrage als `BodyFrame::step` protokolliert. Die Kontaktfraction im Ereignis bezieht sich auf den Abwärtstest aus angehobener Lage.

64 Workspace-Tests und Clippy über alle Targets bestehen. Zwei neue Tests prüfen wiederholbares Aufsteigen mit erhaltener Vorwärtsgeschwindigkeit, abschaltbare Stufenlogik, zu hohe Hindernisse, niedrige Decken, fehlende Landeflächen und Körper ohne Bodenunterstützung. Ein zusätzlicher Fall stellt sicher, dass ein Körperabstand zur Ausgangsfläche kein zu hohes Hindernis zulässig macht.

Dies ist eine eigene PC-Integrationsentscheidung. Die native Schrittüberwindung oder deren Parameter wurden dafür nicht rekonstruiert. Noch keine automatische Spawn-Auswahl, dynamische Geometrie, Reibung oder native Walking/Falling-Parität. Android-Prüfung weiterhin auf Nutzerwunsch zum Schluss; APK und Emulator unverändert.

## Originalkarten-Nachweise

Die gleiche statische Körperwelt wie bei den bisherigen Kollisionsprüfungen liefert in geo_01a und entry jeweils acht zusätzliche Bewegungsproben, in ctf_hangar 320. Die Proben beginnen an den 42 zuvor ermittelten Bodenpositionen und verlangen unabhängig voneinander horizontale Verschiebungen von 128 beziehungsweise 512 Einheiten in vier Richtungen, je innerhalb eines 0,05-Sekunden-Schritts. Das sind Belastungsproben mit hohen Geschwindigkeitsvorgaben, keine kontinuierliche Gameplay-Simulation. Maximale Stufenhöhe 24, Sicherheitsabstand 0,5, begehbare Normalen-Z-Komponente mindestens 0,7: eigene Diagnoseparameter.

Alle 336 Proben liefern einen Körperzustand ohne Query-Fehler. Vier Aufstiegsereignisse in ctf_hangar werden angenommen: Höhengewinn 7,7870247143 (StaticMeshActor157), 0,0027245119 (StaticMeshActor312), 21,6746334577 (StaticMeshActor531) und 7,7844002024 (StaticMeshActor146). Der sehr kleine Höhengewinn ist ein mathematisch akzeptierter Kandidat und kein Nachweis einer sichtbaren Treppenstufe. Alle Werte wurden gegen die eingestellte Höhengrenze geprüft. geo_01a und entry liefern in diesen Wegen keine Aufstiegsereignisse.

Zusätzlich bestehen die bisherigen sechs kontinuierlichen Abläufe mit insgesamt 1.080 Schritten erneut: Endzustand bodengebunden, Geschwindigkeit null, keine Höhendrift in der abschließenden Ruhephase. Diese kurzen Wege lösen keine Aufstiegsereignisse aus; erfolgreiche aufeinanderfolgende Stufen sind im synthetischen Test nachgewiesen.

Berichte: `analysis/collision/*-step-probes.json`, Zusammenfassung `analysis/collision/step-validation.json` und `analysis/evidence.json`. Keine Android-Integration oder Emulatorprüfung in diesem Arbeitsschritt.


Aktualisierung: Eine eigene PC-Eingaberegel mit Beschleunigung, Bremsen und Sprungsteuerung liegt jetzt vor. Die native Verwendung der aufgelösten Bewegungswerte bleibt offen. [Details](CONTROLLER.md).
