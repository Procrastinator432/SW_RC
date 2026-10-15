# Kampagnenbedienung – 0.49.0

## Auf dem Gerät starten

1. Die APK `build/releases/0.49.0/republic-commando-0.49.0-campaign.apk` als Update installieren.
2. `build/releases/0.49.0/mission1-scenes.rcpack.zip` auf das Gerät kopieren und im Hauptmenü einmal **Szenenpaket importieren** wählen. Das Paket ist ungefähr 49 MiB groß und enthält alle vier benötigten Szenen. Es wird direkt als ZIP ausgewählt, ohne vorheriges Entpacken.
3. **Neue Mission starten** beginnt in der Landezone. Ein vorhandener letzter Spielstand wird erst nach Bestätigung durch den neuen Einstieg ersetzt.
4. **Fortsetzen** lädt den letzten Checkpoint. Das Hauptmenü zeigt Kartenabschnitt und Speicherzeit; beschädigte Checkpoints öffnen weiterhin die Wiederherstellung.

Die vorhandenen Szenen werden vollständig in den privaten App-Speicher übernommen. Die Datei im Downloadordner wird nach dem Import nicht mehr gebraucht. Ein erneuter Dateidialog ist im normalen Kampagnenlauf nicht erforderlich. Das Paket enthält A → B → C und den anschließenden Fabrikeinstieg; der eigentliche weitere Fabrik-Missionsablauf bleibt wie bisher außerhalb dieses Abschnitts.

## Kartenwechsel und Pause

Nach einem abgeschlossenen Abschnitt wählt die App automatisch die nächste importierte Karte. Die native Reise übernimmt Spielerbestand und Squad. Ein am Kartenende gespeicherter Checkpoint löst nach dem Laden denselben Übergang aus. Eine beschädigte oder fehlende importierte Szene bietet die Rückkehr ins Hauptmenü zum erneuten Paketimport; sie führt nicht stillschweigend zu einer anderen Karte.

Im Spiel belegt die obere Leiste 48 dp. **Pause** hält die Simulation an; **Menü** bietet Fortsetzen, Checkpoint speichern/laden, Steuerung, bestätigten Kartenneustart und Hauptmenü. Manuelle Szenenwahl und Grafikumschaltung bleiben dort für Diagnose und Wiederherstellung erreichbar. Die separate **Diagnose** im Hauptmenü enthält den bisherigen Geometriebetrachter.

Ein erneut geladener laufender Abschnitt beginnt weiterhin pausiert. Die sechs Aktionsflächen und gehaltene Eingaben werden bei Pause, Hintergrundwechsel und Kartenwechsel zurückgesetzt. Vorhandene Checkpoints werden beim Paketimport nicht geändert; das native Checkpointformat bleibt kompatibel zu 0.46–0.48.

Wenn Android einen Hintergrundprozess beendet und die ursprünglich mit „Neue Mission“ gestartete Spiel-Activity wiederherstellt, lädt sie ebenfalls den Checkpoint. Der ursprüngliche Startauftrag darf dabei keinen erneuten Beginn bei Tick null auslösen. Ein während des Imports beendeter Prozess veröffentlicht kein unvollständiges Paket; in diesem Fall den Import erneut auswählen.

## Kleine Displays und Empfindlichkeit

Die Aktionsflächen haben mindestens 48 dp Durchmesser, Pause/Menü mindestens 56 dp Breite. Zeichnung und Trefferprüfung verwenden dieselbe Geometrie. Die feste linke Bewegungsfläche und die sechs rechten Aktionen liegen ohne Überschneidung innerhalb der Spielfläche. Das Startmenü und die Pausenaktionen sind bei geringer Höhe scrollbar.

Unter **Steuerung** lässt sich die Blickempfindlichkeit zwischen 50 und 200 Prozent speichern. **Standard** setzt sie auf 100 Prozent zurück. Beide Blickachsen verwenden dieselbe Skalierung nach dp; die Einstellung übersteht einen Prozessneustart. Bei 100 Prozent entsprechen 80 dp Ziehbewegung ungefähr 0,375 Radiant beziehungsweise 21,5 Grad, bei 200 Prozent dem Doppelten. Die vertikale Kamera bleibt durch die vorhandene native Blickbegrenzung beschränkt.

## Importformat und Build

Das Paket beginnt mit `campaign.properties` im Format `RCCAMPAIGN1`. Für `GEO_01A`, `GEO_01B`, `GEO_01C` und `GEO_03A` enthält es jeweils Dateiname, Bytezahl und SHA-256 sowie genau die vier kanonisch benannten `.rcplay`-Einträge. Die App streamt die Inhalte begrenzt auf temporäre Dateien, prüft Größe/Hash und lädt jede Karte nativ zur Prüfung ihrer tatsächlichen Kartenkennung. Diese Kennung kommt aus gebundenen Laufzeitassets; Missions- oder Übersetzungstexte bestimmen keine Kartenidentität.

Das Paketverzeichnis wird erst nach erfolgreicher Prüfung aller vier Karten atomar veröffentlicht. Szenen und Checkpoints teilen den nach Inhalts-Hash benannten Szenenspeicher. Ein erneuter Import dupliziert die Szenen nicht und kann beschädigte oder fehlende Cachedateien reparieren. Abgewiesene Pakete ersetzen den vorhandenen Index nicht; liegen gebliebene temporäre Dateien werden beim nächsten Import bereinigt. Der Paketimport ist auch über mehrere Activity-Instanzen im Prozess serialisiert.

```powershell
# Quell-/Java-Prüfungen einschließlich Paketstore und Touch-Geometrie:
.\build.ps1

# APK:
.\build.ps1 -Target Android

# ZIP aus den vier expliziten Szeneneingaben erzeugen:
.\build.ps1 -Target Package -SceneInputs build/scenes/resolved-inputs.json

# Der vorhandene All-Pfad baut nach den Szenen jetzt auch das Importpaket.
# Voraussetzungen und vollständiger Aufruf: docs/BUILD.md
```

Der APK-Bedientest liegt unter `scripts/Test-CampaignAndroid.py`. Er nimmt APK, Paket und das Verzeichnis echter Replay-Checkpoints als explizite Argumente. Die Standardpfade entsprechen dem zentralen Build unter `build/artifacts` und `build/scenes/runtime-validation`. Er verwendet ausschließlich `emulator-5554` und stellt dessen vorherige Größen-/Dichteüberschreibungen anschließend wieder her.

## Nachweise und Grenzen

- Zentraler Prüfpfad: Debug-/Release-Bibliothekstests, Doc-Tests, Clippy aller Targets, Native-/Inspector-Build sowie Java-Kampagnen-, Checkpoint-, Paket- und Szenenleseprüfungen.
- Java: fehlende, korrupte, doppelte und pfadfremde ZIP-Einträge, native Abweisung ohne Indexwechsel, Reparatur ohne Veränderung des Checkpoints, Bereinigung unterbrochener Importe, wiederholter Import ohne Szenenduplikate. Das echte Vier-Szenen-Paket wird unter 96 MiB Java-Heap importiert; die größte Szene ist größer als dieser Heap.
- Touch: 16 Layoutvarianten über vier Dichten, Mindestgröße, Randabstände, fehlende Überschneidungen und lineare dichteunabhängige Empfindlichkeit.
- APK auf Android x86_64 bei 960 × 540 Pixeln und 240 dpi, entsprechend 640 × 360 dp: echter Dateidialog/Paketimport mit nativer Kartenprüfung, erhaltener Spielstand, kalter Neustart, Start von A ohne Szenendialog, sichtbare Abweisung eines unvollständigen Pakets, Blickmessungen beider Achsen, gespeicherte Empfindlichkeit sowie Bewegung/Schuss/Sprung per Touch.
- Automatische Übergänge A→B, B→C und C→Fabrikeinstieg verwenden echte Abschluss-Checkpoints aus dem vollständigen Eingabelauf. Der Test prüft die tatsächlich geladenen Zielkarten und den übernommenen Eintrittsbestand ohne Dateidialog. Diese Checkpoints werden für den Bedientest direkt im privaten Testspeicher bereitgestellt; das ist kein erneut manuell gespielter vollständiger Touch-Missionslauf.

Die kleine Oberfläche wurde im Emulator geprüft. Das physische Griffgefühl und die Bedienbarkeit auf dem persönlichen ARM64-Gerät bleiben für den Gerätetest offen. Die bekannten Grenzen von Originalengine-Parität und nicht gespeicherter PCM-Abspielposition bestehen weiter.
