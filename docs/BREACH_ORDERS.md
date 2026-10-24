# Unterbrechbare Türbrechaufträge auf Karte C

Stand: 28.10.2026 · **0.82.0-breach-orders / Code 86**.

Das Türbrechmanöver auf GEO_01C pausiert jetzt bei einem nicht einsatzbereiten Squad-Mitglied. Scorch, Fixer und Sev lassen sich in Reichweite wiederbeleben. Danach wird das Manöver ausdrücklich am ursprünglichen Marker fortgesetzt; die vorhandenen Originalanimationen und ihre Ereignisse beginnen nicht erneut.

[APK für ARM64 und x86_64](../build/breach-orders/android/artifacts/republic-commando-0.82.0-breach-orders.apk) · [Weiterverwendbares Szenenpaket 0.78](../build/weapon-raise/package/mission1-scenes.rcpack.zip)

Nur die APK aktualisieren. Das Szenenpaket und bestehende Spielstände bleiben verwendbar.

## Bedienung

| Situation | BENUTZEN am sichtbaren Türmarker |
|---|---|
| Auftrag noch nicht begonnen, alle drei einsatzbereit | Aufstellung starten |
| Squad stellt sich auf | Nach Loslassen erneut drücken: abbrechen |
| Abbruch gerade erfolgt | Erst loslassen, dann neu vergeben |
| Aufstellung wegen Ausfall pausiert | Nach Bergung erneut drücken: fortsetzen; bei weiterhin fehlendem Mitglied abbrechen |
| Türbrechanimation läuft | Nach Loslassen erneut drücken: pausieren |
| Türbrechanimation pausiert, alle einsatzbereit | Nach Loslassen erneut drücken: fortsetzen |

Ein gestarteter Animationsablauf wird nicht abgebrochen und von vorn gestartet. So werden Granatenwurf, Türschalter und bereits ausgesandte Ereignisse nicht doppelt ausgeführt. Ein schon betätigter Schalter lässt die Tür auch während der Pause fertig öffnen. Die anschließende Balkoninszenierung wartet während dieser Pause, damit sie Sev nicht gleichzeitig steuert.

Bei einem beigetretenen, ausgefallenen Mitglied in weniger als 180 Einheiten Entfernung und mit freier Sicht **BENUTZEN drei Sekunden halten**. Das gilt für Scorch, Fixer und Sev. Loslassen oder Verlust der Reichweite/Sicht verwirft die laufende Wiederbelebungszeit. Bacta hat Vorrang, danach Wiederbelebung, danach der Markerbefehl. Auch der Abschlussschritt einer Wiederbelebung setzt das Manöver nicht nebenbei fort. Das HUD zeigt Wiederbelebung und Auftragspause an.

## Spielstände

Das optionale Feld `map.opening.planning.guard.breach.control` speichert Tastenzustand, Pause, Loslassesperre und eine laufende Wiederbelebung. Die bestehenden Animationskanäle, Anker, Notifies, Ereigniszeiten und der Türstand bleiben erhalten. Ungültige Phasen-/Steuerdaten werden vor dem Einsetzen des Spielstands zurückgewiesen.

Alte Spielstände ohne dieses Feld laden unverändert. Beim ersten Schritt wird der aktuelle BENUTZEN-Zustand übernommen, damit gehaltenes BENUTZEN aus dem alten Ablauf nicht versehentlich als neuer Pausen-/Abbruchbefehl zählt. Eine gespeicherte Wiederbelebungszeit bleibt beim pausierten Laden erhalten; der nächste Schritt ohne BENUTZEN beendet diesen Halteversuch wie normales Loslassen.

## Prüfungen

- Zentralcheck: je **1.554 Debug-/Release-Bibliothekstests**, Materialimport, Doctests, Clippy, Java und Buildvertrag bestanden; der zentrale Befehl baut den neuen C-Prüfer mit.
- Unveränderter C-Eingabelauf ab echtem Kampagnenstand bei Tick 240: **12.390 weitere Eingaben** bis zum Kartenende. Zusätzlich vollständige Mission mit **49.795 Eingaben, 1.233 Audioereignissen**, drei Übergängen und vier kalten Wiederherstellungen bestanden.
- Originalgeometrie-Fixtures: Aufstellung abbrechen/neu vergeben; Scorch, Fixer und Sev bergen; Animation/Anker während Pause erhalten; vor und nach dem Türschalter speichern; ohne Clipneustart beenden; Schalter nicht doppelt auslösen; ungültigen Spielstand atomar abweisen.
- Native Schnittstelle: **450 weitere Eingaben und sechs kalte Wiederherstellungen**, identische Bilder sowie vollständige Abschluss-Checkpointbytes gegenüber Rust.
- **36 ältere Checkpoints und neun Einstiegskamerabilder** bestehen. Ein echter C-Teilstand aus Version 0.81 bei Tick 6965 lädt byte-/bildgleich; der erste Schritt mit gehaltenem BENUTZEN löst weder Pause noch Animationsneustart aus.
- Android-Emulator: alle drei C-Mitglieder über den produktiven BENUTZEN-Touchknopf wiederbelebt; Pausenzustand nach Prozessverlust bytegleich, drei Pausenbilder pixelgleich. Die Animation bleibt während Bergung und Wiederbelebung eingefroren und verlangt danach weiterhin die ausdrückliche Fortsetzung am Marker.
- B-/C-Musik, Pause und kaltes Fortsetzen bestehen auch mit dieser APK im Emulator. Freie Hörabnahme bleibt offen.
- ARM64-/x86_64-Build, ELF-/JNI-Verträge und unveränderte Szenenpaketbytes sind geprüft.

[Geometrie und Befehle](../build/breach-orders/behavior/validation.json) · [Native Restores](../build/breach-orders/native-command/validation.json) · [Alter C-Teilstand](../build/breach-orders/legacy/legacy-081-partial.json) · [Vollständige Mission](../build/breach-orders/campaign/mission-audio.json) · [Android](../build/breach-orders/android-runtime/validation.json) · [Gesamtnachweis](../build/breach-orders/verification-summary.json)

## Grenzen und nächster Anschluss

Dies sind ausdrückliche Regeln des ausgewählten Portcontrollers, keine vollständig rekonstruierte native Marker-/GoalQueue-Implementierung. Die lokale Wiederbelebung setzt Gesundheit und Einsatzbereitschaft zurück; native Umfall-/Aufsteh-/Heilanimationen sind damit nicht umgesetzt. Eine unterbrochene Türbrechpose bleibt sichtbar eingefroren.

Die Wiederbelebungs-Fixtures setzen Gesundheit, Gegnerzustand und nahe bodengeprüfte Spielerpositionen nach echten Eingabepräfixen ausdrücklich als Testaufbau. Der Markerstandort wird erst nach dem gespeicherten Fortsetzungstest wieder zugewiesen. Nur die anschließende Abschlussprüfung isoliert den späteren Raumkampf schrittweise. Diese Nachweise ersetzen keinen frei gespielten Handyweg. Der unveränderte vollständige Missionslauf wird getrennt geprüft.

Als nächster Anschluss bleiben körperliche Markerpositionierung und passende Arbeitsanimationen für die B-Aufträge, autonome Ausführung nach einmaliger Vergabe sowie allgemeine Reservierung, Recall und native Gegner-/Squad-GoalQueue. Freie Handy-, Hör- und Leistungsabnahme bleibt offen.
