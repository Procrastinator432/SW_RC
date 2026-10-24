# Autonome Arbeitsmarker auf Karte B

Stand: 29.10.2026 · **0.83.0-marker-execution / Code 87**.

Scorch und Fixer können die fünf B-Aufträge nach einmaligem BENUTZEN selbstständig ausführen. Sie gehen zum zugehörigen Originalmarker, richten sich aus und arbeiten erst in dessen Nähe mit freier Sicht auf das Ziel. Loslassen beendet den Auftrag nicht mehr. Arbeitszeiten und Auftragsvergabe bleiben ausdrückliche Portregeln, keine vollständige native GoalQueue.

[APK für ARM64 und x86_64](../build/marker-execution/android-final-2/artifacts/republic-commando-0.83.0-marker-execution.apk) · [Neues Szenenpaket](../build/marker-execution/package/mission1-scenes.rcpack.zip)

APK aktualisieren und das neue Paket einmal importieren. Für das neue Verhalten eine neue Mission starten oder aus Karte A nach B wechseln. Die unveränderten Karten A/C/Fabrik und vorhandene B-Spielstände bleiben verwendbar: Bereits gespeicherte B-Stände behalten ihre alte, per Prüfsumme gespeicherte Szene und damit die bisherige Haltebedienung. Ein Paketimport schreibt diese Spielstände nicht um.

## Ablauf

- Das verfügbare Ziel wie bisher aus kurzer Entfernung anvisieren und **BENUTZEN einmal drücken**, dann loslassen.
- Scorch übernimmt die drei Sprengziele, Fixer die beiden Slice-Ziele. Jeder hat höchstens einen aktuellen Auftrag. Ein anderes Ziel für denselben Teilnehmer ersetzt diesen Auftrag; alter Teilfortschritt bleibt erhalten.
- Nach Loslassen erneut am selben Ziel drücken: Auftrag pausieren; nochmals drücken: fortsetzen. Nach Ausfall pausiert der Auftrag und verlangt eine ausdrückliche Fortsetzung, sobald das Mitglied wieder einsatzbereit ist.
- Beim Gehen zählt keine Arbeitszeit. Fortschritt beginnt erst innerhalb von 16 Einheiten um den bodengeprüften Arbeitsmarker und mit freier Sicht auf das zu bearbeitende Objekt. Die bisherigen Arbeitszeiten von drei bzw. sechs Sekunden gelten weiter.
- Zugewiesene Mitglieder werden nicht gleichzeitig für die einfache Kampfunterstützung verwendet. Sind beide bereit, arbeitet der begrenzte Host jeweils an einem Marker pro Schritt.
- Versperrt der untätige Scorch Fixer den Weg, erhält er ein erreichbares, boden- und kollisionsgeprüftes Ausweichziel. Dorthin geht er mit normaler begrenzter Bewegung; das Ziel wird mitgespeichert. Dies ersetzt keine allgemeine dynamische Gruppenvermeidung.
- Ladungen werden wie bisher aus sicherer Entfernung mit FEUER gezündet. Ein einmal abgeschlossener Auftrag wird nicht durch gehaltenes BENUTZEN neu begonnen.

Die Arbeitsmarker stammen aus `MarkerDemolition2/0/1`, `MarkerHackTerminal0` und `MarkerHackDoor53` der Originalkarte. Die Bindung wird über deren `Anchor` zur ursprünglichen Ladung/Konsole geprüft. Die Markerpositionen sind von den Zielpositionen getrennt; der Import passt weder Geometrie noch Materialien, Musik oder die anderen Karten an.

## Speichern und Kompatibilität

Die neuen optionalen Daten sind `map.gameplay.execution` für Aufträge, Tastenzustand und Ausweichziel sowie `gameplay.work_sites` im Szenenpaket. Die vorhandene Reservierungs-/Fortschrittsverwaltung bleibt für Arbeitszeit und Abschluss zuständig. Ohne Arbeitsmarker verwendet der Port weiterhin den bisherigen Controller. Aufträge, Wege und Ausweichziel sind nach kaltem Laden fortsetzbar; ungültige Teilnehmer-/Auftragsdaten werden atomar abgewiesen.

Die Prüfung eines späteren B-Spielstands hat außerdem einen bestehenden Fehler sichtbar gemacht: Kampfmusik und die anschließende Siegsequenz erzeugen absichtlich Stimmenkennungen mit gesetztem Bit 62 bzw. 61. Die allgemeine Checkpoint-Zahlengrenze wies solche gültigen Spielstände ab. Nur am exakten Feldpfaden dieser beiden Musikkennungen ist jetzt der erzeugbare 64-Bit-Bereich zulässig. Koordinaten, andere Kennungen und beliebige große Zahlen behalten ihre Grenzen; die Musikzustandsprüfung läuft weiterhin vor dem Einsetzen des Spielstands.

## Reproduzierbarer Paketbau

```powershell
./build.ps1 -Target Markers `
  -CampaignPackage build/weapon-raise/package/mission1-scenes.rcpack.zip `
  -MarkerInventory local-assets/mission1/assets/map_b_inventory.json `
  -Python python -OutputDirectory build/marker-execution
```

Der Builder verwendet ausdrücklich übergebene Dateien und keine historischen Analyseordner. Er erhält alle anderen Szenenbytes sowie B-Musikbank und eingebettete B-Grundszene unverändert. Der Paketbeleg dokumentiert Quellprüfsummen und alle fünf Markerbindungen.

## Prüfergebnisse

- Zentralcheck mit je **1.567 Debug-/Release-Bibliothekstests**, Materialimport, Doctests, Clippy, Java und Buildvertrag bestanden; 13 neue Testgruppen.
- Vollständige Mission mit **50.031 Eingaben und 1.232 Audioereignissen**, drei Kartenübergängen und vier kalten Wiederherstellungen bestanden. Davon 17.782 B-Eingaben einschließlich der 236 zusätzlichen Warteschritte.
- Alle fünf Originalmarker: Pause und Fortsetzung, fehlende Fernarbeit, begrenzte körperliche Bewegung, Ausfall und Abschluss nach Loslassen geprüft. Die Teilnehmer gehen in den isolierten Fortsetzungstests 206 bis 341 Einheiten zum Auftrag.
- Native Schnittstelle: **1.584 weitere Schritte und 14 kalte Wiederherstellungen** mit identischen Bildern und vollständigen Abschluss-Checkpointbytes gegenüber Rust. Ungültige Teilnehmerzuordnung wird atomar abgewiesen.
- **36 ältere Checkpoints und neun Einstiegskamerabilder** sowie 840 weitere Schritte mit alten B-Arbeitsständen bestehen unverändert.
- Android-Emulator: gespeicherte Spreng-/Slice-Pause nach Prozessverlust exakt erhalten, beide Pausenbilder pixelgleich. Je ein produktiver BENUTZEN-Touchimpuls von 300 ms genügt; nach zwölf Sekunden ohne BENUTZEN ist der Auftrag abgeschlossen.
- B-/C-Musik, Pause und kaltes Fortsetzen bestehen mit der neuen APK und dem neuen Paket im Emulator.
- ARM64-/x86_64-Build und ELF-/JNI-Verträge bestanden. Der zentrale Paketbefehl erzeugt bytegleich dasselbe neue Paket.

## Prüfverfahren und Grenzen

Der kontinuierliche A/B-Prüfer verwendet einen echten A-Kampagnenstand, verbleibende unveränderte A-Eingaben, die reale Übergabe nach B und die bisherigen B-Eingaben. Nur beim Loslassen vor Auftragsabschluss fügt er zusätzliche Schritte ohne Eingabe ein: 35, 62, 63, 40 und 36 Schritte, insgesamt 236 bzw. rund 3,93 Sekunden. Die vollständige B-Spur enthält damit 17.782 Eingaben. Spieler-/Squadpositionen, Ressourcen und Missionsereignisse werden in diesem Lauf nicht gesetzt.

Die getrennten Auftrags-Fixtures stammen aus den echten Vergabepunkten dieser Spur. Sie isolieren Gesundheit, Gegner-Todesflags, Spawnkapazitäten und Projektile, versetzen aber keine Squad-Mitglieder. Sie prüfen Pause/Fortsetzung, erhaltene Arbeit, begrenzte körperliche Bewegung zum Marker, fehlende Fernarbeit, Ausfall sowie kalte byte-/bildgleiche Fortsetzung. Der Ausfalltest setzt die Kampfunfähigkeit ausdrücklich als Testaufbau; eine neue Fixer-Wiederbelebung auf B ist damit nicht implementiert.

[Gesamtbeleg](../build/marker-execution/verification-summary.json) · [Eingabespur](../build/marker-execution/behavior/route-validation.json) · [Auftrags-Fixtures](../build/marker-execution/behavior/validation.json) · [Paketbindungen](../build/marker-execution/package/validation.json)

Die Arbeitsdarstellung verwendet noch die bisherigen Ruhe-/Laufposen; passende native Setz-/Slice-Animationen mit vollständigen Übergängen sind der nächste Anschluss. Ebenfalls offen bleiben allgemeine Markerwahl aller vier Mitglieder, nativer Recall/GoalQueue, vollständige Gegner-KI und freie Handy-, Hör- und Leistungsabnahme. Die heutige Änderung betrifft die körperliche Positionierung, selbstständige Arbeitsausführung und Persistenz des ausgewählten B-Hosts.
