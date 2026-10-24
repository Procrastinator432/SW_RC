# Dialogfortsetzung nach Pause und Audiofokusverlust

**Folgestand 0.66.0:** [Fünf Audio-Lebenszykluskorrekturen](AUDIO_LIFECYCLE.md) ergänzen wartende Java-Dialoge, die erste Spielpose als Rendergrenze, Fehler-/Startbehandlung und Tests des Produktionsproduzenten. Die folgenden Zahlen beschreiben den Stand 0.65.0.

Stand: 26.10.2026. **0.65.0-dialog-resume** ergänzt den nativen Missions-WAV-Pfad um begrenzte Audiofortsetzung im Arbeitsspeicher. Bei normaler Pause oder Audiofokusverlust kann ein laufender Dialog im neuen Ausgabekontext an seiner bisherigen Quellenposition weiterlaufen. Das bestehende Szenenpaket bleibt verwendbar.

## Verhalten und Grenzen

Der native Besitzerworker liest vor dem Schließen die tatsächlich spielenden Dialogquellen und ihre `AL_SAMPLE_OFFSET`-Positionen aus. Er hält außerdem bereits angenommene wartende Dialoge, Ereigniszeiger, Rendertakt, Listenerpose und gewünschte Dauerquellen fest. Der neue Kontext lädt dieselbe Bank, stellt Dialogparameter und Quellenposition wieder her und beginnt eine neue PCM-Ausgabeepoche. Abgeschlossene Clips und kurze Waffen-/Foley-Nachklänge werden nicht wiederholt.

Dauerquellen warten auf die erste aktuelle Zustandsmeldung des Spiels. Nur bei gleicher Besitzer-, Sound-, Aktivierungs- und Flagkennung wird die alte Quellenposition übernommen. Entfernte oder neu aktivierte Quellen bekommen keinen alten Abspielstand. Die normale Spielpause beendet einen gehaltenen Bacta-Benutzungsvorgang; dessen Quelle darf deshalb verschwinden. Ein schneller zweiter Pausenwechsel erhält auch noch nicht abgeglichene Dauerquellenpositionen.

Der Java-Produzent erstellt den Snapshot vor der nativen Freigabe. Ein versionsgebundener Zwischenspeicher und die serielle Produzentenfolge schützen vor verspäteten Ergebnissen. Neustart, Stoppen, Kartenwechsel und Schließen verwerfen den Zwischenspeicher. Die Ereignismailbox wartet beim Start auf echte aktuelle Loopdaten. Ein verworfener Snapshot führt zu einer frischen Ausgabe; falsche Bank, ungültige Positionen, unbekannte Felder und überschrittene Budgets werden vor der Wiederherstellung geprüft. Bei einem physischen Teilfehler wird der betroffene Kontext gesperrt und ersetzt. Der Snapshot ist höchstens 256 KiB groß und enthält keine nativen Handles oder Zeiger.

**Dies ist eine Fortsetzung an der Rendergrenze.** Die Position liegt hinter bereits gerendertem, eventuell noch ungehörtem PCM. Beim Pausieren verworfenes Software-/AudioTrack-PCM wird nicht nachgereicht; die Softwarequeue fasst 20 ms, der Geräte-/Trackpuffer kommt hinzu. Resampler-Bruchteile und Filterhistorie werden nicht gespeichert. Eine lückenlose Fortsetzung am tatsächlich gehörten Sample wird daher nicht behauptet.

**Der Snapshot liegt ausschließlich im RAM.** Nach Prozessverlust greift weiterhin der persistente Missionscheckpoint: Spielzustand und gewünschte Weltquellen werden wiederhergestellt, ein halber Dialogsatz dagegen nicht. Der SoundPool-Vergleichspfad bleibt beim bisherigen Verhalten. Allgemeines Vorbis-Streaming, originale Prioritäts-/Duckingorchestrierung und vollständige DSP-Parität sind weiter separate Arbeiten.

## Nachweise

- Der zentrale Quellcheck besteht mit je **1.450 Debug-/Release-Bibliothekstests**, Materialimport, Doctests, Clippy, Java- und Buildvertrag. Beide Android-ABIs sind gebaut; die APK enthält die geprüften JNI-/C-Exporte und bytegleichen Bibliotheken nach Gradles Strip-Stufe mit mindestens 16-KiB-Ausrichtung.
- Elf neue Rust-Gruppen prüfen Positionen/Parameter, abgeschlossene Clips, wartende Dialoge, Loopabgleich, schnelle zweite Pause, Bank-/Format-/Budgetfehler, frische Zielkontexte und physische Teilfehler. Ein echter OpenAL-Test mit Pitch 1,25 kontrolliert Quellenposition und restliche Abspieldauer über einen Kontextwechsel.
- Zwei neue C-ABI-Gruppen prüfen Größenabfrage ohne Taktfortschritt, Snapshot/Restore über neue Besitzerkennungen, Epochen, falschen Hosttyp und fehlerhafte Daten.
- Vier Java-Gruppen prüfen Kopienbesitz, Versionsgrenzen, leeren Fallback und den konkurrierenden Reset gegen verspätete Speicherung. Die fünf Mailboxgruppen prüfen auch das Warten auf die erste aktuelle Zustandsmeldung. Insgesamt 34 PCM-/Mailbox-/Cache-Javagruppen.
- Der echte Missionsprüfer erreicht nach 2.789 normalen GEO_01A-Eingaben und 57 Ereignissen sowohl Dialog als auch Dauerquelle. Beide behalten über je zwei direkte Neuaufbauten exakt ihre Quellenposition; falsche Bank und abgeschnittene Daten werden abgewiesen. Die anschließende tatsächliche PCM-Ausgabe ist nicht stumm.
- Der vollständige Windows-Regressionslauf umfasst wieder 49.795 Eingaben, 1.230 Ereignisse, drei Kartenübergänge, vier kalte Spielrestores und den Fabrikeinstieg. 36 ältere Spielstände und neun Einstiegskameraframes werden separat geprüft.

[Dialog-/Pausenlauf](../build/dialog-resume/dialog-resume.json) · [Vollständige Mission](../build/dialog-resume/campaign/mission-audio.json) · [Zentralcheck](../build/dialog-resume/check/validation.json) · [Ältere Spielstände](../build/dialog-resume/legacy/validation.json) · [Android-Build](../build/dialog-resume/android/validation.json) · [Gesamtnachweis](../build/dialog-resume/verification-summary.json)

## Lieferung und Geräteabnahme

[APK 0.65.0 für arm64 und x86_64](../build/dialog-resume/android/artifacts/republic-commando-0.65.0-dialog-resume.apk). Im Startmenü **Nativer Mixer** wählen; Szenenpaket und Missionscheckpoints können weiterverwendet werden.

APK-SHA-256: `e41c37c484f983b77a550a553c1fa5130d34eb3cffb12f7aa9cf78b50834605b`. [Quellstand und Artefaktzuordnung](../build/dialog-resume/source-and-verification.json) binden APK, Windows-Bibliothek, Berichte und den lokalen Quellsnapshot zusammen. Frühere APKs und eingefrorene Lieferungen bleiben unverändert; es wurde kein Commit, Tag oder Push erzeugt.

Auf dem Gerät stehen noch aus: mitten im gesprochenen Satz pausieren/fortsetzen, mehrfach rasch in den Hintergrund wechseln, Audiofokus verlieren und zurückkehren, anschließend neu starten und Karte wechseln. Dialoge dürfen weder am Satzanfang doppelt beginnen noch aus einer verlassenen Karte weiterlaufen. Bacta benötigt nach Pause eine erneute Benutzung. Zusätzlich Pegel, Knackser, Latenz und Leistung hören beziehungsweise beobachten. Ein erzwungener Prozessverlust soll den Gameplaycheckpoint wiederherstellen; Dialogpositionen sind dabei ausdrücklich nicht persistent.

Windows-OpenAL-Laufzeit und APK-/ELF-Struktur werden geprüft. **Android-Laufzeit und Hörabnahme sind in diesem Abschnitt nicht ausgeführt.** Die Java-Rennentests ersetzen keine echte Activity-/Fokusabnahme.

```powershell
./build.ps1 -Target Check -OutputDirectory build/dialog-resume/check
./build.ps1 -Target Android -OutputDirectory build/dialog-resume/android -Abis arm64-v8a,x86_64
python scripts/Test-DialogResume.py --dll target/release/rc_native.dll --library tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll --package build/linux-sync-20261025/package/artifacts/mission1-scenes.rcpack.zip --inputs build/linux-sync-20261025/complete/scenes/resolved-inputs.json --output build/dialog-resume/dialog-resume.json
```

Die nächsten sinnvollen Arbeiten sind die Geräteabnahme dieses Anschlusses, die Entscheidung über persistente Dialogfortsetzung nach Prozessverlust und die schrittweise Integration der bereits rekonstruierten allgemeinen Audio-/Streamingregeln. Diese Änderung erhöht keine Zählung originalgetreu rekonstruierter Funktionen; sie verbessert den Plattform- und Lebenszyklusanschluss.
