# Musikgruppen und Kampfmusik in der Mission

Stand: 27.10.2026 · **0.74.0-music-groups**.

Die vollständigen erreichbaren SoundMultiple-Gruppen sind im Missionspaket enthalten und mit dem ursprünglichen Auswahlkern verbunden. Die Kampfprüfung verwendet nun die importierten Kriterien sowie die Gegner-, Bereichs- und Fabrikbeobachtungen des Ports. Android-Import, B-/C-Musik und persistente Audiofortsetzung wurden im Emulator geprüft.

[APK, ARM64 und x86_64](../build/music-groups/android/artifacts/republic-commando-0.74.0-music-groups.apk) · [Neues Szenenpaket](../build/music-groups/assets/package/mission1-scenes.rcpack.zip) · [Prüfbericht](../build/music-groups/verification-summary.json) · [Quellzuordnung](../build/music-groups/source-and-verification.json)

**Zum Ausprobieren:** APK installieren, das neue Szenenpaket einmal importieren und **Neue Mission starten** wählen. Bestehende Spielstände bleiben an ihre bisherige Szene gebunden; bloßes Fortsetzen oder Kartenneustart migriert deren Assets nicht. Das Paket enthält lokal aus der eigenen Installation erzeugte Originalressourcen.

## Umsetzung

- **Vollständige Gruppengraphen:** B enthält sechs Gruppen und 18 komprimierte Titel, C drei Gruppen und 13 Titel. Auch Querverweise auf RAS-Musik werden aufgelöst. FirstSound, Reihenfolge, Gewichte, Filter und ausgewählte Untergruppen werden erhalten; Siegesmusik folgt der tatsächlich gewählten Untergruppe. Loop- und Fade-Einstellungen stammen vom ausgewählten Soundobjekt. Die bestehenden Grenzen von 24 Titeln und 24 MiB pro Bank genügen.
- **Gespeicherte Auswahl:** Ein eigener, gespeicherter Zufallsstrom verwendet den ursprünglichen SRand-Algorithmus. Gruppen-Caches, Zufallszustand, aktive Episode, vorgemerkter Raumzustand und Kampfzustand gehören zum Gameplaycheckpoint. Eine bloße Ausgabeabfrage trifft keine neue Auswahl. Dieser private Strom bildet nicht die globale Zufallsfolge des Originalspiels nach.
- **Kampfmusik:** Attack, Proximity und AttackAndProximity sowie Schwellen und beide Radien werden aus LevelInfo und Aktionen übernommen. Die tatsächlichen Angriffsentscheidungen des begrenzten Port-KI-Hosts liefern dessen Angriffszustand; vollständige originale Controller-KI wird dadurch nicht behauptet. Drei BattleVolumes verwenden ursprüngliche BSP-Ebenen und Transformationen, einschließlich Rückseite bei Ebenengleichheit. Alle darin gefundenen Fabriken sind im Missionshost vorhanden; deren Spawnzähler verhindern verfrühtes Kampfende.
- **Musikwechsel:** Der Host prüft im Sekundentakt; beim Entfernen lebender Gegner gilt die belegte strenge Zwei-Sekunden-Nachlaufgrenze. Episoden sperren weitere Episoden. Vorgemerkte Umgebungsmusik wird nach Freigabe neu ausgewählt und gestartet. Wiederholtes Deaktivieren einer bereits inaktiven Kampfmusik setzt den Raumtitel nicht zurück.
- **Frischer Einstieg A:** LevelInfo.MusicState und die anfänglichen Raum-/Overlay-/Episodenzuweisungen sind leer. Der native Konstruktor weist keinen Titel zu; der Viewportpfad stellt vorhandene Zustände wieder her. Daher erhält ein frischer A-Einstieg weiterhin keinen erfundenen Briefing-Titel. Die vollständige Musikübernahme aus Originalmenü/Filmsequenzen ist damit nicht umgesetzt.

## Im Android-Test gefundene und behobene Fehler

Der größere Szenensatz machte zwei bisherige Lücken sichtbar:

1. **Speicher beim Laden:** Eine vollständige Szene als Java-Bytearray neben Grafik- und Audiobanken überschritt auf dem Emulator den normalen 192-MiB-Heap. Importprüfung, Kartenwechsel, Checkpointladen und Wiederherstellung öffnen jetzt den geprüften Szenencache über eine begrenzte native Datei-Ladefunktion. Manuelle Dokumentimporte werden mit einem kleinen Puffer auf Platte zwischengespeichert. Größenlimit, Inhaltsprüfung und atomarer Cache bleiben erhalten; der Heap wurde nicht vergrößert.
2. **Gameplaycheckpoint ohne Audio-Sidecar:** Bereits konsumierte Ereignisnummern fehlten im neuen Mixer. Der erste folgende Sidecar wurde dadurch wegen unterschiedlicher Cursor verworfen. Ein frischer Audiohost übernimmt jetzt diesen Ausgangszeiger, bevor neue Ereignisse eintreffen. Ein erfolgreich geladener Sidecar behält seinen eigenen Zustand; ein abgewiesener Sidecar fällt auf einen korrekt initialisierten frischen Host zurück.

## Nachweise

- Je **1.518 Debug-/Release-Bibliothekstests**, **82 Java-Audiogruppen**, Clippy, Material-/Dokumentationstests und zentraler Buildvertrag bestehen.
- **4.026 ausgeführte Originalaufrufe**: 1.280 Kampfprüfungen, 2.106 Gruppenauswahlen, 128 Episodengates und 512 SRand-Ziehungen. Beziehung, Tod, Volumen-/Fabrikantworten und weitere Callbackgrenzen werden im Originalvergleich kontrolliert vorgegeben.
- Vollständiger A/B/C→Fabrik-Lauf: **49.795 Eingaben, 1.230 WAV-Ereignisse, 39.836.000 Stereo-Frames**, drei Übergänge und vier kalte Restores. Mit den nun importierten Kriterien entstehen im geprüften Verlauf acht Musikwechsel auf B und dreizehn auf C. Frühere Zahlen der vereinfachten Radiusregel sind kein Sollwert.
- Getrennte native/JVM-Prozesse bestätigen Musikposition, Lautstärke und Ducking. Pool-, Dialog-, Pause- und Checkpointregressionen bestehen. **36 ältere Checkpoints, neun genaue Einstiegsbilder und ein echter Musikstand aus 0.72** bleiben verwendbar.
- Datei- und Byte-Lader liefern für alle vier Szenen dieselben initialen Checkpoints und Soundbanken. Ein gültiger A→B-Dateiübergang stimmt mit dem bisherigen Übergang überein; sechs ungültige Dateieingaben und ein fehlgeschlagener Übergang werden sicher abgewiesen. Zwei Streamingfälle bewegen 64 MiB bei **16 MiB Java-Heap**.
- **Android 35, x86_64, 960×540 bei 240 dpi:** produktiver Paketimport über DocumentsUI; echte Replaycheckpoints erreichen B/C. Nativer Mixer rendert, Pausensidecars werden gespeichert und nach Prozessneustart geladen. Der pausierte Gameplaystand bleibt bytegleich; Musik und Spielzeit laufen anschließend weiter. Kein Audiofehler und kein erneuter Speicherabsturz im erfolgreichen Lauf. ARM64-Bibliotheken sind gebaut und strukturell geprüft; die physische ARM64-Laufzeit ist nicht damit abgenommen.

## Verbleibende Grenzen

Die vollständige originale Musikzustandsmaschine ist weiterhin offen: globale RNG-Zeitlinie, RefreshGroup-Lebenszyklus, Overlay-/Player-down-Verhalten und Menü-/Filmübernahme. Die ausgewählte Port-KI, statische Volumenzugehörigkeit und Fabrikzähler ersetzen keine bewiesene Gleichheit sämtlicher originaler Actor-/Touch-Callbacks. Überlappende Kampfbereiche werden ausdrücklich abgewiesen, solange ihre native Reihenfolge unbekannt ist. Der Sekundentakt folgt der pausierbaren Simulationszeit.

**Eine freie vollständige Handy- und Hörabnahme bleibt nötig.** Der Emulator wurde ohne Audioausgabe zum Lautsprecher betrieben. Klang, reale Latenz, Pegel und Touch-Handhabung über die gesamte Mission sind damit nicht beurteilt; A erreicht im bisherigen Mischtest weiterhin den S16-Spitzenwert.

## Reproduzieren

```powershell
./build.ps1 -Target Check -OutputDirectory build/music-groups/final-check
./build.ps1 -Target Android -Abis arm64-v8a,x86_64 -OutputDirectory build/music-groups/android
./build.ps1 -Target Soundtrack -CampaignPackage <Basis-Kampagnenpaket> -GameData <Eigene-GameData> -Python <Python> -OutputDirectory <Neuer-Ausgabeordner>
```

Zusatzprüfungen: `Test-MusicDecisionsOriginal.py`, `Test-MusicRandomOriginal.py`, `Test-GameFileLoading.py`, `Test-MissionNativeAudio.py`, `Test-MissionSoundtrack.py` und `Test-MusicAndroid.py`. Alle Eingabepfade sind explizit zu übergeben. Originalproben benötigen die unter `tools/analysis-python` lokal eingerichteten gebundenen Analyseabhängigkeiten. Die abgeschlossene Lieferung liegt unter `build/music-groups`; frühere Lieferungen bleiben unverändert. Es wurde weder committet noch getaggt oder gepusht.
