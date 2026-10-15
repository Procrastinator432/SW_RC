# Checkpoint-Wiederherstellung – 0.47.0

Die neue APK liegt unter `analysis/mission30-checkpoint-recovery/delivery/republic-commando-0.47.0.apk`. Als Update installieren; die bisherigen Mission-1-Szenen und Checkpoints aus 0.46.0 bleiben verwendbar. Das native Snapshotformat `rcport-state-0.46.0-1` ist unverändert. Bedienung: [Checkpoints und Fortsetzen](CHECKPOINTS.md).

## Änderungen

- Ein erfolgreich geladener vorheriger Checkpoint wird sofort wieder als Hauptspielstand gespeichert. Auch ein erneutes Beenden direkt nach der Wiederherstellung benötigt keine zusätzliche Speichertaste und keinen Hintergrundwechsel.
- Eine abgewiesene Szene oder ein abgewiesener Snapshot verändert die Referenz für „Karte am Anfang starten“ nicht mehr. Sie wird erst nach erfolgreicher nativer Vorbereitung aktualisiert.
- Der neue Spielstand wird vollständig geschrieben und synchronisiert, bevor die Backup-Generation ersetzt wird. Ein Fehler beim Schreiben des neuen Inhalts bewahrt dadurch beide bisher gespeicherten Generationen. Scheitert das Backup-Schreiben, bleibt der Hauptspielstand erhalten und die vorbereitete neue Datei wird entfernt. Fehler bleiben sichtbar; anschließend kann erneut gespeichert werden.

Ein fehlgeschlagener atomarer Austausch kann weiterhin zwei identische Generationen hinterlassen, wenn die Backup-Rotation bereits abgeschlossen war. Mindestens der bisherige Hauptspielstand bleibt dann erhalten. Die Sicherung ist auf Prozessabbrüche ausgelegt; Stromausfall und tatsächliche vollständige Datenträgerbelegung wurden nicht nachgestellt.

## Prüfungen

Der Java-Speichertest blockiert gezielt die Schreibpfade für Haupt- und Backupdatei. Er prüft beide gespeicherten Generationen bytegenau, öffnet den Speicher neu, prüft die Bereinigung vorbereiteter Dateien und speichert nach Beseitigung des Fehlers erneut. Zusätzlich bestehen die bisherigen Korruptions-, Prüfsummen-, Pfad- und Szenencache-Prüfungen.

Die APK-Prüfung auf Android x86_64 umfasst periodisches und Hintergrund-Speichern, tatsächliches `am force-stop`, exakte Wiederaufnahme, Rückgriff auf die vorherige Generation und drei anschließende Kaltstarts ohne zusätzlichen Speichervorgang. Ein blockierter temporärer Schreibpfad erzeugt einen echten Dateisystemfehler; beide vorhandenen Generationen bleiben bytegleich und ein späterer Versuch gelingt. Eine ungültige Szenendatei verändert die Wiederherstellungsszene nicht. Der sichtbare Start nach beschädigter Kartenübergabe wird erneut geprüft.

Eine weitere APK-Prüfung verwendet echte Checkpoints aus dem vollständigen A/B/C-Eingabelauf bei C-Tick 3400, 7300 und 10000. Aktueller und Eintrittszustand werden exakt übernommen, abgesehen von absichtlich verworfenen alten Audioereignissen; die Pause hält die Simulationszeit an. Nach kurzem normalem Weiterspielen, Hintergrund-Speichern und Prozessabbruch entspricht der erneut geladene Zustand exakt dem Hintergrund-Spielstand. Die Teststände werden dafür direkt im privaten Testspeicher bereitgestellt; sie wurden in diesem Test nicht manuell per Touch erreicht.

Die vorherigen 704 eingefrorenen Nachweisdateien bleiben unverändert. Berichte, Bildschirmbilder, APK und Quellstand werden in der neuen Lieferung mit SHA-256 erfasst. Die nativen Bibliotheken sind gegenüber 0.46.0 bytegleich; deren bestehende native Tests gelten unverändert.

## Grenzen und Fortsetzung

Physisches ARM64, ein vollständiger manueller Touch-Missionslauf, tatsächliche Datenträgererschöpfung und Stromausfall bleiben offen. Bei dauerhaftem Schreibfehler kann neuer Fortschritt nicht gesichert werden; die App meldet den Fehler und behält den letzten erfolgreichen Stand. Die bisherigen Grenzen von Audio-Wiedergabeposition und Originalengine-Parität bestehen weiter. Als nächster Abschnitt bietet sich die Geräteabnahme mit wiederholtem Hintergrundwechsel, Pause und Fortsetzen in allen drei Missionskarten an.
