# GEO_01B als Folgekarte – 0.33.0

**GEO_01B („Into the Hive“) ist als begehbare zweite Karte angebunden.** Der ursprüngliche Ausgang von GEO_01A kann die Sitzung auf den Original-PlayerStart der zweiten Karte umstellen. Gesundheit, DC-15s-Magazin/Reserve, Granatenbestand und Scorch-Mitgliedschaft/Gesundheit/Schilde/Kampfunfähigkeit werden übernommen. Die originalen Aufgaben, Gegnerfabriken und Türskripte von GEO_01B sind noch nicht rekonstruiert; die Karte ist derzeit ein Erkundungsabschnitt.

## Ausprobieren

1. `analysis/mission16-map2/delivery/republic-commando-0.33.0.apk` installieren (ARM64 und x86_64, Code37).
2. Die bisherige `analysis/mission15-complete/delivery/geo_01a-first-mission.rcplay` und die neue `analysis/mission16-map2/delivery/geo_01b.rcplay` auf das Gerät kopieren.
3. GEO_01A wie bisher spielen. Am Ausgang **Weiter: GEO_01B** wählen und im Dateidialog `geo_01b.rcplay` öffnen. Die bestehende native Sitzung wird erst nach erfolgreicher Zielprüfung ersetzt.
4. Alternativ GEO_01B direkt öffnen: Falls die App eine abgeschlossene GEO_01A-Übergabe gespeichert hat, wird diese automatisch wiederhergestellt. Ohne gespeicherte Übergabe beginnt der eigenständige Erkundungsstart mit Standardwerten und ohne beigetretenen Scorch.

Ein Update der App erhält deren private Übergabedatei; Deinstallation oder Löschen der App-Daten entfernt sie. **Neustart** in GEO_01B beginnt wieder am neuen Einstieg und behält die übernommenen Anfangswerte. Die bisherigen GEO_01A-Dateien bleiben unverändert.

## Originalkarte und Darstellung

Die Karte wird eigenständig als RCPLAY14 mit einem unveränderten RCPLAY02-/RCPLAY01-Unterbau gespeichert; sie wird nicht an die bereits große GEO_01A-Datei angehängt. Größe:32.810.668Bytes. Original-PlayerStart0: `[-10640.3818359375, -2686.11376953125, -808.0003662109375]`, ursprüngliche Blickrichtung aus dessen Rotator.348 Welt-/Actor-Kollisionsquellen und249.736 sichtbare Dreiecke bilden den aktuellen Stand.

Der neue Export löst Actor-Klassendefaults auf.437 StaticMesh-Instanzen einschließlich Türen, Kisten, Props und ursprünglicher Mover-/KActor-Ausgangsposen werden berücksichtigt; acht ursprünglich versteckte Instanzen werden ausgelassen. BSP-Texturen und die gewählten Mesh-UV-Kanäle werden aus Originalmaterialien gebunden. Panner werden statisch abgetastet;4.559 Effekt-Dreiecke ohne unterstützte opake Oberfläche werden ausgelassen. Zwölf BSP-Dreiecke besitzen im Original keine Materialreferenz und behalten die vorhandene Renderer-Fallbackdarstellung. Das ist in `surface-receipt.json` separat sichtbar; es wird keine vollständige Shader-/Licht-/Karma-/Mover-Parität behauptet.

DC-15s-Modell, Original-Schussgeräusch und Scorch-Mesh samt DC-17m-Befestigung stammen aus den bereits geprüften Originalbindungen der ersten Karte. Scorch wird an einer freien, bodengeprüften Stelle beim Einstieg erzeugt. Der begrenzte Companion-Host folgt nur über unterstützte, kollisionsgeprüfte Bodenkorridore und bleibt bei blockiertem Weg stehen. Wegpunkte, alte Positionen, Trigger, Ereignisse und temporäre KI-Aufträge aus GEO_01A werden verworfen. Für diese neue Karte sind noch keine Kampf-/Wiederbelebungs-/Befehlsabläufe gebunden. Der Granatenbestand wird erhalten; ein separater GEO_01B-Wurf-/Auftragshost folgt mit dessen Gameplay-Integration.

## Wechsel und gespeicherte Wiederaufnahme

`Game.travel_to` verlangt den abgeschlossenen Originalausgang und dessen Übergabe an `GEO_01B.ctm#playerstart`. Es lädt und validiert die gesamte Zielkarte und übernimmt erst danach die geprüften Inventar-/Squad-Werte. `rc_game_travel` ersetzt die Sitzung atomar und verwirft die alten GPU-Paket-Caches. Zu früher Wechsel, beschädigte Daten und falsche Zielkarte ändern den bisherigen Spielzustand nicht. Die native Handle-Identität bleibt beim Wechsel erhalten.

`rc_game_restore_handoff` bindet eine gespeicherte Schema1-Übergabe an eine frische Zielkarte. Falsche Quelle/Ziel, unvollständige Mission, ungültige Inventar-/Squad-Werte, wiederholte Wiederherstellung und Wiederherstellung nach Simulationsbeginn werden abgewiesen. Ein erfolgreicher Restore erneuert den Frame-Cache und den Neustartzustand. Die APK liest `files/geo_01a-handoff.json` nur für GEO_01B; ein gültiger vorhandener Datensatz wird angewandt, statt die Zielkarte still mit Standardgesundheit zu beginnen.

## Prüfung

Der Rust-Test spielt GEO_01A mit18.730 normalen Eingaben bis zum echten Ausgang, wechselt nach GEO_01B und prüft anschließend360 neue Ticks. Im Referenzlauf werden20HP,13Magazinschüsse,180Reserve und Scorch mit100HP/100Schilden übernommen. Bewegung erreicht764,35Einheiten; Springen, Ducken, zwei Schüsse, Nachladen und exakter Zustands-/Bildneustart bestehen. Ungültige Übergaben bleiben ohne Zustandsänderung.

Windows- und Android-ABI prüfen denselben vollständigen ersten Kartenlauf, echten Wechsel, gespeicherte Wiederaufnahme, Bestandsübernahme, Originalwaffenaudio und Neustart. Die APK wurde zusätzlich im vorhandenen Emulator mit einer aus diesem tatsächlichen Android-Durchlauf gespeicherten Übergabe geöffnet:20HP/13Munition beim Start, Original-GPU-Geometrie, Touch-Bewegen/Schießen/Ducken, Schussgeräusch und Pause bestehen. Das ist ein APK-Wiederaufnahme-Fixture, kein vollständiger manueller Touch-Durchlauf beider Karten. Die physische ARM64-Abnahme bleibt beim Gerätetest.

1.097 bestehende Rust-Bibliothekstests sowie Clippy/Format/Whitespace bestehen. Bekannter und unbekannter Dateiumfang wurden mit dem echten Java-Dateileser bei48MiB Heap geprüft. Lieferung und Nachweise: `analysis/mission16-map2/delivery/validation-manifest.json`, `playthrough/validation.json`, `native/validation.json`, `android/validation.json` und `source-snapshot.zip`.

Nächster funktionaler Abschnitt: originale GEO_01B-Einstiegsskripte, Ziele, Türen/Terminals und erste Kampfbegegnungen integrieren. GEO_01C ist weiterhin nicht angebunden. Die gesamte erste Mission über alle drei Karten ist daher noch nicht abgeschlossen.
