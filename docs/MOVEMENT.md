# PC-Bewegung und PlatzierungsprÃ¼fung

`rc-package::movement` begrenzt Verschiebungen gegen die vorhandenen Welt- und transformierten Mesh-HÃ¼llen. Kontaktabstand, Gleiten, erneute Anwendung frÃ¼herer Kontaktebenen und begrenzte Iterationen sind implementiert. Eine Bodenprobe bewertet den ersten AbwÃ¤rtskontakt anhand seiner Normalen. Noch keine rekonstruierte Pawn-Physik, Schwerkraft, Stufen oder Android-Integration.

## BerÃ¼hrung und Eindringen

`body_placement` klassifiziert eine vorgeschlagene Position als `Free`, `Touching` oder `Penetrating`. Die Position wird dabei nicht verÃ¤ndert. Die ursprÃ¼ngliche geschlossene `sweep`-Abfrage zÃ¤hlt weiterhin jeden Kontakt einschlieÃŸlich BerÃ¼hrung. Ihr `start_overlapping` beweist deshalb kein tatsÃ¤chliches Eindringen.

`sweep_motion` verwendet eine eigene mathematische Kontaktregel: Eine trennende Ebene bei bloÃŸer BerÃ¼hrung und tangentialer oder auswÃ¤rts gerichteter Bewegung schlieÃŸt den betreffenden konvexen KÃ¶rper als Hindernis aus. Andere HÃ¼llen werden trotzdem geprÃ¼ft. EinwÃ¤rtsbewegung bleibt blockiert und bekommt auch bei Kontaktzeit null eine Normale. Das erlaubt seitliche Bewegung auf dem Boden, Abheben und Bodenproben aus einer berÃ¼hrenden Position. Die eigene f64-Grenztoleranz betrÃ¤gt 1e-9; native ToleranzparitÃ¤t ist nicht nachgewiesen.

`slide_body` erlaubt BerÃ¼hrung am Start. TatsÃ¤chliches Eindringen fÃ¼hrt weiterhin zu `InitialContact` ohne Bewegung. Fehler oder unvollstÃ¤ndige Daten liefern `Err` ohne Kandidatenposition. `Stopped` und `IterationLimit` mÃ¼ssen berÃ¼cksichtigt werden. `Finished` bedeutet abgearbeitete Restbewegung einschlieÃŸlich Projektion, nicht zwingend Erreichen des angeforderten Endpunkts. SchrÃ¤ges Gleiten kann ohne Schwerkraft eine vertikale Komponente erzeugen.

## OriginalkartenprÃ¼fung (2026-10-06)

59 Workspace-Tests und Clippy Ã¼ber alle Targets bestehen. Neue FÃ¤lle prÃ¼fen BodenberÃ¼hrung mit seitlicher, abhebender und einwÃ¤rts gerichteter Bewegung; zusÃ¤tzliche Wand hinter berÃ¼hrtem Boden; Ã¼berlappende zweite Geometrie; geneigten Boden und Steigungsgrenze; Abgang an einer konvexen Ecke bei unverÃ¤nderter geschlossener Sweep-Semantik.

Der Importer prÃ¼ft vier PositionsvorschlÃ¤ge pro Startpunkt (Z-Versatz 0/16/84/128), drei Verschiebungen pro Originalstart und eine horizontale Bewegung von 128 Einheiten aus exaktem Bodenkontakt. Die VersÃ¤tze sind reine DiagnosevorschlÃ¤ge. Es gibt keine automatische Spawn-Auswahl oder Teleportation. KÃ¶rper [40,40,84], Sicherheitsabstand 0,5 und maximal acht Bewegungsiterationen bleiben eigene Diagnoseparameter.

| Karte | PositionsvorschlÃ¤ge | Klassifikation | Bodenpositionen | Horizontale Bewegung am Boden |
|---|---:|---|---:|---|
| geo_01a | 4 | 4 frei | 1 | 1 Finished |
| entry | 4 | 2 frei, 1 berÃ¼hrend, 1 eindringend | 1 | 1 Finished |
| ctf_hangar | 160 | 160 frei | 40 | 39 Finished, 1 Stopped |

Alle 42 Bodenpositionen sind als `Touching` klassifiziert. Keine Query-Fehler in den ausgefÃ¼hrten Platzierungs-, Bewegungs- und Bodenproben. Die 126 ursprÃ¼nglichen Verschiebungen liefern geo_01a 2 Finished/1 Stopped, entry 3 Stopped und ctf_hangar 33 Finished/87 Stopped. Die Berichte liegen unter `analysis/collision/*-placement-probes.json`; Ã¤ltere `*-movement-probes.json` bleiben historische Nachweise der strengeren Ausgangsregel.

Korrektur des frÃ¼heren entry-Befunds: Der gespeicherte KÃ¶rper bei [-64,0,-44] berÃ¼hrt Geometrie, dringt aber nicht ein. Die alte Diagnose fasste beides unter `start_overlapping` zusammen. Z+16 und Z+84 sind frei, Z+128 dringt ein; bloÃŸes Anheben ist daher keine allgemeine PlatzierungslÃ¶sung.

Offen: native Spawn-Auswahl und Freistellen eindringender StartkÃ¶rper, Bodenbindung, Schwerkraft, Stufen, bewegte Actors, weitere Kollisionsformen und native Toleranzen. Eine freie vorgeschlagene Position allein bestÃ¤tigt keinen Gameplay-Spawn. JNI, Snapshots und Android-Steuerung verwenden diesen Solver noch nicht. APK und Emulator bleiben unverÃ¤ndert; die Android-PrÃ¼fung erfolgt auf Nutzerwunsch zum Schluss.


Aktualisierung: Schwerkraft und Bodenbindung sind jetzt als eigene PC-Integration umgesetzt und separat geprüft. Die oben genannten Einschränkungen zu fehlender Schwerkraft beschreiben den Stand vor dieser Erweiterung; native Physikparität bleibt offen. Siehe [PHYSICS.md](PHYSICS.md).
