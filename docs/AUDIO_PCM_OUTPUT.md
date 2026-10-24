# PCM-Ausgabe und Android-Diagnose

Stand: 26.10.2026. Anschluss an den [Windows-OpenAL-Loopbackbackend](AUDIO_OPENAL_BACKEND.md). Der neue Android-Ausgabepfad nimmt dessen bereits gerendertes Stereo-PCM entgegen. Die App enthält einen separaten Diagnosebildschirm; der Missionssound verwendet weiterhin den bestehenden SoundPool-Host. OpenAL selbst läuft bisher auf Windows, nicht im Android-Prozess.

## Fünf umgesetzte Punkte

1. **Rust-/Java-Transport.** `audio_pcm::Packet` übernimmt `audio_device::Rendered` und kodiert Stereo-S16 in Little Endian. Java prüft dieselbe Struktur. Eine gemeinsam verwendete synthetische Fixture sichert den Sprachübergang ab. Dateiformat, Paketlängen, Rate, Kanäle, Zähler, Restbytes und Größenlimits werden geprüft.
2. **Begrenzte Ausgabequeue.** `PcmPump` besitzt höchstens 16 Pakete und 250 ms noch nicht bestätigte PCM-Frames. Aufnahme liefert bei vollem Puffer sofort `false`; der Diagnoseproduzent wartet außerhalb des UI-Threads. Paketdaten werden kopiert, Folgenummern und Sitzungsepochen müssen passen. Bestätigte Teilübertragungen geben nur die tatsächlich übertragenen Frames frei.
3. **AudioTrack mit eigenem Worker.** Erzeugung, Play, Pause, Flush, Schreiben, Abfragen und Release geschehen auf einem Besitzerthread. Nichtblockierende Schreibaufrufe dürfen 0 oder vollständige Stereo-Frames zurückgeben. Negative, zu große oder ungerade Antworten sperren die Ausgabe und geben das Gerät frei. Ein defekter Track wird nicht mit bereits übertragenen Daten neu gestartet; ein ausdrücklicher Start erzeugt eine neue Ausgabeinstanz.
4. **Lebenszyklus und Fokus.** Pause erhöht die Epoche, verwirft die Queue und macht alte Bestätigungen wirkungslos. Vor dem Schreiben der nächsten Epoche wird der Track pausiert und geleert, auch wenn Pause und Start zwischen zwei Workerbeobachtungen lagen. Audiofokusverlust, Activity-Pause und Schließen stoppen die Diagnose. Fokusgewinn startet sie nicht automatisch neu. Nach normalem Ende wartet der Worker auf den Playback-Head; die Activity gibt anschließend den Fokus frei.
5. **Prüfbare Diagnose und Pakete.** `rc-audio-pcm-package` verpackt die vorhandenen echten OpenAL-Mischungen. Ein unabhängiger Pythonvergleich prüft jedes PCM-Byte gegen die WAV-Quelle; Java liest beide vollständigen Container. Die App 0.61.0 enthält den Einstieg **PCM-Ausgabe prüfen** und einen Dateidialog für diese einmaligen Diagnoseimporte.

Die 250 ms begrenzen ausschließlich die Softwarequeue. AudioTrack besitzt zusätzlich einen vom System abhängigen Trackpuffer, mindestens den von Android verlangten Mindestpuffer beziehungsweise angeforderte 100 ms. Am Dateiende wird höchstens ein solcher Puffer Stille nachgeschoben, damit auch kurze Clips die Startschwelle erreichen; der Abschluss wartet nur auf die eigentlichen Nutzframes. Das ist keine Zusage einer maximalen Gesamtlatenz. Die Diagnose lädt eine Datei bis 16 MiB in den Speicher; der Dateivorrat zählt nicht zum laufenden Queuebudget.

## Format

| Offset | Feld |
|---|---|
| 0 | 8 Byte `RCPCM001` |
| 8 | u32 Samplerate, 8.000–192.000 Hz |
| 12 / 14 | u16 Kanäle = 2 / u16 Bits = 16 |
| 16 / 24 | u64 Epoche / Folgenummer; gemeinsamer positiver Java-Zählerbereich |
| 32 / 36 | u32 Frames / Payloadbytes = Frames × 4 |
| 40 | 1–4.096 interleavte Stereo-Frames, S16 LE |

Epoche muss größer 0 sein; Folgenummer beginnt bei 0 und bleibt kleiner `i64::MAX`. Ein Diagnosecontainer beginnt mit `RCPCMSET`, u32 Rate und u32 Paketanzahl, gefolgt von wiederholten u32 Paketlängen und den Paketen. Er enthält Epoche 1 und lückenlose Folgenummern. Beim Abspielen werden diese Pakete auf die aktuelle Ausgabesitzung umgeschrieben. Ein Container ist **kein Mixercheckpoint**; erneuter Diagnosestart beginnt am Dateianfang.

## Nachweise und Wiederholung

- Vier Rust-Testgruppen für Roundtrip/Fixture, beschädigte Daten, Grenzen/Zähler und Containerbildung.
- Vier Java-Paketgruppen und elf Workergruppen, einschließlich vollem Frame-/Paketbudget, Teil-/Nullschreiben, Pause während eines Schreibens, Epochenschranke, verzögertem Drain, Fehlern und einmaligem Release.
- Je **1.398 Debug-/Release-Bibliothekstests**, dazu Doctests, Materialimporttests, Clippy, vorhandene Java- und Buildvertragsprüfungen im Zentralcheck.
- Zwei reale OpenAL-Dateimischungen mit je 380 Paketen, 182.400 Frames, Stereo-S16/48 kHz und 3,8 Sekunden; Nutzbytes gegenüber der jeweiligen WAV unverändert.
- APK-Build für **arm64-v8a und x86_64** einschließlich der bestehenden GPU-Vertragsprüfung. Dies ist ein Bau- und Hostnachweis, kein Android-Laufzeitnachweis.

```powershell
./build.ps1 -Target Check -OutputDirectory build/audio-pcm-sink/check
./build.ps1 -Target Android -OutputDirectory build/audio-pcm-sink/android -Abis arm64-v8a,x86_64
cargo run --locked --offline -p rc-inspect --release --bin rc-audio-pcm-package -- build/audio-openal/real-a.wav build/audio-pcm-sink/real-a.rcpcm
python scripts/Test-PcmTransport.py build/audio-openal/real-a.wav build/audio-pcm-sink/real-a.rcpcm --report build/audio-pcm-sink/real-a.validation.json
```

[Zentralcheck](../build/audio-pcm-sink/check/validation.json) · [Android-Bau](../build/audio-pcm-sink/android/validation.json) · [Gesamtnachweis](../build/audio-pcm-sink/verification-summary.json)

## Für den späteren Handytest

1. [APK 0.61.0 installieren](../build/audio-pcm-sink/android/artifacts/republic-commando-0.61.0-pcm-output.apk).
2. [Testmix A](../build/audio-pcm-sink/real-a.rcpcm) und [Testmix B](../build/audio-pcm-sink/real-b.rcpcm) auf das Gerät kopieren.
3. Im Hauptmenü **PCM-Ausgabe prüfen**, Datei importieren und **Testmix starten** wählen. Der Testmix besitzt bewusst leise/pausierte Abschnitte aus der bestehenden OpenAL-Prüfung.
4. Vollständiges Ende, wiederholten Start, Stopp, Hintergrund/Vordergrund, Audiofokusverlust und Geräteausgabe prüfen. Danach die bestehende Mission separat prüfen.

Dieser Handy-/Emulator-/Hörtest wurde auf Nutzerwunsch noch nicht durchgeführt. Ältere abgenommene APKs und Szenenpakete bleiben erhalten.

## Nächster Anschluss

OpenAL für Android bauen, Render-/Quellenhost über einen gebundenen nativen Besitzerthread und JNI anschließen, Queuekapazität vor dem Rendern reservieren und den Szenen-/Ereignisstrom versorgen. Erst danach den Missionssound gezielt auf den neuen Pfad umstellen. Der jetzige Dateiproduzent reserviert keine Live-Renderzeit und hält keine Engine-Uhr an. Prozesswiederherstellung und Mixerrekonstruktion müssen später gemeinsam orchestriert werden; der Ausgabepuffer wird absichtlich nicht als samplegenauer Spielstand behandelt. Keine neuen Originalfunktionen oder DSP-Bitparität aus diesem Adapterblock ableiten.

API-Grundlagen: [AudioTrack: Teilübertragungen, Fehler und Flush](https://developer.android.com/reference/android/media/AudioTrack), [Android: Audiofokus anfordern und abgeben](https://developer.android.com/media/optimize/audio-focus). Der Adapter verwendet APIs, die für das bestehende minSdk 26 verfügbar sind; aktuelle APIs oberhalb compileSdk 35 werden nicht vorausgesetzt.

## Nachfolgender nativer Live-Anschluss

Der [native Android-Live-Diagnosepfad](AUDIO_NATIVE_LIVE.md) baut OpenAL für beide Android-Architekturen und verbindet Rust-Besitzerworker, JNI, Reservierung und AudioTrack. Eine WAV kann damit direkt im Android-Prozess gerendert werden. Build-/Hostnachweise bestehen; Android-Laufzeit und Klang sowie der allgemeine Szenen-/Missionshostanschluss bleiben offen.
