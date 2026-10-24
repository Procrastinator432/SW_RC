# Unterbrechbare Squad-Befehle und Zielwechsel

Stand: 28.10.2026 · **0.79.0-squad-orders / Code 83**.

Dieser Abschnitt verbessert den Befehlslebenszyklus von Scorch auf A und die gemeinsame Begleiter-Navigation auf B/C. Er ist ein erster zusammenhängender Teil der allgemeinen Gegner-/Squad-/Skriptarbeit. Vollständige originale CTBot-/GoalQueue-/Marker- und Skriptausführung ist damit weiterhin offen.

[APK für ARM64 und x86_64](../build/squad-orders/android/artifacts/republic-commando-0.79.0-squad-orders.apk) · [Weiterverwendbares Szenenpaket 0.78](../build/weapon-raise/package/mission1-scenes.rcpack.zip)

Die APK aktualisieren. Das bereits importierte Paket 0.78 und bestehende Checkpoints bleiben verwendbar; ein erneuter Paketimport oder Missionsneustart ist für diese Laufzeitkorrektur nicht erforderlich.

## Sichtbare Änderungen

- **Halten bleibt der Grundbefehl.** Scorch merkt sich Halten während eines Fokusauftrags. Wird das Ziel besiegt oder geht verloren, hält er anschließend an seiner erreichten Position. Eine Rückkehr zum früheren Haltepunkt ist nicht Bestandteil dieses Hosts. Fokuswechsel überschreiben den gemerkten Grundbefehl nicht.
- **Fokus hat einen Abschluss.** Sichtbare Ziele aktualisieren die letzte bekannte Position und die vorhandene Drei-Sekunden-Frist. Bei Sichtverlust wird nur diese letzte Position verfolgt; nach Fristablauf endet der Fokus und der Grundbefehl kehrt zurück. Ein entferntes oder besiegtes Ziel beendet ihn sofort.
- **Neue Befehle verwerfen alte Wege.** Halten/Folgen und Fokuswechsel löschen Route, Wegpunkt und Planungssperre. Die Fokusauswahl erfordert eine freie Sichtlinie vom Spieler.
- **Wiederbelebung beginnt ohne veraltete Folgeroute.** Eigene und Scorchs Kampfunfähigkeit sowie die Rückkehr aus der Aufstehanimation verwerfen die alte Route. Die vorhandenen Gesundheits-, Rettungs- und Bedrohungsregeln bleiben bestehen.
- **Gemeinsame B/C-Navigation reagiert auf Zielwechsel.** Bei einer Zielverschiebung über die bereits verwendete Grenze von 140 Einheiten wird eine alte Route beziehungsweise ein fehlgeschlagener Plan sofort verworfen. Kleinere Änderungen behalten den Umweg, damit die Graphanschlüsse nicht ständig wechseln. Kampfunfähige, tote, nicht beigetretene und am Ziel angekommene Begleiter behalten keinen alten Planungstimer.

Die Bewegung bleibt körper- und bodengeprüft. Es werden keine Begleiter durch Wände versetzt. Die neue Lebenszyklusregel ist ausdrücklich eine Verbesserung des begrenzten Portcontrollers; keine neu belegte native Originalroutine. Die ausgewählten Originalkörper, Clips, Kampfentscheidungen und Wiederbelebungsdaten werden weiterverwendet.

## Speichern und Kompatibilität

Nur ein vorübergehender Fokus aus Halten benötigt das neue optionale Feld `squad.return_order`. Alte Checkpoints ohne dieses Feld bleiben lesbar und behalten beim bloßen Laden dieselben Bytes. Aus einem alten Fokusstand kann der inzwischen verlorene frühere Haltebefehl nicht nachträglich rekonstruiert werden; in diesem Fall gilt die bisherige Rückkehr zu Folgen.

Die Checkpointprüfung lehnt einen weiteren Fokus als Rückkehrbefehl sowie einen Rückkehrbefehl außerhalb eines aktiven Fokus ab, bevor der geladene Zustand installiert wird. Ein beschädigter neuer Befehlszustand verändert den laufenden Spielstand nicht.

## Prüfumfang

Zehn neue Rust-Fälle prüfen Befehlsersetzung, Sichtverlust und Fristgrenzen, expliziten Abbruch, alte und neue Serialisierung, unzulässige Checkpoints, Kampfunfähigkeit, Zielumkehr, Wiederaufnahme nach Heilung sowie Planungssperren. Die Bewegungsfälle verwenden ausdrücklich isolierte physische Bodenfixtures.

`rc-squad-check` erreicht den Abschnitt zunächst über echte Missionseingaben und stellt danach isolierte Originalgeometrie-Fälle durch Positions-/Gesundheitszuweisungen her. Der Fokusauftrag aus Halten wird über einen echten Checkpoint in eine neu geladene Spielinstanz übernommen; Bilder, sämtliche Zustände und der weitere Kampfverlauf müssen übereinstimmen. Dieser Fixture-Nachweis ist kein frei gespielter Missionslauf.

`rc-squad-live-check` verwendet hingegen ausschließlich gewöhnliche Eingaben: Nach Feindkontakt stellt der Spieler das Feuer ein und ruft bei eigener Kampfunfähigkeit um Hilfe. Scorch muss einen Missions-Warrior erreichen, beschießen und besiegen. Der vollständige A/B/C-Lauf wird zusätzlich separat geprüft.

[Originalgeometrie-Fälle](../build/squad-orders/behavior/validation.json) · [Eingabelauf](../build/squad-orders/live/validation.json) · [Nativer Befehlsrestore](../build/squad-orders/native-command/validation.json) · [Gesamtnachweis](../build/squad-orders/verification-summary.json)

## Weiter offen

Die vier Mitglieder besitzen weiterhin keine gemeinsame vollständige originale GoalQueue-, Deckungs-, Markerreservierungs-, Waffenwahl- oder Friendly-Fire-Planung. B/C erhalten durch diesen Abschnitt die bessere gemeinsame Folgenavigation, noch keine frei verwendbaren A-Fokus-/Haltebefehle. Allgemeine Spreng-/Slice-/Breach-Aufträge, ihre Unterbrechung und erneute Vergabe sowie eine allgemeine Originalskript-Laufzeit sind die nächsten größeren Integrationsschritte. Anfangsmusik, freie Handy-/Hörabnahme und die übrigen Grafikbefunde bleiben gesondert offen.

## Ergebnisse

- Zentralcheck: **je 1.534 Debug-/Release-Bibliothekstests**, Materialimport, Doctests, Clippy, Java und Buildvertrag bestanden.
- Eingabebasierter Scorch-Kampflauf mit **16.348 Eingaben** bestanden. Der Spieler stellt nach Feindkontakt sein Feuer ein; Scorch muss den Gegner selbst besiegen.
- Originalgeometrie-Fixtures für Befehlsflanken, abgebrochene Wiederbelebung, gegenseitige Rettung, Kampfbeteiligung und Niederlage bestanden. Der gespeicherte Fokus aus Halten liefert nach einem kalten Rust-Neuaufbau dieselben Checkpointbytes und Bilder sowie denselben Kampfabschluss.
- Drei zusätzliche C-ABI-Restores und **27 weitere Schritte** ergeben dieselben Checkpointbytes; der Abschluss stimmt mit dem Rust-Fixturebericht einschließlich der ursprünglichen f32-Animationsbits überein. Ein anfänglicher Berichtvergleich wurde wegen unterschiedlicher JSON-Dezimalschreibweisen korrigiert; es wird keine numerische Toleranz verwendet.
- **36 ältere Checkpoints und neun Einstiegskamerabilder** sowie ein echter Load-Zwischenstand aus 0.78 bleiben byte-/bildgleich.
- Vollständige Mission mit **49.795 Eingaben und 1.233 WAV-Ereignissen**, drei Kartenübergängen und vier kalten Wiederherstellungen bestanden. Begleiterbewegung und daraus folgende Kampf-/Audiotermine dürfen sich gegenüber 0.78 ändern.
- Android: Paketimport, exakter Fokus-/Haltestand nach Prozessverlust, pixelgleicher Pausenscreenshot und Fortsetzung bestanden. Zusätzlich B/C-Musik mit Pause und kalter Fortsetzung geprüft. Der A-Fall verwendet den ausdrücklich isolierten Geometrie-Fixturestand, keinen frei gespielten Handyweg.
- ARM64-/x86_64-Build und ELF-/JNI-Verträge bestanden.

[Zentralcheck](../build/squad-orders/check-final-verified/validation.json) · [Kampagne](../build/squad-orders/campaign/mission-audio.json) · [Android-Laufzeit](../build/squad-orders/android-runtime/validation.json)

Der erste zentrale Windows-Bau wurde durch eine noch geladene alte DLL blockiert. Der veraltete Lauf wurde beendet; ausgeliefert wird ausschließlich die anschließend neu gebaute und vollständig geprüfte Bibliothek. Die fehlgeschlagenen Zwischenprotokolle bleiben getrennt vom abschließend bestandenen Bericht erhalten.
