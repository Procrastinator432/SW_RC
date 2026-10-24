# Unterbrechbarer Sprengauftrag auf GEO_01A

Stand: 28.10.2026 · **0.80.0-marker-tasks / Code 84**.

Scorchs Sprengauftrag reagiert jetzt auf Abbruch, Kampfunfähigkeit und erneute Vergabe. Die bereits platzierte Ladung und ihr Arbeitsfortschritt bleiben erhalten; nach einer Unterbrechung muss Scorch körpergeprüft zum Sprengpunkt zurückkehren. Dies ist ein weiterer Teil der Gegner-/Squad-/Missionsauftragsarbeit, weiterhin ein begrenzter Portcontroller.

[APK für ARM64 und x86_64](../build/marker-tasks/android-final/artifacts/republic-commando-0.80.0-marker-tasks.apk) · [Weiterverwendbares Szenenpaket 0.78](../build/weapon-raise/package/mission1-scenes.rcpack.zip)

Nur die APK aktualisieren. Das importierte Paket 0.78 und bestehende Checkpoints bleiben verwendbar.

## Bedienung und Verhalten

- Während Scorch zum Sprengpunkt läuft oder die Ladung einrichtet, ihn aus der Nähe anvisieren und **BENUTZEN kurz drücken**. Vor dem Platzieren bricht dies den Auftrag ab und gibt den Marker frei. Nach dem Platzieren pausiert der Auftrag; die Ladung verschwindet nicht.
- Für eine neue Vergabe beziehungsweise Fortsetzung **die Ladung anvisieren und BENUTZEN drücken**. Scorch muss wieder einsatzfähig sein. Er geht über die vorhandene Körper-/Bodenprüfung zum Sprengpunkt und setzt erst dort seine Arbeit fort. Es gibt keine Versetzung zum Marker.
- Scorchs Kampfunfähigkeit oder der Wechsel zur Rettung des Spielers pausiert die laufende Einrichtung automatisch. Bewegung, Arbeitszeit und die zugehörigen Ereignisse laufen während der Pause nicht weiter. Die bestehenden Wiederbelebungsregeln bleiben verwendbar.
- Bereits scharfe Ladungen bleiben unabhängig von Scorchs Verfügbarkeit zündbar. Die abschließende Aufstehpose blockiert den bereits freigegebenen Zünder nicht. Die ursprüngliche Auswahl des Zünders und anschließendes Zurücktreten bleiben erhalten.
- Der HUD kennzeichnet einen pausierten Sprengauftrag. Beim Wiederbeleben haben Scorchs Rettungs-/Aufstehposen Vorrang vor der Sprenganimation.

Erneute Vergaben lösen `SpawnDemoDroids` nicht noch einmal aus. Auch Platzieren, Scharfschalten und die dazugehörigen Originalnotifies werden nach einer Rückkehr nicht erneut abgespielt. Das Wiederaufnehmen bewahrt den Stand des ursprünglichen Clips und der Arbeits-/Skriptuhren.

## Umsetzung und Grenzen

Der vorhandene Demolition-Host enthält optionale Pausen-, Erstaktivierungs- und Rückkehrphasendaten. Nach einer Wiederaufnahme läuft zunächst eine gespeicherte Rückkehrroute über die gemeinsame Begleiter-Navigation. Die ursprüngliche normale Auftragsroute bleibt für den ersten ununterbrochenen Auftrag erhalten.

Alte Checkpoints ohne die neuen Felder bleiben beim Laden bytegleich. Ein alter bereits aktiver Auftrag gilt bei Abbruch oder Wiederaufnahme als schon aktiviert, damit seine ursprüngliche Spawnanforderung nicht wiederholt wird. Unzulässige Rückkehrphasen werden vor dem Installieren eines Checkpoints abgewiesen.

Die ursprüngliche Klasse `GOAL_ExecuteMarker` deklariert Marker, Priorität, `bCancelIfUnderway` und `bWaitUntilFinish`; ihre Ausführung ist nativ. Dieser Abschnitt beansprucht keine Rekonstruktion dieser vollständigen GoalQueue. Abbruch-, Pausen- und Wiedervergaberegeln sind ausdrücklich Portregeln. Der neue manuelle Ablauf benötigt den vorhandenen Squad-/Navigationshost; ältere Szenen ohne diesen Host erhalten ihn nicht nachträglich.

B/C-Slice-, Spreng- und Breach-Aufträge werden durch diese Lieferung noch nicht verallgemeinert. Gemeinsame Markerreservierung aller Mitglieder, alternative Teilnehmer, Kampf-/Deckungsplanung während eines Auftrags und allgemeine Originalskriptausführung bleiben weitere Schritte.

## Prüfverfahren

Sechs neue Rust-Fälle prüfen Abbruch und erneute Vergabe, einmalige Aktivierung, eingefrorene Pausen, erhaltene Ladung und Arbeitszeit, Rückkehr und erneute Unterbrechung, scharfe Ladung/Zünder sowie alte und neue Serialisierung und ungültige Phasen.

`rc-demolition-interrupt-check` erreicht den Auftrag zunächst über echte A-Eingaben. Danach verwendet er ausdrücklich isolierte Originalgeometrie-Fixtures: nahe Spielerposition, Gegnergesundheit, Scorchs Kampfunfähigkeit und eine frühere Position auf seiner ursprünglichen Route werden als Testaufbau gesetzt. Die Wiederbelebung und die erneute Vergabe erfolgen über die bestehenden Eingaben. Pausierter Zustand und Rückkehr werden in neue Spielinstanzen geladen; Checkpointbytes, Bilder und Fortsetzung müssen übereinstimmen. Dieser Geometrienachweis ist kein frei gespielter Handyweg.

Der unveränderte A-Eingabelauf und die vollständige A/B/C-Mission werden getrennt geprüft. Die C-ABI-Prüfung verwendet dieselben ausdrücklich isolierten Fixturestände; Android prüft deren produktiven Lade-/Fortsetzungspfad sowie die bestehenden B/C-Musikfälle.

[Geometrie-/Auftragsprüfung](../build/marker-tasks/behavior/validation.json) · [Native Checkpoints](../build/marker-tasks/native-command/validation.json) · [Kampagne](../build/marker-tasks/campaign/mission-audio.json) · [Android](../build/marker-tasks/android-runtime/validation.json) · [Gesamtnachweis](../build/marker-tasks/verification-summary.json)

## Ergebnisse

- Zentralcheck mit **je 1.540 Debug-/Release-Bibliothekstests**, Materialimport, Doctests, Clippy, Java und Buildvertrag bestanden. Der Inspector-Bauschritt erstellt jetzt auch die Squad- und Sprengauftragsprüfer.
- Unveränderter A-Eingabelauf bis zur Kartenübergabe bestanden. Die vollständige Mission besteht mit **49.795 Eingaben, 1.233 WAV-Ereignissen**, drei Kartenübergängen und vier kalten Wiederherstellungen.
- Originalgeometrie-Fixture: Abbruch und Neuvergabe vor dem Platzieren, Pause bei Kampfunfähigkeit, Wiederbelebung über Benutzen, rund **716 Einheiten körpergeprüfter Rückweg** und einmaliges Scharfschalten nach **775 weiteren Schritten** bestanden.
- Pausierter Stand und Rückweg sind in kalten Rust-Spielinstanzen exakt fortsetzbar. Eine unzulässige Rückkehrphase wird atomar abgewiesen.
- C-ABI: **835 weitere Schritte und acht kalte Wiederherstellungen** mit identischen Checkpointbytes und Bildern bestanden; die Ladung wird genau einmal platziert und scharfgeschaltet.
- **36 ältere Checkpoints und neun Einstiegskamerabilder** sowie der gespeicherte Fokus-/Haltefall aus 0.79 bleiben byte-/bildgleich.
- Android: Beide isolierten Auftragsstände werden im produktiven Spiel geladen und nach Prozessverlust exakt wiederhergestellt; beide Pausenbilder bleiben pixelgleich. Nach Fortsetzen bleibt die Einrichtung pausiert beziehungsweise Scorch setzt den körpergeprüften Rückweg fort. B/C-Musik mit Pause und kalter Fortsetzung besteht ebenfalls. Freies Spielen und Hören am physischen Handy bleiben offen.
- ARM64-/x86_64-Build und ELF-/JNI-Verträge bestanden. Szenenpaket und Originalressourcen bleiben unverändert.

Der erweiterte Geometrieprüfer hält mehrere vollständige Spielinstanzen gleichzeitig. Sein Windows-Hauptthread lief dabei zunächst in die Stackgrenze; der Prüfer verwendet jetzt einen expliziten 16-MiB-Arbeitsthread. Die Spiel-/Android-Threads wurden dafür nicht verändert.
