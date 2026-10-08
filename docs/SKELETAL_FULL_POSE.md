# Verbundene vorbereitete Vollpose

Stand: 2026-10-08. `apply_full_pose_prepared` verbindet den Vollpose-Abschnitt von ApplyAnimation ab `1050a615` mit der bereits rekonstruierten Kanal-Auswertung, der Director-Hierarchie und dem Bounds-Abschluss.

`PreparedFullPose` enthält die vorherige lokale Pose, persistenten Scratch, Ergebnismatrizen und Bounds. Das Cachebyte +61 liegt ausschließlich in den Bounds und wird gemeinsam für den Einstieg und den Abschluss verwendet. Der bestehende Kanal-Einstieg bleibt kompatibel; seine Logik wurde auf einen ebenfalls nutzbaren Puffer-Einstieg verlagert.

Bei gesetztem Cachebyte außerhalb des Editors endet der Aufruf vor Pufferprüfung, Kanal-Hosts, Directors und Bounds-Publikation. Andernfalls lesen alle geeigneten Kanäle dieselbe vorherige Instanzpose und schreiben in den gemeinsamen Scratch. Erst nach erfolgreicher Kanalschleife wird der Scratch übernommen. Ohne angewandten Kanal oder bei gesetztem Blockierwort +11c wird die Referenzpose kopiert.

Die anschließende Auswertung erzeugt je Knochen die Matrix, verknüpft sie mit dem Elternknochen, wendet den ersten aktiven passenden Director an und sammelt den aktuellen Bounds-Punkt. Nach lokaler Box und Sphäre erfolgt die vom Host gewählte Weltpublikation. Erst danach werden +60/+61 auf 1 und +179 auf 0 gesetzt. Der nächste reguläre Aufruf verwendet damit den Cache; ein neuer Animationsschritt benötigt eine Invalidierung durch den Aufrufer.

Ein Kanalfehler erhält die vorherige lokale Pose und die alten Matrizen, aber bereits geschriebene Kanal-History und Scratchwerte. Ein späterer Director-/Bounds-/Publikationsfehler erhält die bereits übernommene Kanalpose sowie abgeschlossene Matrix- und History-Schreibzugriffe. Die Abschlussflags werden bei einem solchen Fehler nicht gesetzt. Der Editor umgeht das Cachegate auch bei einem zuvor gesetzten Cachebyte.

## Prüfung

`rc-full-pose-check` liest die Original-Animationstracks erneut und führt für 225 Linkups auf 130 Skeletten jeweils vier diagnostische, vom Aufrufer invalidierte Schritte aus. Jeder Schritt wird unmittelbar mit einem Cache-Aufruf wiederholt. Die 900 Posen mit 29.912 Matrizen stimmen hinsichtlich lokaler Pose, Scratch, Kanal-History und Host-Aufrufen mit dem zuvor unabhängig geprüften Originaltrack-Bericht überein.

`Record-FullPoses.py` berechnet zusätzlich jede Hierarchiematrix anhand der Original-ASM-Ausdrücke sowie jede lokale Box und Sphäre unabhängig neu. Publikationssnapshots belegen die Reihenfolge vor dem Schreiben der Abschlussflags. Die 900 Cachewiederholungen verursachen keine weiteren Kanal- oder Publikationsaufrufe. Sechs Integrationstests ergänzen Editor-/Cacheverhalten, Director-Einfluss auf Kindknochen und Teilzustände bei Fehlern. **347 Workspace-Tests**, Clippy mit `-D warnings` und Formatprüfung bestanden.

## Grenzen

Der Einstieg setzt vorbereitete Instanzpuffer, ausreichend großen Scratch, Hierarchie, MeshToWorld, Referenz-Inversen und Linkups voraus. Der Root-only-Pfad ist weiterhin separat. Die Originaltrack-Probe liefert Kanalzeiten und Invalidierungen diagnostisch, verwendet portable Quaternion-/Winkelmathematik und keine Directors oder Actor-Weltpublikation. Director-Integration ist hier durch Tests und zuvor durch eigenständige Originalpose-Diagnosen geprüft; Weltpublikation bleibt über den bereits geprüften Bounds-Host verfügbar.

Die [Instanzvorbereitung](SKELETAL_PREPARATION.md) rekonstruiert inzwischen die Pufferinhalte bei Größenänderungen und verbindet Transform-Host, Referenzcache und Linkup-Hosts in Originalreihenfolge. Der [vollständige Vollpose-Einstieg](SKELETAL_ANIMATION_ENTRY.md) verbindet jetzt auch ihre getrennten Quaternion-/Positionsarrays mit dieser Pose-Auswertung. Echte Actor-/Script-Anbindung, Root-only-Verbindung, Skinning und Renderer-Verwendung stehen weiterhin aus. Eine spielbare native Animation wird damit noch nicht behauptet. Androidprüfung am Projektende.

Nachweise: `analysis/reports/full-poses.json`, `full-poses-validation.json`, `full-pose-tests.log`, `analysis/decompiled/skeletal-root-frame.asm`, `analysis/evidence.json`. Verwandt: [Kanalstack](SKELETAL_CHANNEL_STACK.md), [Director-Bounds](SKELETAL_DIRECTED_BOUNDS.md), [Referenzcache](SKELETAL_REFERENCE_CACHE.md).
