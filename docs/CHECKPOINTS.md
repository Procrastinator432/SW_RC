# Persistente Checkpoints und Fortsetzen – 0.46.0

Die App speichert jetzt vollständigen Fortschritt innerhalb einer Karte. „Fortsetzen“ im Hauptmenü lädt den letzten Checkpoint aus dem privaten App-Speicher. Die Szene wird aus einer geprüften lokalen Kopie geladen; eine erneut erteilte Dateidialog-Berechtigung ist nicht erforderlich. Die vorhandenen Mission-1-Szenen und Kartenübergaben bleiben verwendbar.

## Benutzung

1. `analysis/mission29-checkpoints/delivery/republic-commando-0.46.0.apk` als Update installieren. Die Szenen aus dem Mission-1-Paket 0.45.0 können unverändert weiterverwendet werden.
2. Eine Szene öffnen. Ihr Einstieg wird automatisch gespeichert, anschließend alle 600 Simulationsticks (zehn Spielsekunden), bei Änderungen der wesentlichen Missionsziele und beim Wechsel der App in den Hintergrund.
3. **Checkpoint speichern** schreibt zusätzlich manuell. Nach einer Niederlage bleibt der letzte spielbare Checkpoint erhalten.
4. **Checkpoint laden** stellt den letzten gespeicherten Fortschritt wieder her. Nach einem Prozessverlust die App neu öffnen und im Hauptmenü **Fortsetzen** wählen.
5. Wiederhergestellte Sitzungen beginnen pausiert. **Fortsetzen** im Spiel startet die Simulation. **Neustart** setzt weiterhin die Karte auf ihren übernommenen Eintrittszustand zurück; es ist vom Laden des Checkpoints getrennt.

Ein abruptes Beenden kann die seit dem letzten erfolgreichen Checkpoint gespielten Sekunden verlieren. Beim normalen Hintergrundwechsel wird sofort gespeichert; die periodischen Checkpoints decken den Fall ab, dass Android den Prozess ohne weiteren Lebenszyklusaufruf beendet. Bereits vor dem Update verlorener Zwischenfortschritt lässt sich nicht nachträglich aus einer Kartenübergabe rekonstruieren.

## Wiederherstellung bei Fehlern

Der Dialog **Spielstand wiederherstellen** bietet sichtbare Aktionen:

- **Vorherigen Checkpoint laden** verwendet die vorherige gespeicherte Generation, sofern vorhanden.
- **Karte am Anfang starten** verwendet den lokal gespeicherten Szeneninhalt und versucht die passende Kartenübergabe für den Eintrittszustand. Schlägt auch diese fehl, kann sie ausdrücklich umgangen werden.
- Bei einer beschädigten Kartenübergabe ist **Ohne gespeicherte Übergabe starten** verfügbar. Das startet die Szene mit ihren eigenständigen Standardwerten; der verlorene übernommene Bestand wird dabei nicht erfunden.
- **Andere Szene auswählen** und **Abbrechen** bleiben verfügbar. Eine bereits laufende Sitzung wird bei fehlgeschlagener Wiederherstellung nicht ersetzt.

Abgewiesene Speicherdateien werden bei einer gewählten Wiederherstellung als `.corrupt` aufbewahrt. Eine kaputte Hauptdatei wird niemals über den vorherigen Checkpoint rotiert. Bei defektem Szenencache kann die Originalszene über den Dateidialog neu ausgewählt werden; dadurch wird ihre interne Kopie repariert.

## Gespeicherter Zustand

Der native Snapshot enthält den aktuellen und den ursprünglichen Eintrittszustand: Spielerposition/-bewegung, Blick, Lebenspunkte, Munition, Granaten, Gegner, Squad, Missionsskripte und Wartezeiten, lokale RNG-/Animationszustände, dynamische Türen, Glas/Schild sowie noch ausstehende Eingaben und Simulationsrestzeit. Die Wiederherstellung synchronisiert die Kollisionswelt und erneuert die Grafik-Caches. Es wird kein Missionslauf nachgespielt.

Szene und Checkpoint-Formatrevision sind gebunden. Dateilänge, Prüfsummen, Format, Wertebereiche und Laufzeitstruktur werden geprüft, bevor der neue Zustand gilt. Ungültige Checkpoints verändern die bestehende Sitzung nicht. Vor dem tatsächlichen Weiterspielen verwirft die Android-Oberfläche gehaltene Eingaben und alte Audioereignisse, damit Prozessverlust keine zusätzlichen Schüsse oder Sprünge auslöst.

Privater Speicher: `files/checkpoints/resume.save`, `previous.save`, `last-scene` und `scenes/<SHA256>.rcplay`. Zwei Checkpoint-Generationen werden mit Dateisynchronisierung und atomarem Austausch geschrieben; ein liegen gebliebenes temporäres Schreibfile gilt nicht als Spielstand. Die vollständigen Szenendateien werden je Inhalt einmal in den privaten Speicher kopiert, nicht in den vom System bereinigbaren Cache. Schreibfehler werden sichtbar gemeldet.

## Nachweise

- 25 Checkpoints im tatsächlichen A/B/C-Lauf, einschließlich Einstieg/Abseilen, Kampf, dynamischer Geometrie, Türbrech-/Scharfschützenablauf und Kartenende. Jeder prüft exakten Zustand und Grafik, bis zu 90 identische Folgeeingaben sowie Neustart auf den übernommenen Karteneintritt. A→B→C→GEO_03A wird vollständig erreicht.
- Ausstehender Sprung/Schuss und ein halber Simulationstick werden am Fabrikeinstieg exakt wiederhergestellt.
- Windows-ABI: ursprüngliches Handle freigegeben, neues Handle erstellt, Checkpoint geladen; Zustand/Grafik, weitere 220 Eingaben und Eintritts-Neustart identisch. Acht beschädigte/inkompatible Checkpoints und eine falsche Szene werden atomar abgewiesen.
- Android-Native: separater Speicher- und Wiederherstellungsprozess mit exaktem Status/Grafikpaket, Fortlauf und Neustart; beschädigte Dateien bleiben ohne Wirkung auf die neue Sitzung.
- APK auf emulator-5554: normale Touch-Bewegung/Schüsse, periodisches Speichern ohne Speichertaste, Hintergrundwechsel, tatsächliches `am force-stop`, kalter App-Start und „Fortsetzen“ mit exakt gleichem Simulationszustand. Sichtbare Rückkehr zum vorherigen Checkpoint nach absichtlich beschädigter Hauptdatei sowie Szenenstart nach beschädigter Kartenübergabe geprüft.
- Java-Speicherprüfung: zwei Generationen, unterbrochenes temporäres Schreiben, kaputte Hauptdatei ohne Überschreiben des Backups, Szenen-Prüfsumme und Reparatur, abgeschnittene Daten und unzulässige Pfade. 1.120 Bibliothekstests sowie Clippy/Format bestehen.

APK, Quellstand, Zustandsbeispiele, Bilder und Berichte liegen in der Lieferung; SHA-256 im `validation-manifest.json`. Die 652 bisherigen eingefrorenen Nachweisdateien bleiben unverändert.

## Grenzen

Laufende Audio-Wiedergabe wird nicht samplegenau an ihrer alten Position fortgesetzt. Missions-/Untertitelzeiten bleiben erhalten; alte Soundereignisse werden beim Laden verworfen. Inkompatible spätere Checkpoint-Formatrevisionen benötigen den Wiederherstellungsweg statt einer stillen Zustandsmigration. Die Geräteabnahme dieser Version auf physischem ARM64 und ein vollständiger manueller Touch-Missionslauf bleiben offen; der Prozessverlust-/Dialogtest läuft auf Android x86_64. Die vorher dokumentierten Grenzen der Originalengine-Parität bestehen unverändert.
