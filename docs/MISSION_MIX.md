# Lautstärkesteuerung des nativen Missionsmixers

**Folgestand 0.68.0:** [Native Dialogbesitzregel](MISSION_VOICE_OWNER.md) ergänzt das bereits übertragene NoOverride-Bit, Konsum abgewiesener Zeilen und Schutz über Audio-Restore. Die folgenden Zahlen beschreiben 0.67.0.

Stand: 26.10.2026. **0.67.0-mission-mix** ergänzt vier gespeicherte Lautstärkeregler. Das bestehende Mission-1-Szenenpaket und die Missionscheckpoints können weiterverwendet werden.

## Die fünf umgesetzten Punkte

1. **Nativer Mischvertrag.** `RCMIX001` enthält vier endliche Faktoren von 0 bis 1: Gesamt, Waffen/Geräusche, Sprache und Umgebung. Die Gesamtlautstärke multipliziert die jeweilige Gruppe. Ereigniskategorie 1 ist Sprache; 0 und 2 sind Waffen/Geräusche; Dauerquellen gehören zur Umgebung. Die originalen Quellenparameter und die vorhandene Grundskalierung 0,35 bleiben getrennt erhalten. Alle Standardwerte sind 100 Prozent und verändern den bisherigen Pegel nicht. Das sind Plattformregler, keine neu rekonstruierte Original-Duckinglogik.
2. **Besitzersicherer C-/JNI-Anschluss.** `rc_scene_audio_mix` und `NativeMixer.sceneMix` übernehmen ausschließlich das feste 24-Byte-Paket und reichen es auf denselben Rust-/OpenAL-Besitzerworker weiter. Falscher Hosttyp, veraltete Kennung, ungültige Länge, NaN und unzulässige Faktoren werden abgewiesen. Vollständige Validierung erfolgt vor Backendänderungen. Bei einem physischen Teilfehler wird die Sitzung gesperrt.
3. **Gespeicherte Bedienung.** Im Hauptmenü gibt es **Lautstärke – nativer Mixer**, im Pausenmenü des nativen Pfads **Lautstärke**. Vier beschriftete Prozentregler liegen in einem scrollbaren Dialog mit 48-dp-Reglerhöhe. **Speichern** übernimmt alle vier Werte gemeinsam in private Einstellungen, **Abbrechen** verwirft die Eingabe, **Standard** speichert 100 Prozent. Fehlende oder beschädigte Einstellungstypen fallen auf Standardwerte zurück; außerhalb liegende gespeicherte Zahlen werden begrenzt. SoundPool bleibt als Vergleichspfad beim bisherigen Verhalten.
4. **Anwendung auf laufende und wiederaufgenommene Quellen.** Die Mailbox hält nur den neuesten Einstellungsstand. Der Produzent wendet ihn nach einem optionalen Dialogrestore und vor seiner Bereitschaft an; spätere Änderungen werden ebenfalls auf demselben Besitzer verarbeitet. Nur AL-Gain ändert sich: kein erneutes Play, Upload, Seek oder Rendern durch den Regler. Aktuelle Lautstärke-/Räumlichkeitsparameter bestehender Dauerquellen werden für spätere Gainänderungen nachgeführt. Stummschalten lässt Clipzeit und Ereigniszeiger weiterlaufen. Der RAM-Snapshot speichert ursprüngliche Quellenparameter; aktuelle Benutzereinstellungen gelten auch nach Pause, Neustart, Kartenwechsel und neuem Activity-Aufbau.
5. **Pegel- und Fortlaufprüfungen.** Rusttests prüfen Gruppenisolation, ungültige Eingaben ohne IO, unveränderten Takt/Snapshot, unveränderte Einstellungen ohne neue Backendaufrufe, geänderte Umgebungspegel und aktuelle Einstellungen nach Restore. Java prüft Paketformat, Prozentgrenzen, Kopienbesitz, Änderungszusammenfassung und Anwendung nach Restore vor Bereitschaft. Ein tatsächlicher Windows-OpenAL-Prüfer misst alle Gruppen mit einer realen Missions-WAV, einschließlich halbem Pegel, unabhängigen Gruppen, Master-Mute und Quellenfortlauf.

## Prüfungen

Der zentrale Quellcheck umfasst je **1.456 Debug-/Release-Bibliothekstests**, Materialimport, Doctests, Clippy, Java und Buildvertrag. Fünf Rust-Gruppen und eine native ABI-Gruppe sind neu; eine vorhandene ABI-Gruppe prüft zusätzlich den falschen Hosttyp. Vier reine Java-Mischgruppen und zwei Produzentengruppen kommen hinzu; insgesamt **51 PCM-/Mailbox-/Cache-/Produzenten-/Mischgruppen**.

Der native Pegelprüfer verwendet 26 frische OpenAL-Sitzungen und die tatsächliche GEO_01A-WAV-Bank. Derselbe Sprachclip wird als kontrollierter Testreiz auf jeder Gruppe ausgegeben; die künstliche Gruppenzuweisung wird nicht als originale Missionszuordnung ausgegeben. Standard/100 Prozent und eine fremde stumme Gruppe müssen denselben RMS liefern, halbe Gruppe beziehungsweise halber Master ungefähr die halbe Amplitude, Master-Mute nur den S16-Ditherboden. Eine laufende Sprachquelle muss trotz Mute ihre Sampleposition fortsetzen und nach Restore mit den aktuellen Einstellungen wieder hörbares PCM liefern. Dies misst Samples, keine physische Hörqualität.

[Zentralcheck](../build/mission-mix/check/validation.json) · [Native Pegelmessung](../build/mission-mix/mix.json) · [Java-zu-Nativ-Pausenübergabe](../build/mission-mix/pause-ingress.json) · [Dialogrestore mit echten Eingaben](../build/mission-mix/dialog-resume.json) · [Vollständige Mission](../build/mission-mix/campaign/mission-audio.json) · [Ältere Spielstände](../build/mission-mix/legacy/validation.json) · [Android-Build](../build/mission-mix/android/validation.json) · [Gesamtnachweis](../build/mission-mix/verification-summary.json)

Die vollständige native Missionsregression und die historischen Restores werden mit der neuen DLL erneut ausgeführt. Diese Windows-Prüfungen bestätigen keinen Android-Menü-/Preferences-/AudioTrack-Lauf. Beide Android-ABIs werden gebaut und auf C-/JNI-Exporte, neue Klassen, Bibliotheksidentität im APK und mindestens 16-KiB-ELF-Ausrichtung geprüft.

Gemessen mit demselben WAV-Testreiz: Standard-RMS 2.313,34, halber Gruppen-/Masterpegel 1.156,67, Mute-RMS 0,497. Diese Roh-S16-Werte sind Messwerte des kontrollierten Prüfreizes, keine Empfehlung für Gerätepegel.

## Lieferung und Grenzen

[APK 0.67.0 für arm64 und x86_64](../build/mission-mix/android/artifacts/republic-commando-0.67.0-mission-mix.apk) · [Quellstand und Artefaktzuordnung](../build/mission-mix/source-and-verification.json)

APK-SHA-256: `ae51746bf2daeafe953fd87a81685bb377786aaf97fdf12af819d57c088e025d`.

**Android-Bedienung, kleine Displays, Speicherung über echten Prozessverlust und physisches Hören sind noch nicht abgenommen.** Auf dem Gerät die vier Gruppen einzeln ändern, Dialoge pausieren/fortsetzen, stummschalten/zurückregeln, Karte wechseln und die App neu öffnen. Die Regler müssen erhalten bleiben; fremde Gruppen dürfen nicht mitspringen. Klang, Knackser, Latenz und Leistung bleiben eigene Abnahmen. Gainänderungen können innerhalb des bereits gepufferten PCM erst verzögert ankommen.

Die Regler verringern den vorhandenen Pegel; sie implementieren keinen Limiter und garantieren bei Standardpegel keine Übersteuerungsfreiheit. Die vorige A-Kartenmischung erreichte bereits S16-Vollaussteuerung. Es gibt keine neue Behauptung originaler DSP-, Prioritäts- oder Duckingparität. Vorbis-/Musikstreaming bleibt außerhalb der gegenwärtigen Missions-WAV-Pakete. Eine eigene Musiklautstärke wäre ohne angeschlossene Musikquelle irreführend und ist deshalb nicht enthalten.

Dialogfortsetzung bleibt RAM-basiert an der Rendergrenze; Prozessverlust stellt den Gameplaycheckpoint, keine Dialogposition wieder her. Benutzereinstellungen liegen separat in Preferences und werden nicht durch ältere Gameplay-/Audio-Snapshots überschrieben. Frühere APKs bleiben erhalten; dieser Abschnitt erzeugt keinen Commit, Tag oder Push.

```powershell
./build.ps1 -Target Check -OutputDirectory build/mission-mix/check
./build.ps1 -Target Android -OutputDirectory build/mission-mix/android -Abis arm64-v8a,x86_64
python scripts/Test-MissionMix.py --dll target/release/rc_native.dll --library tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll --package build/linux-sync-20261025/package/artifacts/mission1-scenes.rcpack.zip --output build/mission-mix/mix.json
```
