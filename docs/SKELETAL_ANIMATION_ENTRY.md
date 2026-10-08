# Vollpose-Einstieg mit Instanzvorbereitung

Stand: 2026-10-08. `apply_animation_full` verbindet die belegte Instanzvorbereitung mit dem Vollpose-Pfad von ApplyAnimation. Der Einstieg arbeitet auf getrennten Quaternion-, Positions- und Matrixarrays und verwendet ein einziges Cachebyte in den Bounds.

## Ablauf und Speicher

`AnimationInstance` besitzt die lokalen Arrays, MeshToWorld und Bounds. Der inverse Referenzcache wird separat übergeben, da er im Original zum Mesh gehört und von mehreren Instanzen verwendet werden kann. Auch der Scratch wird separat übergeben; sein Inhalt und seine Lebensdauer liegen beim Aufrufer wie bei den ursprünglichen globalen Scratcharrays.

Zuerst werden die Instanzpuffer angepasst, MeshToWorld abgefragt, der inverse Referenzcache bei Bedarf erzeugt und die Linkups in Originalreihenfolge aktualisiert. Erst danach greift das Vollpose-Cachegate. Außerhalb des Editors überspringt ein weiterhin gesetztes +61 dann Kanäle, Scratchvorbereitung, Directors und Bounds. Der Transform und die Linkup-Aufrufe haben zu diesem Zeitpunkt bereits stattgefunden. Eine Pufferänderung oder ein leerer Referenzcache invalidiert +61 und erzwingt die Auswertung.

Bei erforderlicher Auswertung folgt das ursprüngliche Quaternion-Scratch-Gate. Die getrennten Arrays werden für die bestehenden Kanal-/Pose-Module vorübergehend in gemeinsame Transformwerte überführt. Nur gepaarte Einträge werden zurückgeschrieben; ein ungepaarter Quaternion-Anhang bleibt bitgleich erhalten. Die Rückübertragung geschieht auch bei Fehlern. Deshalb bleiben Scratch- und Kanal-History-Schreibzugriffe eines fehlgeschlagenen Kanals erhalten, während die lokale Pose erst nach erfolgreicher Kanalschleife übernommen wird. Spätere Director-/Bounds-/Publikationsfehler erhalten die bereits übernommene lokale Pose, Matrix- und History-Schreibzugriffe.

Die Puffer-Schnittstelle des bestehenden Vollpose-Moduls wurde hierfür ergänzt; dessen bisheriger Einstieg bleibt verfügbar. Hierarchie, Directors, Bounds und Abschlussflags verwenden dieselben zuvor geprüften Implementierungen. Es wird keine zweite Animationsmathematik eingeführt.

## Prüfung

`rc-animation-entry-check` liest die Original-Animationstracks erneut. 225 diagnostische Instanzläufe auf 125 Skeletten mit auflösbaren Animationen ergeben 900 Posen mit 29.912 Matrizen und jeweils einen anschließenden Cache-Aufruf. Die übrigen fünf Skelette bleiben im Bericht ohne Animationslauf; insgesamt werden dieselben 130 unterstützten Meshobjekte erfasst.

Alle lokalen Posen, Matrizen, Kanal-History, Scratchwerte, Bounds und Publikationssnapshots stimmen mit dem zuvor unabhängig anhand Originaltracks und ASM geprüften Vollpose-Bericht überein. `Record-AnimationEntry.py` prüft zusätzlich jeden Vorbereitungszustand, den einmaligen Referenzcache-Aufbau je animiertem Mesh, die Wiederverwendung über Instanzläufe hinweg und sämtliche Transform-/Linkup-Hostereignisse. Die Referenz-Inversen entsprechen dem unabhängig anhand Original-ASM geprüften Cachebericht.

Neun zusätzliche Integrationstests prüfen Kaltstart mit leeren Puffern, Invalidierung durch einen leeren Referenzcache, Vorbereitung vor Cache-Rücksprung, Editor-Modus, Director-Einfluss, Teilzustände nach Hostfehlern und einen ungepaarten Scratch-Anhang. **365 Workspace-Tests**, Clippy mit `-D warnings` und Formatprüfung bestanden.

## Grenzen

Der Einstieg bildet den Vollpose-Pfad ab. Der [Root-only-Einstieg](SKELETAL_ROOT_ENTRY.md) verwendet inzwischen dieselbe Instanzvorbereitung und überträgt die Rootwerte in dieselben getrennten Originalarrays. Die virtuelle Transformabfrage und die Linkup-Aktualisierung werden über Hosts angebunden; der Diagnosehost liefert einen Transform und protokolliert Linkup-Aufrufe. Eine konkrete Actor-/Script-Anbindung ist damit noch nicht hergestellt. Hierarchie, Kanalzeiten, Invalidierung, Padding, Actor-Skalierung und Directorzustände bleiben explizite Eingaben.

Die Originaltrack-Probe beginnt mit referenzgefüllten lokalen Arrays und Scratch, verwendet portable Quaternion-/Winkelmathematik und keine Directors oder Actor-Weltpublikation. Kaltstart, Directors und Fehlerfälle werden durch Integrationstests ergänzt. Bei einem zu kurzen Scratch-Positionsarray liefert Rust einen kontrollierten Fehler, nachdem das originale Quaternion-Gate ausgeführt wurde; native ungültige Speicherzugriffe und CPU-Sondermodi werden nicht emuliert.

Root-only und Vollpose sind jetzt jeweils mit derselben Vorbereitung verbunden. Die [gemeinsame GetFrame-Zweigauswahl](SKELETAL_FRAME_DISPATCH.md) wählt inzwischen anhand des ursprünglichen Frame-Typs den passenden Einstieg. Echte Runtime-Hosts und Ticking, Skinning und Renderer-Verwendung bleiben offen. Der Code ist noch keine sichtbare native Spielanimation. Androidprüfung am Projektende.

Nachweise: `analysis/reports/animation-entry.json`, `animation-entry-validation.json`, `animation-entry-tests.log`, `analysis/decompiled/skeletal-root-frame.asm` und `analysis/evidence.json`. Vorgänger: [Instanzvorbereitung](SKELETAL_PREPARATION.md), [vorbereitete Vollpose](SKELETAL_FULL_POSE.md).


Fortsetzung 2026-10-08: [Konkrete Hosts fuer geladene Animationen](SKELETAL_RUNTIME_HOSTS.md) verbinden jetzt GetSequence, Originaltracks und tatsaechlich aktualisierte gemeinsame Linkup-Mappings ueber den Frame-Einstieg. 386 Workspace-Tests bestanden; dynamische Actor-/Script-Anbindung, Ticking und Skinning bleiben offen.
