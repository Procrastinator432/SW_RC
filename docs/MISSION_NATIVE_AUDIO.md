# Nativer Audioanschluss der spielbaren Mission

**Folgestand 0.65.0:** [Dialogfortsetzung nach Pause](DIALOG_RESUME.md) ergänzt die hier beschriebene Ausgabe um RAM-Snapshots mit Quellenposition. Die folgenden Wiederaufnahmegrenzen beschreiben den historischen Stand 0.64.0; Prozessverlust bleibt auch im Folgestand ohne gespeicherte Dialogposition.

Stand: 26.10.2026. APK **0.64.0-mission-audio** verbindet erstmals die vorhandenen Missions-WAVs mit dem gemeinsamen Rust-/OpenAL-Worker und AudioTrack. Der Standardpfad der Mission verwendet diesen Anschluss; unter **Audioausgabe** bleibt SoundPool als ausdrücklich gewählter Vergleich verfügbar. Das vorhandene Mission-1-Szenenpaket wird weiterverwendet.

## Umsetzung

- **Missionsereignisse und Soundbank:** `PlayActivity` übergibt die unveränderte `RCSND001`-Bank und die konsumierten `RCAUD003`-Ereignisse an `NativeSceneAudio`. Die Bank wird einmal pro Ausgabesitzung geladen. Lautstärke/Pitch stammen aus den bestehenden Ereignisparametern. Ereignisabstände folgen den Simulationsticks; neu beginnende Sprache desselben Besitzers ersetzt dessen vorherige Sprachquelle.
- **Dauerquellen:** Die jeweils gewünschte Menge wird mit Sound, Besitzer, Aktivierungskennung, Position, Radius, Lautstärke und Pitch übertragen. Unveränderte Quellen behalten ihren AL-Source; Änderungen der Aktivierung oder das Verlassen der gewünschten Menge stoppen beziehungsweise ersetzen die Quelle. Vorhandene geometrische Radius-/Fade-Ergebnisse aus dem Spiel bleiben die Eingaben.
- **Physischer Listener:** `RCLOOP04` erweitert die bisherige schreibgeschützte Umgebungsschnittstelle um Kameraneigung. Die gewählte Präsentationskamera einschließlich Intro/Rappel liefert Position und Orientierung. Der Backend setzt tatsächlich `AL_POSITION` und `AL_ORIENTATION`; Pitch/Yaw ergeben orthonormale Vorwärts-/Aufwärtsvektoren. Die bisherige Achsenumordnung und der Faktor 0,01 werden für Quellen und Listener gemeinsam angewendet.
- **Lebenszyklus und Spielstände:** Pause, Fokusverlust, Neustart, Kartenladen und Activity-Ende verwerfen die Ausgabesitzung mit ihrer Queue. Fokusverlust pausiert die Mission. Eine neue Sitzung erhält eine neue native Kennung und eigene Mailbox; alte Produzenten/Tickets können keine neuen Pakete übernehmen. Nach kaltem Missionsrestore liefert der vorhandene Spielstand wieder die gewünschten Dauerquellen und ihre bereits gewählten Parameter. Bereits konsumierte Einzeleffekte werden nicht erneut zugestellt. Der Missionsspielstand bleibt alleinige Gameplay-Autorität; es gibt keinen zweiten, möglicherweise abweichenden Audio-Spielstand.
- **Kurzer laufender Pfad:** Der Missions-WAV-Adapter verwendet denselben Geräteexecutor/Backend und nativen Besitzerworker, serialisiert aber keinen vollständigen Hostcheckpoint pro Audioblock. Unveränderte Parameter werden nicht wiederholt hochgeladen. Vor dem Rendern wird Queueplatz reserviert. Das Softwarebudget beträgt 960 Frames / 20 ms bei 48 kHz; AudioTrack erhält ebenfalls einen angeforderten 20-ms-Puffer, mindestens jedoch den vom Gerät verlangten Mindestpuffer. Der Modus für geringe Latenz wird angefordert.

Die dafür vorhandenen 16 physischen Quellen, Besitzerersetzung und Loopreservierung bilden die bisherige Missionsausgabe ab. Dieser schnelle WAV-Anschluss ersetzt noch nicht jeden ursprünglichen allgemeinen FALSource-/Prioritäts-/Duckingpfad. Der allgemeine WAV-/Vorbis-Host bleibt als [separater gemischter Pfad](AUDIO_MIXED_LIVE.md) erhalten; Vorbis-Musik wird nicht durch die aktuellen WAV-Missionspakete transportiert. Es wird keine neue Zahl originalgetreu rekonstruierter Funktionen oder DSP-Bitparität behauptet.

## Klang- und Wiederaufnahmegrenzen

Räumliche Stereo-WAVs werden für die Punktquellenausgabe nach Mono gemischt. Nicht räumliche Stereoaufnahmen behalten ihre Kanäle. Die bisherige portable Grundlautstärke 0,35 bleibt erhalten; nicht räumliche Monoquellen erhalten eine Kompensation für OpenALs mittige Kanalverteilung. Das ist ein Plattformadapter und keine Behauptung identischer SoundPool-/Original-DSP-Ausgabe. Der physische Listener ersetzt die zuvor allein in Java berechnete Stereoverteilung; ein bestimmtes HRTF-Verfahren wird nicht vorausgesetzt.

Laufende Einzeleffekte/Dialogclips werden bei Pause verworfen; es gibt noch keine samplegenaue Fortsetzung eines halben Satzes. Dauerquellen starten nach Neuaufbau am Dateianfang mit derselben gespeicherten Aktivierung und Parameterauswahl. Ein gehaltener Bacta-Benutzungsvorgang wird durch die vorhandene Gameplay-Suspendregel beendet; dessen Ton darf deshalb beim Pausieren verschwinden. Dies wird von dauerhaften Weltquellen unterschieden.

Die Softwarequeue ist begrenzt, Androids Track-/Gerätepuffer kommt hinzu. Angeforderte Puffergröße und Leistungsmodus garantieren keine bestimmte Endlatenz. Bei vollständig von Dauerquellen belegten 16 Slots kann ein weiterer Einzeleffekt nicht aufgenommen werden; im vollständigen geprüften Mission-1-Ablauf waren höchstens drei Dauerquellen gleichzeitig ausgegeben. Im A-Mix wurde S16-Vollaussteuerung 32.767 erreicht. Pegel, Überlagerungen, Knackser, tatsächliche Latenz und Leistung benötigen die spätere Hör-/Geräteabnahme.

## Prüfungen

Der vollständige aktuelle Windows-C-ABI-Durchlauf verwendet echte Szenen, echte Spielereingaben und den tatsächlichen OpenAL-Mixer. Kein Gegnerstatus, Ziel, Spielerstand oder Abschluss wird für den Test erfunden. Die drei gespeicherten Routen werden bis zum Abschluss gespielt, anschließend erfolgt der tatsächliche Übergang zur nächsten Karte. Der Fabrikeinstieg wird fünf Sekunden weitergeführt.

| Karte | Normale Eingaben | Zugestellte Ereignisse | WAVs in der Bank | Kalter Neuaufbau |
|---|---:|---:|---:|---|
| GEO_01A | 19.319 | 461 | 102 | Ja, mit erhaltenen Weltquellen |
| GEO_01B | 17.546 | 476 | 42 | Ja |
| GEO_01C | 12.630 | 293 | 56 | Ja |
| GEO_03A, Einstieg | 300 | 0 | 10 | Ja |

Insgesamt **49.795 Eingaben, 1.230 Ereignisse, 39.836.000 Stereo-Frames, drei Kartenübergänge und acht Audio-Ausgabesitzungen**. Die Kamerapose und gewünschten Quellen sind unmittelbar nach dem Restore identisch. Die anschließende normale Gameplay-Pause darf nur den Bacta-Benutzungston beenden; persistente Weltquellen bleiben unverändert. Alte konsumierte Einzeleffekte erscheinen nach dem Restore nicht erneut. Die Szene-/Soundbankidentität stimmt auch nach jedem Neuaufbau. Die Fabrik liefert im geprüften Einstieg keine Audioereignisse; ihr niedriger RMS um 0,5 ist der OpenAL-Ditherboden, kein neu erfundener Ton.

Weitere Nachweise:

- Elf neue Rust-Gruppen für Bank-/Paketgrenzen, atomare Vorprüfung, Timeline/Parameter, Sitzungsgrenzen, Loopidentität, räumliche Listenerpose, Wiederaufbau, Sprachersetzung, Fehler und tatsächliche Links/Rechts-Ausgabe sowie natürliche WAV-Enden.
- Zwei neue native ABI-Gruppen für Sitzungsersetzung, veraltete Epochen/Kennungen, falschen Hosttyp und ungültigen Bankstart.
- Fünf neue reine Java-Mailboxgruppen und ein zusätzlicher Pump-Test für das kurze Missionsbudget. Insgesamt **30 PCM-/Mailbox-Javaprüfgruppen**.
- Je **1.437 Debug-/Release-Bibliothekstests**, Doctests, Materialimport, Clippy, Java- und Buildvertrag im Zentralcheck.
- 36 ältere Spielstände aus den Ständen 0.52–0.60 erneut exakt wiederhergestellt, dazu neun unveränderte Einstiegskameraframes.
- APK für arm64 und x86_64 gebaut; ELF-/APK-Nachweise prüfen auch die neuen Missions-C-/JNI-Exporte und Klassen. Native Bibliotheken sind nach Gradles Strip-Stufe bytegleich im APK und mindestens auf 16 KiB ausgerichtet.

[Zentralcheck](../build/mission-native-audio/check/validation.json) · [Android-Bau](../build/mission-native-audio/android/validation.json) · [Vollständiger nativer Missionslauf](../build/mission-native-audio/campaign-final/mission-audio.json) · [Alte Spielstände](../build/mission-native-audio/legacy/validation.json) · [Gesamtnachweis](../build/mission-native-audio/verification-summary.json)

Der Laufzeitnachweis betrifft Windows-C-ABI und OpenAL. Android-JNI, Audiofokus, Activity-/Prozesswechsel und tatsächliche AudioTrack-Ausgabe wurden wie vereinbart **noch nicht am Emulator oder Handy ausgeführt**. Diese Lieferung ist dafür vorbereitet, keine behauptete Geräteabnahme.

## Lieferung und spätere Abnahme

1. [APK 0.64.0 installieren](../build/mission-native-audio/android/artifacts/republic-commando-0.64.0-mission-audio.apk). Das bereits importierte Szenenpaket und die vorhandenen Missionsspielstände können weiterverwendet werden.
2. Im Startmenü unter **Audioausgabe** den **Nativen Mixer** wählen; dies ist bei fehlender Einstellung der Standard. Zum Vergleich kann dort **SoundPool zum Vergleichen** gewählt werden. Die Einstellung gilt beim nächsten Laden der Mission.
3. Mission normal starten/fortsetzen. Waffen/Foley, Dialoge, Bacta und Weltquellen prüfen. Den Blick seitlich und nach oben/unten drehen, anschließend pausieren, in den Hintergrund wechseln und einen Checkpoint kalt laden. Quellen dürfen weder doppelt anlaufen noch nach einem Kartenwechsel aus der alten Karte weiterklingen.
4. Den Ablauf bis zum Fabrikeinstieg spielen und Klang, Pegel, Reaktionszeit, Aussetzer und Bildleistung bewerten. Die bisherigen SoundPool-spezifischen Logassertionen sind nur auf den Vergleichsbackend anzuwenden; sie sind kein Nachweis des neuen nativen Pfads.

```powershell
./build.ps1 -Target Check -OutputDirectory build/mission-native-audio/check
./build.ps1 -Target Android -OutputDirectory build/mission-native-audio/android -Abis arm64-v8a,x86_64
python scripts/Test-MissionNativeAudio.py --dll target/release/rc_native.dll --library tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll --package build/linux-sync-20261025/package/artifacts/mission1-scenes.rcpack.zip --inputs build/linux-sync-20261025/complete/scenes/resolved-inputs.json --output build/mission-native-audio/campaign-final
```

Der native Prüfer bindet die Replaydateien über ihre manifestierten Hashes und liest das explizit übergebene Szenenpaket. Die kurze Android-Pufferkonfiguration verwendet den offiziellen [AudioTrack-Buildervertrag](https://developer.android.com/reference/android/media/AudioTrack.Builder#setPerformanceMode(int)); die tatsächliche Unterstützung ist geräteabhängig.
