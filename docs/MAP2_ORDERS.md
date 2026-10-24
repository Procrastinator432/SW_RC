# Gespeicherte Spreng- und Slice-Aufträge auf Karte B

Stand: 28.10.2026 · **0.81.0-map2-orders / Code 85**.

Die drei Spreng- und zwei Slice-Ziele auf GEO_01B verwenden jetzt eine gemeinsame gespeicherte Auftragsverwaltung. Loslassen, fehlende Einsatzbereitschaft und Zielwechsel verwerfen die bereits geleistete Arbeit nicht mehr. Es wird jeweils nur ein nutzbares Ziel bearbeitet; Scorch und Fixer erhalten je höchstens eine Markerreservierung.

[APK für ARM64 und x86_64](../build/map2-orders/android/artifacts/republic-commando-0.81.0-map2-orders.apk) · [Weiterverwendbares Szenenpaket 0.78](../build/weapon-raise/package/mission1-scenes.rcpack.zip)

Nur die APK aktualisieren. Paket 0.78 und vorhandene Spielstände bleiben verwendbar; ein erneuter Import oder Neustart ist nicht erforderlich.

## Bedienung und Verhalten

Wie bisher am Ziel **BENUTZEN halten**. Loslassen oder sich vom nutzbaren Ziel entfernen pausiert die Arbeit. Zum Fortsetzen dasselbe Ziel erneut anvisieren und BENUTZEN halten. Die ursprünglichen Voraussetzungen, Sicht-/Entfernungsprüfung und bisherigen Port-Arbeitszeiten bleiben bestehen.

- Scorch übernimmt Sprengziele, Fixer Slice-Ziele. Nur ein einsatzbereites, beigetretenes Mitglied kann Fortschritt erzielen.
- Wird für dasselbe Mitglied ein anderes verfügbares Ziel gewählt, ersetzt es die Reservierung. Der alte Teilfortschritt bleibt pausiert und kann später durch erneute Auswahl fortgesetzt werden. Ein Abschluss weist diesen alten Auftrag nicht ungefragt wieder zu.
- Aktive Arbeit und pausierter Fortschritt sind im HUD unterscheidbar. Bereits scharfe Ladungen behalten die bisherigen Abstands-/Feuerregeln.
- Während der bestehenden lokalen Scorch-Wiederbelebung wird kein Marker bearbeitet. Auch der Schritt, in dem die Wiederbelebung endet, zählt nicht gleichzeitig als Arbeitszeit.
- Der vorhandene begrenzte Unterstützungsangriff verwendet ein aktiv arbeitendes Mitglied nicht gleichzeitig zum Schießen. Bei pausierter Arbeit ist diese Unterstützung wieder möglich.
- Start- und Abschlussereignisse werden nicht durch Loslassen erneut gestartet. Die ursprünglichen Skriptcursor laufen weiter und warten für Türen/Ziele auf das tatsächliche Abschlussereignis.

## Speichern und Kompatibilität

Das optionale Feld `map.gameplay.orders` enthält die Reservierungen für Scorch/Fixer und das zuletzt aktiv bearbeitete Ziel. Fortschritt und die vorhandenen `armed`-/`used`-Zustände bleiben in ihren bisherigen Feldern. Die Checkpointprüfung verwirft ungültige Teilnehmerzuordnungen, Indizes, abgeschlossene Reservierungen und widersprüchliche Arbeitszustände atomar.

Alte Teilstände ohne diese Daten werden beim Laden nicht umgeschrieben und behalten zunächst ihre bisherige Anzeige. Erst beim nächsten Simulationsschritt wird die noch offene Arbeit übernommen. Ein erster Schritt ohne BENUTZEN erhält den alten Fortschritt und markiert die Arbeit als pausiert.

## Umfang und Grenzen

Dies verbessert den ausgewählten Portcontroller; es ist keine neu rekonstruierte native Marker-/GoalQueue-Routine. BENUTZEN steuert die Arbeitszeit weiterhin direkt. Körperliches Positionieren am Arbeitsmarker, passende Setz-/Slice-Animationen, autonome Ausführung nach einem einmaligen Befehl, allgemeine Markerreservierung aller vier Mitglieder und ein allgemeiner Abbruch-/Recall-Pfad bleiben weitere Ausbauschritte.

Die B-Wiederbelebung bleibt der vorhandene lokale Drei-Sekunden-Host für Scorch. Die Fixer-Verfügbarkeitsprüfung ist keine neue Fixer-Wiederbelebung. Auch Zünderauswahl und Kampfunterstützung bleiben begrenzte Portregeln. Die C-Breach-Aufträge werden durch diese Lieferung noch nicht verallgemeinert.

## Prüfverfahren

Sieben neue Rust-Fälle prüfen erhaltenen Fortschritt bei Loslassen/Ausfall, getrennte und ersetzte Reservierungen, einmaligen Abschluss, ein bearbeitetes Ziel, kalte Übernahme alter Daten, ungültige Zustände und die unterbundene automatische Rückvergabe alter Arbeit.

`rc-map2-orders-check` verwendet zunächst den echten B-Eingabelauf ab dem gespeicherten Kampagnenstand bei Tick 240. Er muss weiterhin alle acht B-Ziele und die Übergabe nach C erreichen. Aus diesem Lauf stammen die Teilstände für Sprengen und Slicen.

Die anschließenden Originalgeometrie-Fixtures isolieren den Auftrag ausdrücklich: Gesundheit, Spawnkapazitäten, Gegner-Todesflags, Scorchs nahe bodengeprüfte Position und Kampfunfähigkeit werden als Testaufbau gesetzt. Scorch wird über die vorhandene Haltebedienung wiederbelebt. Die Fixer-Ausfallprüfung setzt dessen Zustand nur im Fixture. Neue Spielinstanzen müssen dieselben Checkpointbytes, Bilder und weiteren Arbeitsabschlüsse liefern. Ein ungültiger Teilnehmer-/Arbeitszustand muss den vorhandenen Spielstand unverändert lassen. Das ist kein frei gespielter Handyweg.

Die native Schnittstelle prüft dieselben gespeicherten Fälle. Ein zusätzlicher echter Teilstand aus der eingefrorenen Version 0.80 muss byte-/bildgleich laden und beim ersten Loslassen erhalten bleiben. Die vollständige Mission wird getrennt mit Originaleingaben über A/B/C bis zum Fabrikeinstieg geprüft.

[Geometrie-/Auftragsprüfung](../build/map2-orders/behavior/validation.json) · [Native Prüfung](../build/map2-orders/native-command/validation.json) · [Alter Teilstand](../build/map2-orders/legacy/legacy-080-partial.json) · [Kampagne](../build/map2-orders/campaign/mission-audio.json) · [Android](../build/map2-orders/android-runtime/validation.json) · [Gesamtnachweis](../build/map2-orders/verification-summary.json)

## Ergebnisse

- Zentralcheck mit **je 1.547 Debug-/Release-Bibliothekstests**, Materialimport, Doctests, Clippy, Java und Buildvertrag bestanden. Der zentrale Bauschritt erstellt den neuen B-Auftragsprüfer mit.
- Der unveränderte B-Eingabelauf ab dem echten Kampagnenstand bei Tick 240 erreicht weiterhin alle acht Ziele und die Übergabe nach C.
- Originalgeometrie-Fixtures für Sprengen/Slicen: Loslassen und fehlende Einsatzbereitschaft erhalten Arbeit; Scorchs Wiederbelebung zählt nicht gleichzeitig als Arbeitszeit; beide Aufträge sind nach kaltem Laden mit identischen Zuständen/Bildern fortsetzbar. Die Fixer-Ausfallprüfung ist ausdrücklich ein Fixture, keine neu implementierte Fixer-Wiederbelebung.
- C-ABI: **840 weitere Schritte und acht kalte Wiederherstellungen** bestehen; beide vollständigen Abschlusssnapshots sind bytegleich zu den Rust-Fixtureläufen. Eine falsche Teilnehmerreservierung wird atomar abgewiesen.
- **36 ältere Checkpoints und neun Einstiegskamerabilder** bestehen. Ein zusätzlicher echter B-Teilstand aus 0.80 bei Tick 2483 lädt byte-/bildgleich; beim ersten Loslassen bleibt sein Fortschritt erhalten und wird als pausierter Auftrag übernommen.
- Vollständige Mission mit **49.795 Eingaben, 1.233 WAV-Ereignissen**, drei Kartenübergängen und vier kalten Wiederherstellungen bestanden.
- Android: Beide isolierten B-Aufträge bleiben nach Prozessverlust und Loslassen erhalten; beide Pausenbilder sind pixelgleich. Erneutes Halten des echten BENUTZEN-Touchknopfs setzt die Arbeit fort. Im Sprengfall wird Scorch vorher über denselben vorhandenen Knopf wiederbelebt. B/C-Musik mit Pause und kalter Fortsetzung besteht ebenfalls; freie physische Handy-/Hörabnahme bleibt offen.
- ARM64-/x86_64-Build und ELF-/JNI-Verträge bestanden. Paket 0.78 und Originalressourcen bleiben unverändert.

Die erste zentrale Prüfung beanstandete eine Schleife im neuen Prüfer. Sie verwendet jetzt den vorgesehenen Iterator; der abschließend bestandene Bericht gehört zum korrigierten Quellstand.
