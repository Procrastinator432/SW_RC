# Fünf Korrekturen am Missionsaudio-Lebenszyklus

**Folgestand 0.67.0:** [Missionslautstärke](MISSION_MIX.md) ergänzt vier gespeicherte Regler, den nativen Mischvertrag, Anwendung auf laufende/wiederaufgenommene Quellen und echte OpenAL-Pegelmessungen. Die Zahlen unten beschreiben 0.66.0.

Stand: 26.10.2026. **0.66.0-audio-lifecycle** ergänzt die RAM-Dialogfortsetzung aus [0.65.0](DIALOG_RESUME.md). Die Änderungen betreffen die Java-Übergabe an den nativen Missionsmixer. Gameplay, persistente Spielstände und Szenenpaket bleiben beim bisherigen Format.

## Umgesetzte Punkte

1. **Wartende Dialoge beim Pausieren erhalten.** Die UI versiegelt die alte Mailbox. Der bisherige Besitzerproduzent übergibt noch wartende Sprachereignisse vor dem Snapshot an denselben nativen Ereigniszeiger. Bereits entnommene Pakete werden nicht nochmals übertragen. Ausstehende Waffen-/Foley-Ereignisse werden gefiltert; Lücken in den Ereignisnummern sind zulässig. Der Übergabepfad rendert dabei kein weiteres PCM. Er hat ein Limit von 512 wartenden Dialogereignissen und benötigt eine gültige Spielpose; ein Überschreiten oder ein nativer Annahmefehler verwirft die Audiofortsetzung kontrolliert.
2. **Wiederhergestellte Sprache bis zum aktuellen Spielzustand anhalten.** Native Ressourcen dürfen schon bereit sein, damit das Spiel seinen ersten Zustand liefern kann. Der Produzent reserviert und rendert aber erst, wenn die neue Mailbox eine echte aktuelle Loop-/Listenerzustandsmeldung erhalten hat. Eine wiederhergestellte Sprachquelle läuft damit während der Vorbereitung nicht unhörbar weiter. Unveränderte leere Loopmengen zählen ebenfalls als gültige erste Meldung.
3. **Fehlerhafte Sitzungen von der Fortsetzung ausschließen.** Öffnungs-, Update-, Render-, Snapshot- und Freigabefehler dürfen keinen alten Snapshot als neuen Fortschritt veröffentlichen. Fehler löschen nur ihre eigene Cacheversion; ein späterer Reset beziehungsweise neuer Stand gewinnt weiterhin gegen alte Ergebnisse. Ein fehlerhafter Restore schließt seinen Kontext genau einmal und öffnet einmal frisch. Abgebrochene Öffnung ohne Taktfortschritt erhält einen zuvor gültigen Pausensnapshot. Native Aufrufe einschließlich Freigabe bleiben auf dem Besitzerproduzenten.
4. **Hängenden Audiostart erkennbar beenden.** Solange native Ressourcen vorbereitet werden, prüft die normale Bereitschaftsabfrage eine monotone 20-Sekunden-Grenze. Danach meldet der bestehende Fehlerpfad verständlich, dass die Mission erneut geladen werden soll. Eine verspätete Bereitschaft hebt den abgebrochenen Start nicht wieder auf. Nach erfolgreicher Vorbereitung läuft diese Frist nicht weiter. Die Grenze beendet den UI-Wartezustand; sie unterbricht keinen bereits blockierten nativen Aufruf gewaltsam.
5. **Den tatsächlichen Produktionsablauf unter kontrollierten Unterbrechungen prüfen.** `NativeSceneAudio` verwendet jetzt den Android-unabhängigen `SceneAudioProducer`. Dessen Tests steuern genau die produktiven Übergänge mit Latches statt zeitabhängigen Pausen. Die dabei erzeugten gefilterten Übergabepakete werden zusätzlich an den echten Windows-C-ABI-/OpenAL-Pfad mit der realen GEO_01A-WAV-Bank übergeben.

## Prüfungen und Aussagegrenzen

Neun neue Produzentengruppen prüfen erste Pose, Pause während Rendern beziehungsweise Update, wartende Sprache, Reset während Snapshot, ungültigen Restore mit frischem Kontext, Fehler einschließlich fatalem Backendfehler, Startfrist, Abbruch beim Öffnen und Öffnungsfehler. Die Fehlergruppe umfasst Update, Rendern, Snapshot, Freigabe und einen fatalen Fehler. Zwei zusätzliche Mailboxgruppen prüfen gemischte Sprach-/Foley-Pakete, Versiegelung, einmaliges Leeren, Übergabebudget und fehlende Pose. Die Gruppenzahl zählt Testszenarien, keine neu dekompilierten Originalfunktionen.

Der native Anschlussprüfer verwendet die **tatsächlich vom Java-Produzententest geschriebenen Bytes**. Ereignis 1 wird vor der Pause zugestellt; die wartenden Dialoge 2 und 4 werden anschließend vor dem Snapshot angenommen, während Foley-Ereignis 3 entfällt. Die native Warteschlange `[2, 4]` bleibt über einen echten OpenAL-Kontextwechsel exakt erhalten und wird nach Wiederaufnahme einmal verbraucht. Das sind kontrollierte Ereignisse mit realer WAV-Bank, kein zusätzlich frei gespielter Missionsdurchlauf.

Der Zentralcheck umfasst je **1.450 Debug-/Release-Bibliothekstests**, Materialimport, Doctests, Clippy, Java und Buildvertrag. Die PCM-/Mailbox-/Cache-/Produzentengruppe umfasst jetzt **45 Java-Prüfgruppen**. APK und ELF-Struktur werden für arm64 und x86_64 einschließlich der neuen Produktionsklasse geprüft.

Der native Rust-/C-Code ist gegenüber 0.65.0 unverändert. Die Nachweiszuordnung kontrolliert dieselbe Windows-DLL und dieselben nativen Android-Bibliotheken. Der vollständige Missionstest mit 49.795 Eingaben, 1.230 Ereignissen, drei Übergängen und vier kalten Restores sowie 36 ältere Spielstände/neun Kameraframes stammen aus der **unveränderten, hashgebundenen 0.65.0-Abnahme**. Sie werden wiederverwendet und nicht als neue Android- oder Java-Laufzeitabnahme ausgegeben.

[Zentralcheck](../build/audio-lifecycle/check/validation.json) · [Java-zu-Nativ-Übergabe](../build/audio-lifecycle/pause-ingress.json) · [Android-Build](../build/audio-lifecycle/android/validation.json) · [Gesamtnachweis](../build/audio-lifecycle/verification-summary.json) · [Quellstandzuordnung](../build/audio-lifecycle/source-and-verification.json)

## Lieferung

[APK 0.66.0 für beide Architekturen](../build/audio-lifecycle/android/artifacts/republic-commando-0.66.0-audio-lifecycle.apk). APK-SHA-256: `8486a93d3fa89d111f54a118265a0b228284a7ad6f45170cb5c7cd73ae431f29`.

Das bestehende Mission-1-Szenenpaket und die Missionscheckpoints können weiterverwendet werden. Im Startmenü den nativen Mixer wählen.

Die Fortsetzung bleibt auf den RAM und die Rendergrenze beschränkt: ungehörtes gepuffertes PCM und Resamplerhistorie werden verworfen. Nach Prozessverlust stellt der persistente Missionscheckpoint den Spielzustand wieder her, keine Dialogposition. SoundPool bleibt beim bisherigen Vergleichspfad. Allgemeine Vorbis-/Prioritäts-/Duckingintegration und vollständige Original-DSP-Parität sind weiter offen.

**Android-Laufzeit, echte Audiofokuswechsel, AudioTrack und Hören sind weiterhin nicht abgenommen.** Auf dem Gerät stehen schnelle Pause-/Hintergrundwechsel, Unterbrechung unmittelbar vor einer Dialogzeile, Neustart/Kartenwechsel, Klang und Leistung aus. Die kontrollierten Java-Tests prüfen den Produktionsablauf, ersetzen aber keine Android-Ausführung.

```powershell
./build.ps1 -Target Check -OutputDirectory build/audio-lifecycle/check
./build.ps1 -Target Android -OutputDirectory build/audio-lifecycle/android -Abis arm64-v8a,x86_64
python scripts/Test-AudioPauseIngress.py --dll target/release/rc_native.dll --library tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll --package build/linux-sync-20261025/package/artifacts/mission1-scenes.rcpack.zip --fixture build/audio-lifecycle/check/java/mission-pause-ingress.bin --output build/audio-lifecycle/pause-ingress.json
```

Vorherige Lieferungen bleiben erhalten. Dieser Abschnitt erzeugt keinen Commit, Tag oder Push. Als Nächstes bleiben Geräteabnahme, optionale persistente Dialogfortsetzung und die Integration der allgemeinen Original-Audioorchestrierung.
