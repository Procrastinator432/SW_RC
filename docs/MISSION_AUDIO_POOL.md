# Poolbereinigung, Fehlergrenzen und Diagnose im Missionsaudio

**Folgestand 0.70.0:** [Optionale Audioergänzung am Missionscheckpoint](AUDIO_CHECKPOINT.md) ermöglicht nach abgeschlossener Pause einen gebundenen kalten Dialogrestore. Die folgenden RAM-Grenzen und Zahlen beschreiben 0.69.0.

Stand: 26.10.2026. **0.69.0-audio-pool** verbessert den vorhandenen 16-Quellen-Missionsadapter. Die Auswahl nach Alter und reservierte Dauerquellen bleiben erhalten. Dieser Abschnitt fügt keine angenommene Originalpriorität hinzu.

## Fünf umgesetzte Punkte

1. **Beendete Quellen vor Aufnahme bereinigen.** Tatsächliches `AL_STOPPED` wird jetzt sowohl vor der Ereignisausgabe als auch vor einem gültigen Zustands-/Loopupdate ausgewertet. Beendete Einzeleffekte werden gestoppt, vom Puffer gelöst und aus der Slotbelegung entfernt. Dadurch kann eine neue Dauerquelle einen bereits fertigen Slot nutzen, statt eine noch laufende ältere Stimme zu verdrängen. Ein Abschluss wird nur einmal gezählt. Ungültige Eingabepakete lösen weiterhin vor der Validierung keine IO aus.
2. **Kapazität von IO-Fehlern trennen.** Die Slotwahl liefert `Some(slot)`, `None` für ausschließlich durch Dauerquellen reservierte Kapazität oder einen echten Fehler. Der Renderer ignoriert nicht mehr pauschal das gesamte Fehlerergebnis von `slot()`. Bei 16 gewünschten Dauerquellen darf ein Einzeleffekt kontrolliert entfallen; das ist keine kaputte Sitzung. Ein Fehler beim Stoppen oder Lösen einer verdrängten Quelle wird dagegen weitergereicht.
3. **Fehlerhafte Ausgaben konsequent sperren.** Ein Fehler bei Statusabfrage, Stoppen oder Pufferfreigabe während Bereinigung/Aufnahme sperrt die betreffende Szene. Weiteres Update/Rendern und ein neuer Pausensnapshot werden abgewiesen. Bereits ausgeführte physische Aufrufe werden nicht als zurückgerollt ausgegeben. Der bestehende Java-Fehler-/Cachepfad übernimmt den Abbruch; Freigabe bleibt auf dem Besitzerproduzenten. Ein Fehler beim Lesen/Protokollieren des neuen Status verhindert die anschließende native Freigabe nicht.
4. **Belegung und Entscheidungen messbar machen.** Das feste 88-Byte-Paket `RCPOOL01` liefert Kapazität, verfolgte aktive/einmalige/Loopslots, wartende Ereignisse, Fehlersperre, bestätigte Renderframes und letzten angenommenen Ereigniszeiger. Fünf Zähler unterscheiden erfolgreich aufgenommene Einzeleffekte, NoOverride-Ablehnungen, Kapazitätsablehnungen, erfolgreiche Altersverdrängungen und bereinigte natürliche Abschlüsse. Zähler bleiben über RAM-Restore erhalten; ältere Snapshots ohne Statistik starten mit null. Sie sind auf den positiven Java-Long-Bereich begrenzt. `rc_scene_audio_status`/JNI lesen ausschließlich gespeicherten Zustand, ohne Geräteabfrage, Bereinigung oder Rendertakt. Beim Schließen einer Android-Ausgabesitzung wird ein kompakter Status mit Sitzungsnummer geloggt; es gibt keine neue Abfrage pro Audioblock.
5. **Sättigung und Fehlerfolgen absichern.** Zwölf neue Rust-Gruppen, eine native ABI-Gruppe und vier Java-Gruppen prüfen Belegung, Abschlusswiederverwendung, einmalige Ablehnung, erhaltene Alterswahl, kontrollierte Stop-/Unbind-/Queryfehler, lesbaren Fehlerstatus, konstante Statusgröße, IO-freie Abfragen, alte/neue Snapshots, Zählergrenzen und NoOverride-Zählung. Ein zusätzlicher Windows-Prüfer belastet den tatsächlichen OpenAL-Pool mit realen Missions-WAVs.

## Bedeutung der Diagnose

Die Belegung ist die **vom Adapter zuletzt abgeglichene Slotbelegung**. Ein gerade physisch beendeter Clip kann bis zum nächsten gültigen Update/Render noch darin stehen. Die Statusabfrage bereinigt ihn bewusst nicht. Nach einem Teilfehler kann die tatsächliche Treiberbelegung von diesem letzten bestätigten Zustand abweichen; das Fehlerbit zeigt die gesperrte Sitzung an. Die Paketfelder sind keine Behauptung eines unmittelbar abgefragten Hardware-Spielzustands.

`admitted` zählt erfolgreiche Ereignisstarts, keine Dauerquellen oder bloße Wiederherstellung schon gezählter Dialoge. `preempted` zählt den erfolgreich gestoppten und vom Puffer gelösten Alterskandidaten, auch wenn ein anschließender neuer Start scheitern sollte. Bei einem Fehler zwischen Stop und Unbind bleibt dieser bestätigte Verdrängungszähler unverändert; die Sitzung ist dann gesperrt. Gezielte Ersetzung einer ungeschützten Zeile desselben Besitzers ist keine Altersverdrängung. `completed` zählt bei der Bereinigung beobachtete natürliche Abschlüsse; externe Stopps, Kartenwechsel und verworfene Effektenden werden nicht nachträglich als natürliche Abschlüsse erfunden. Zähler sind Diagnosedaten und ändern keine Gameplayentscheidung.

Die ursprüngliche allgemeine Priorität, vollständige `FindSource`-/Queuesuche, Ducking und Vorbis-Musik bleiben offen. Der ausgewählte [NoOverride-Zweig](MISSION_VOICE_OWNER.md) bleibt erhalten; ein geschützter Sprachbesitzer hat weiterhin keinen generellen Schutz gegen die bestehende Altersverdrängung bei vollem Pool. Neue Zahlen dekompilierter Originalfunktionen oder Original-DSP-Parität werden nicht behauptet.

## Prüfungen

Der Zentralcheck umfasst je **1.478 Debug-/Release-Bibliothekstests**, Materialimport, Doctests, Clippy, Java und Buildvertrag. Die Java-Audioprüfung umfasst jetzt **56 Gruppen**. Der ABI-Test prüft Größenabfrage, zu kleinen Puffer, wiederholte Abfrage ohne Clockfortschritt, falschen Hosttyp, veraltete Kennung und anschließenden gültigen PCM-Start.

Der tatsächliche Windows-Poolprüfer erzeugt kontrollierte Audioanforderungen mit der echten GEO_01A-WAV-Bank:

- 16 aktive Stimmen und eine 17. Anforderung: genau der bisherige älteste Kandidat wird verdrängt und einmal gezählt.
- 16 reservierte Dauerquellen: eine Einzeleffektanforderung wird ohne Sitzungsfehler einmal abgewiesen; nach Freigabe eines Slots funktioniert die nächste Aufnahme.
- Kontextrestore: die Zähler bleiben erhalten; Dauerquellen warten wie zuvor auf den aktuellen Spielzustand.
- 15 längere Originalclips plus ein kürzerer Originalclip: direkt nach dessen physischem Ende wird ein neuer Loop aufgenommen. Der fertige Slot wird wiederverwendet, der noch laufende älteste Dialog bleibt erhalten, `completed` steigt einmal und `preempted` bleibt null.
- Wiederholte Statusabfragen ändern weder Snapshot noch Quellenposition oder Zähler.

Das sind kontrollierte Audioszenarien, keine manipulierten Missionsziele oder ein freier Handytest. Unsichere IO-Fehler werden im kontrollierten Rustbackend injiziert; der tatsächliche OpenAL-Test erfindet keinen Gerätedefekt.

Mit der neuen DLL werden außerdem Sprachbesitzprüfung, Mischpegel/26 OpenAL-Sitzungen, Pausenübergabe, Dialogrestore, vollständiger A/B/C→Fabrik-Lauf und 36 ältere Spielstände/neun Kameraframes erneut geprüft.

[Zentralcheck](../build/audio-pool/check/validation.json) · [Tatsächlicher Poolsättigungstest](../build/audio-pool/pool.json) · [Sprachbesitz](../build/audio-pool/voice-owner.json) · [Pegel](../build/audio-pool/mix.json) · [Dialogrestore](../build/audio-pool/dialog-resume.json) · [Pausenübergabe](../build/audio-pool/pause-ingress.json) · [Vollständige Mission](../build/audio-pool/campaign/mission-audio.json) · [Ältere Spielstände](../build/audio-pool/legacy/validation.json) · [Android-Build](../build/audio-pool/android/validation.json) · [Gesamtnachweis](../build/audio-pool/verification-summary.json)

## Lieferung und Grenzen

[APK 0.69.0 für arm64 und x86_64](../build/audio-pool/android/artifacts/republic-commando-0.69.0-audio-pool.apk) · [Quellstand und Artefaktzuordnung](../build/audio-pool/source-and-verification.json)

APK-SHA-256: `4c6d4025ff51b5955cf59371764923c693170a19a4e6637778667ae33b58fa9e`.

Das vorhandene Szenenpaket, die Missionscheckpoints und die Lautstärkeregler können weiterverwendet werden. APK/ELF-Struktur werden einschließlich Status-C-/JNI-Export und Java-Parser für beide Architekturen geprüft. **Android-Laufzeit, Gerätefehler und Hören bleiben offen.** Der neue `RCNativeAudio`-Logeintrag `pool session=…` erleichtert die spätere Zuordnung von Aussetzern zu Kapazität oder Ersetzung; er ersetzt keine Hörabnahme.

RAM-Dialogfortsetzung bleibt auf die Rendergrenze beschränkt, Prozessverlust speichert keine Dialogposition. SoundPool bleibt der bisherige Vergleichspfad. Frühere APKs bleiben erhalten; kein Commit, Tag oder Push wird erzeugt.

```powershell
./build.ps1 -Target Check -OutputDirectory build/audio-pool/check
./build.ps1 -Target Android -OutputDirectory build/audio-pool/android -Abis arm64-v8a,x86_64
python scripts/Test-MissionAudioPool.py --dll target/release/rc_native.dll --library tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll --package build/linux-sync-20261025/package/artifacts/mission1-scenes.rcpack.zip --output build/audio-pool/pool.json
```
