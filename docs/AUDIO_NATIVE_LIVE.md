# Nativer Android-Live-Mixer bis zur PCM-Ausgabe

Stand: 26.10.2026. Fortsetzung der [PCM-Ausgabediagnose](AUDIO_PCM_OUTPUT.md). Der neue Diagnosepfad kann eine importierte WAV **im Android-Prozess mit OpenAL Soft rendern** und über Rust-Pakete, reservierte Queueplätze und AudioTrack ausgeben. Er benötigt keine vorher gerenderte `.rcpcm`-Datei. APK 0.62.0 ist für arm64 und x86_64 gebaut; Android-Laufzeit und Klang sind noch nicht abgenommen.

## Fünf umgesetzte Punkte

1. **OpenAL für Android.** Die offiziellen Quellen 1.25.2 sind per SHA-256 gebunden. `Build-OpenALAndroid.ps1` baut `libopenal.so` für API 26 mit NDK r27.2.12479018 und beiden ABIs. Hardwarebackends, EAX und eingebettete HRTF-Daten sind ausgeschaltet; der Loopbackmixer bleibt verfügbar. C++-Abhängigkeiten werden statisch in diese separate Bibliothek eingebunden. ELF-Loads sind auf mindestens 16 KiB ausgerichtet. Lizenztexte, Quellenadresse und Baurezept liegen auch im APK-Assetsverzeichnis.
2. **Gemeinsamer Rust-Backend.** `audio_openal` verwendet auf Windows weiterhin den expliziten DLL-Loader, auf Android `dlopen`/`dlsym` mit einem aufgelösten Pfad in das eigene `nativeLibraryDir`. OpenAL bleibt über threadlokale Kontexte an seinen Besitzer gebunden. Das APK lässt native Bibliotheken extrahieren, damit dieser Pfad vorhanden ist. Alle bisherigen AL-Aufträge und Renderoperationen bleiben gemeinsam.
3. **Nativer Besitzer und JNI.** `rc_live_open/render/close` vermitteln an einen eigenen Rust-Worker. Dort erfolgen OpenAL-Erzeugung, Upload, Rendern und Freigabe. Der Java-Produzent besitzt ausschließlich eine eindeutige Kennung; fremde Aufruferthreads, alte Kennungen und doppelte Freigaben werden abgewiesen. Höchstens 16 Sitzungen und ein wartender Auftrag pro Worker sind erlaubt. Ein verlassener Aufrufer signalisiert im TLS-Abschluss nur nichtblockierend das Ende; der OpenAL-Kontext wird auf dem Worker vor dessen Threadabschluss zerstört.
4. **Reservierung vor dem Audiotakt.** Die Java-Queue vergibt einmalige Tickets für Epoche, Folgenummer und genaue Framezahl. Ein Produzent kann vor dem Rendern höchstens einen Platz reservieren. Andere Aufnahmen dürfen diesen Platz nicht überbuchen. Ein falsches Renderresultat sperrt die Ausgabe; Pause/Schließen entwerten Reservierungen. Bei vollem Puffer schreitet der native Audiotakt nicht fort. Fehler führen zur Freigabe statt zum Wiederholen bereits gerenderter Frames.
5. **Live-Diagnose.** Im Hauptmenü führt **Nativen Mixer prüfen** zu einem WAV-Import. Start öffnet den tatsächlichen nativen Mixer und wiederholt die Quelle fünf Sekunden lang. Der Hintergrundproduzent liefert Blöcke von 480 Frames bei 48 kHz. Ende, Fokusverlust, Hintergrundwechsel und Stopp verwenden den vorhandenen Ausgabepfad; ein neuer Start erzeugt eine neue Sitzung und beginnt am Anfang.

Die WAV-Sitzung verwendet den bestehenden Geräteexecutor, eine Quelle und einen Buffer. Sie ist ein begrenzter Integrationsnachweis. Der allgemeine gemischte Quellenhost, Vorbis-Worker, Szenenereignisse, räumliche Parameter und Missionscheckpoint-Wiederaufbau sind hier noch nicht angeschlossen. Der Missionssound bleibt beim bisherigen SoundPool-Host. Der native Teil enthält keinen nachgebauten DSP-Mixer: Das tatsächliche Mischen und Resampling übernimmt OpenAL Soft.

## Gefundene und behobene Fehler

- Eine leere Queue unmittelbar nach Start konnte im bisherigen PCM-Worker als Wiedergabeende gelten. Drain wird jetzt erst nach ausdrücklichem `finish` geprüft. Ein Test hält `play` gezielt an, reicht danach das erste Paket ein und prüft dessen vollständige Ausgabe.
- Die erste direkte TLS-Besitzerlösung blieb im Windows-Test beim Threadabschluss mit offenen OpenAL-Ressourcen hängen. Die überarbeitete Lösung hält den Kontext auf einem dedizierten Rust-Worker. Der TLS-Abschluss des Aufrufers signalisiert nur das Ende und führt weder Join noch DLL-Freigabe aus. Parallele Sitzungstests und der Abbruch ohne ausdrückliches Close bestehen damit.
- OpenALs C++20-Modulpfad schlug mit der vorhandenen NDK-/Clang-Kombination fehl. Die vom Projekt angebotene Option `ALSOFT_ENABLE_MODULES=OFF` behebt den Bau ohne Änderung der OpenAL-Quellen.

## Prüfungen

- Sieben neue Rust-Kerngruppen: exakte Pakete, Auftragsvorprüfung ohne Taktfortschritt, Fünfsekundenbudget, ungültige Eingaben, terminale Backendfehler, falsche Rate und tatsächliches OpenAL-Rendern.
- Sechs native Windows-C-ABI-/Besitzergruppen: zu kleine Ausgabepuffer, fremde Threads, doppelte/alte Kennungen, begrenzte Fehlermeldungen, exaktes Ende und verlassene Aufrufer.
- Sechs neue Java-Live-Gruppen sowie der zusätzliche Start-Regressionsfall. Insgesamt vier Paket-, zwölf Pump- und sechs Live-Produzentengruppen im Zentralcheck.
- Je **1.411 Debug-/Release-Bibliothekstests**, Doctests, Materialimport, Clippy, Java- und Bauvertragsprüfungen.
- Zwei echte Original-WAVs über den Windows-C-ABI-Pfad mit tatsächlichem OpenAL gerendert: jeweils 500 Pakete / 240.000 Stereo-Frames / fünf Sekunden bei 48 kHz. Java liest die vollständigen Container. Dies prüft den Hostpfad; die Android-JNI-Brücke wird dabei nicht ausgeführt.
- APK mit beiden ABIs gebaut. ELF-/APK-Prüfung kontrolliert Architektur, 16-KiB-Ausrichtung, benötigte OpenAL-/Rust-/JNI-Exporte, bytegleiche Bibliotheken aus Gradles Strip-Stufe im APK, enthaltene Diagnoseklassen und Lizenztexte. Dies ersetzt keine Android-Ausführung.

[Zentralcheck](../build/audio-native-live/check/validation.json) · [Android-Bau](../build/audio-native-live/android/validation.json) · [ELF-/APK-Vertrag](../build/audio-native-live/android-libraries.json) · [Gesamtnachweis](../build/audio-native-live/verification-summary.json)

## Später am Gerät prüfen

1. [APK 0.62.0 installieren](../build/audio-native-live/android/artifacts/republic-commando-0.62.0-native-mixer.apk).
2. Im Hauptmenü **Nativen Mixer prüfen**, [WAV A](../build/audio-native-live/input-a.wav) oder [WAV B](../build/audio-native-live/input-b.wav) importieren und starten. Beide Dateien sind private Auszüge aus der lokalen Spielinstallation; sie gehören nicht zu den versionierten synthetischen Fixtures.
3. Fünfsekundenende, wiederholten Start, Stopp während Ausgabe, Fokusverlust und Hintergrund/Vordergrund prüfen. Eine unterbrochene Diagnose startet beim nächsten Start am Dateianfang.
4. Anschließend den vorhandenen Missionspfad gesondert auf Regressionen prüfen. Laufzeit, Klang, tatsächliche Latenz und Geräteunterstützung sind ausdrücklich offen.

Die vorhandene PCM-Dateidiagnose bleibt ebenfalls erreichbar. Das Softwarebudget von 250 ms und 16 Paketen schließt zusätzlich reservierte Frames ein; Androids Trackpuffer und Dateivorrat kommen weiterhin hinzu. Diese Diagnose unterstützt PCM-WAV bis 2 MiB mit Mono/Stereo und 8/16 Bit, keine beliebigen WAV-Codecs. Ihr Quellenvorrat wird vollständig geladen. Es gibt keine samplegenaue Prozesswiederaufnahme. Gradle entfernt vor dem Paketieren Debugsymbole; die geprüften APK-Bibliotheken werden deshalb mit der abschließenden Strip-Stufe verglichen, nicht mit den ungestrippten Linkerdateien.

## Reproduktion und Abhängigkeit

```powershell
# Einmalig mit Netzwerk, danach reichen die geprüften lokalen Quellen:
./scripts/Build-OpenALAndroid.ps1 -Offline:$false
./build.ps1 -Target Check -OutputDirectory build/audio-native-live/check
./build.ps1 -Target Android -OutputDirectory build/audio-native-live/android -Abis arm64-v8a,x86_64
python scripts/Test-NativeLiveAudio.py target/release/rc_native.dll tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll build/audio-buffer/real-gen/00.wav build/audio-native-live/host-a.rcpcm build/audio-native-live/host-a.json
```

Das Android-Bauskript ruft den OpenAL-Bauer automatisch auf. Er benötigt CMake und Ninja auf PATH oder im vorhandenen Visual-Studio-Werkzeugbestand; der direkte Aufruf akzeptiert auch `-Cmake` und `-Ninja`. Geprüft wurde CMake 4.1.1-msvc1. Unveränderte Quelldateien behalten beim Wiederherstellen aus dem geprüften Archiv ihre Zeitstempel, damit der Buildcache nutzbar bleibt. Es erfolgt keine Systeminstallation.

Quelle: [OpenAL Soft 1.25.2](https://github.com/kcat/openal-soft/tree/1.25.2), Archiv-SHA-256 `47e22c066dffa2f65a1747272978344db845dcef9c7ee118449ec560a44adc8f`. Baugrundlagen: [offizielle NDK-CMake-Toolchain](https://developer.android.com/ndk/guides/cmake), [native APK-Paketierung](https://developer.android.com/reference/tools/gradle-api/8.9/com/android/build/api/dsl/JniLibsPackaging).

## Nächster Abschnitt

Den allgemeinen gemischten WAV-/Vorbis-Host auf den nativen Worker setzen, echte Szenenereignisse und räumliche Updates über eine begrenzte Befehlsqueue versorgen sowie Kartenwechsel und Checkpoint-Wiederaufbau verbinden. Danach einen ausgewählten Missionssound umstellen und beide Pfade gezielt vergleichen. Der jetzige Adapterblock erhöht keine Anzahl originalgetreu nachgebauter Spielfunktionen.

## Nachfolgender gemischter Anschluss

Der [gemischte native Host](AUDIO_MIXED_LIVE.md) verbindet jetzt den allgemeinen WAV-/Vorbis-Besitzer mit dem Worker, begrenzten Befehlen und kalter Audiozustandswiederherstellung. Android-Diagnose 0.63.0 ist gebaut und der Windows-C-ABI-Pfad tatsächlich geprüft. Der Missionsereignisstrom, vollständige räumliche Listeneranschluss und Android-Laufzeit-/Hörabnahme bleiben offen.

## Nachfolgender Missionsanschluss

Der [Missions-WAV-Pfad 0.64.0](MISSION_NATIVE_AUDIO.md) verbindet nun die vorhandene Spielsoundbank und Ereignisse mit dem gemeinsamen nativen Worker, physischer Listenerpose und Karten-/Checkpointlebenszyklus. Er vermeidet vollständige Hostserialisierung pro Audioblock; der allgemeine gemischte Vorbis-Host bleibt separat erhalten. Vollständiger Windows-Missionsablauf und kalte Restores bestehen; Android-Laufzeit-/Hörabnahme bleibt offen.
