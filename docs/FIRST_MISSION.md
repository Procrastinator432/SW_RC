# Erste spielbare Karte bis zum Übergang – 0.32.0

Die Lieferung umfasst den zusammenhängenden spielbaren Abschnitt **GEO_01A von der Landung bis zum ursprünglichen Ausgang nach GEO_01B**. Sie endet am Kartenübergang. Der Innenbereich von GEO_01B wird noch nicht geladen. Eine vollständige, bild- und verhaltensgleiche Rekonstruktion aller Originalsysteme wird damit nicht behauptet.

## Ausprobieren

Beide Dateien aus `analysis/mission15-complete/delivery` gemeinsam verwenden:

- `republic-commando-0.32.0.apk` installieren; enthält ARM64 und x86_64, Versionscode36.
- `geo_01a-first-mission.rcplay` auf das Gerät kopieren und im Szenendialog öffnen. Sie enthält alle für diesen Abschnitt gebundenen Texturen, Animationen und86 Original-PCM-Samples; Größe112.525.261Bytes.
- Nach dem Laden fortsetzen. Während des Briefings sind Bewegung und Waffen gesperrt; der Blick lässt sich innerhalb der ursprünglichen Grenzen drehen. Danach Bacta erreichen und die eingeblendeten Ziele verfolgen.

Touch: linker Joystick bewegt, rechter freier Bereich dreht den Blick; Feuer, Sprung, Laden, Benutzen, Granate und Ducken sind beschriftet. Benutzen gilt auch für Bacta, Squad-Aufträge und den Zünder. Am Ausgang bleibt die Abschlussansicht stehen; Neustart beginnt wieder mit der Landung.

## Zusammenhängender Ablauf

Landungsbriefing, originaler Abseilclip und Seilabbau führen zum Verletzungs-/Bacta-Einstieg. Danach folgen Absturzstelle mit Droiden, Thermaldetonator-Aufnahme und Geschützstellung, Treffen mit Scorch, Sprengauftrag samt Setzen/Zünden/Rückzug, Artillerie-/Fabrik-Kampf sowie die Freigaben und Tür zum Turmausgang. Die vorhandenen Originalziele0–6 bleiben in einem Lauf verbunden.

Neu sind die texturierten Intro-Actors452 und530 mit ursprünglichen Breathe-/Intro-Clips, Eintritts-Root-Ankern, nativen Kanal-/LOD-Ticks und AnimEnd-/Notify-Callbacks. Zwei Trooper-Stimmen und vier Warrior-Notify-Sounds ergänzen den bisherigen Audiobestand. Trooper452 verschwindet nach der nicht wartenden zweiten Stimme plus Originalfrist; der Warrior wartet auf sein Animationsende und GunshipWarrior. Actor453 ist weiterhin die gebundene Quelle der ersten Stimme; dessen kompletter späterer LZ-Lebenszyklus wird nicht ausgeführt.

Die Warrior-Platzierung benutzt FindSpot/CheckSlice-Steuerfolge mit dem vorhandenen mathematischen BSP-/StaticMesh-Host. Ziel895 wird vor Anwendung der Zielrotation auf eine freie Position korrigiert. Für den anschließenden Auftrag versucht der begrenzte Gehgraph einen kollisionsgeprüften Weg. In dieser Karte liefert er **NoPath**; dies wird als fehlgeschlagener Auftrag protokolliert und der dekorative Actor durch den nachfolgenden Scriptauftrag entfernt. Eine erfolgreiche Wanderung zu413 oder allgemeine native GoalQueue-/Flug-/Kletterparität wird ausdrücklich nicht behauptet. Dieser dekorative Auftrag blockiert die Spielerziele nicht.

## Übergabe am Originalausgang

Nur der ursprüngliche Exit-Trigger9 nach den erforderlichen Bacta-/Absturzstellen-/Geschütz-/Rendezvous-/Spreng- und Türfreigaben erzeugt die Übergabe an `GEO_01B.ctm#playerstart`. Der Datensatz enthält Schema1, Quellkarte, Ziel, Trigger, Tick, Zielnummern0–6 sowie aktuellen Spieler-/Inventar- und Squad-Zustand. Danach nimmt die Simulation keine weiteren Spielereingaben an. Neustart entfernt den Übergabestatus aus der Simulation.

Die Android-App speichert eine erfolgreiche Übergabe atomar als `files/geo_01a-handoff.json` in ihrem privaten App-Verzeichnis. Die versionierte Port-Aufnahme dient der anschließenden Kartenintegration; ein Import-/Wiederaufnahmehost für GEO_01B und die originale Travel-Serialisierung sind noch nicht implementiert. Der Gesundheitswert stammt aus dem tatsächlichen Spielzustand, nicht aus dem nach Kampf veralteten Bacta-Datenfeld.

`rc_game_map_handoff` stellt dieselbe Aufnahme über die native ABI bereit. Vor dem Ausgang liefert es0Bytes; nach dem Ausgang sind Größenabfrage und wiederholtes Kopieren möglich. Zu kleine Puffer bleiben unverändert. Die zusammenhängenden ABI-Läufe prüfen diese Grenzen einschließlich Neustart.

## Abnahme und verbleibende Grenzen

Der Rust-Gesamtlauf endet nach18.730 normalen Eingaben am Originalausgang, mit20HP und acht Gesamtabschüssen. Er prüft alle neuen Notifies, echten AnimEnd, Trooper-/Warrior-Lebenszyklus, korrigierte Platzierung, Übergabe, Eingabestopp und exakten Zustands-/Bildneustart. Windows- und Android-ABI werden mit demselben vollständigen Eingabestrom sowie dem separaten Squad-Kampflauf geprüft. Das ist kein vollständiger manueller Touch-Durchlauf. Die APK wird zusätzlich im vorhandenen Emulator über Dateiauswahl/GPU, Intro-/Waffenaudio, Touch und Pause geprüft; die physische ARM64-Abnahme liegt beim nächsten Gerätetest.

Die bestehenden Grenzen des Ports gelten weiter: begrenzte Kampf-/Squad-/Navigationshosts, vereinfachte Kameraglättung und Weltkontakte, noch fehlende materialabhängige Abseil-Fußschritte sowie nicht vollständig rekonstruierte Shader-/Partikel- und dekorative LZ-Abläufe. Die funktionale Abnahme betrifft den spielbaren Weg bis zur Übergangsgrenze, nicht vollständige Originalparität.

Nachweise und eingefrorene Lieferdateien: `analysis/mission15-complete/delivery/validation-manifest.json`, `playthrough/validation.json`, `playthrough/map-handoff.json`, `native/validation.json`, `android/validation.json` und `source-snapshot.zip`. Die vorherigen0.31.0-Lieferdateien bleiben unverändert.1.097 Rust-Bibliothekstests sowie Clippy/Format bestehen. Der Java-Audiohost besitzt zusätzlich eine deterministische Pause-/Ladecallback-/Neustartprüfung: wartende Ereignisse bleiben während der Pause erhalten, werden nach Fortsetzen einmal in Reihenfolge abgespielt und bei Neustart verworfen. Die APK-Prüfung bestätigt alle zehn Foley-/Intro-Rollen76–85 im Live-Log. Dabei wurde ein vorheriger Verlust in-flight konsumierter Sounds beim Pausengenerationswechsel behoben.
