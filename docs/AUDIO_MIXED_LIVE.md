# Gemischter Audiohost auf dem nativen Worker

Stand: 26.10.2026. Fortsetzung des [nativen WAV-Live-Pfads](AUDIO_NATIVE_LIVE.md). Der vorhandene allgemeine WAV-/Vorbis-Host ist nun mit dem nativen Besitzerworker, JNI, reservierter PCM-Ausgabe und einer eigenen Android-Diagnose verbunden. Das ist weiterhin ein begrenzter Audiotest; der Missionssound läuft über den bisherigen Host.

## Fünf umgesetzte Punkte

1. **Gemischten Besitzer laden.** Ein `.rcmix`-Paket enthält den logischen Quellenhost, Gerätemanifest, Soundmetadaten und explizite Umgebungseingaben. Der bestehende `audio_device_session::Session` baut Quellen und Buffer neu auf und lädt WAV-/Streamdaten. OpenAL bleibt auf dem bereits eingeführten Rust-Worker. Der WAV-Einzeltest ist weiter verfügbar.
2. **Takt und Nachfüllung.** Ein reservierter PCM-Block löst zuerst wartende Befehle, eine begrenzte Vorbis-Servicerunde und den allgemeinen Quellenzyklus aus. Quellenstatus sowie verarbeitete/eingereihte Buffer kommen aus dem tatsächlichen Backend. Dadurch funktionieren natürliche WAV-Enden, Streamnachfüllung und der bestehende Census-/Duckingpfad auch hinter dem nativen Adapter. Pro Block sind höchstens 480 Frames bei 48 kHz und zwei Streamservices erlaubt.
3. **Begrenzte Befehlsübergabe.** Pause eines Actor-/Soundpaars, StopAll sowie explizite Actor-/Listenereingaben werden mit lückenloser Folgenummer angenommen. Die native Warteschlange enthält höchstens 32 Befehle; Java nimmt höchstens acht Bedienaktionen pro Ausgabesitzung an. Doppelte, alte, übergroße und ungültige Befehle werden abgewiesen, ohne ihre Folgenummer zu verbrauchen. Verarbeitung erfolgt vor dem nächsten Renderblock. Jede neue Diagnose bekommt eine eigene Bedienqueue, damit alte Produzenten keine neuen Aktionen übernehmen.
4. **Audiozustand und kalter Neuaufbau.** Der Snapshot enthält Besitzer, Manifest, Decoder-/Ringzustände, Actor-/Listenereingaben, verbleibendes Testbudget und bereits angenommene, noch wartende Befehle. Ein neu geöffneter nativer Besitzer setzt diese Befehle genau einmal fort. PCM-Paketfolgen beginnen für die neue Ausgabe bei 0; verbleibende Diagnoseframes werden separat gemeldet. Die Android-Diagnose speichert den Audiozustand mit `AtomicFile` im privaten Appverzeichnis und bietet einen eigenen Fortsetzen-Knopf.
5. **Diagnosepaket und Prüfpfad.** APK 0.63.0 enthält **Gemischten Audiohost prüfen**. Der vorhandene OpenAL-Probeexporter kann zusätzlich ein `.rcmix`-Paket schreiben. Ein echter Windows-C-ABI-Test spielt WAV plus zwei Vorbis-Streams, pausiert beide Streams, speichert vor Anwendung dieser Befehle, schließt den Besitzer vollständig und öffnet den Snapshot erneut. Pause, spätere Fortsetzung, Datenfluss, Befehlsfolgen und exaktes Testende sind geprüft.

## Grenzen des Anschlusses

Der Adapter nimmt höchstens 16 MiB Paket-/Snapshotdaten, neun physische Knoten einschließlich Transient, 64 Sound-/Actoreinträge und ein Diagnosebudget von höchstens zehn Sekunden an. Die bereits vorhandenen Sitzungs-, Worker- und PCM-Budgets gelten zusätzlich. Es gibt noch keine Aufnahme neuer Play-Ereignisse aus der laufenden Mission; das Testpaket enthält die anfänglich wartenden Quellen.

Actor-/Listenerdaten sind explizite Eingaben in die bestehenden Hostberechnungen und werden gespeichert. Das bereitgestellte Testpaket verwendet die bisherigen **2D-Quellenflags**; dort bewirken Actor-Links/Rechts-Aktionen erwartungsgemäß keine räumliche Klangwanderung. Der allgemeine physische Listener mit Ausrichtung sowie eine laufend passende RSQRT-Seedpolitik für beliebige 3D-Bewegungen bleiben offen. Dieser Block belegt die Befehls-/Zustandsübergabe, keine fertig abgenommene 3D-Missionsakustik.

Der Snapshot liegt am **bereits gerenderten** Rand und kann der gehörten Ausgabe um die gepufferten Frames voraus sein. Beim Fortsetzen werden statische Quellen neu gestartet und Streams an ihren behaltenen Ringgrenzen rekonstruiert. Dies ist keine samplegenaue Wiederaufnahme. Der Diagnose-Audiozustand ist unabhängig vom vorhandenen Missionsspielstand; automatische Speicherung bei Fokusverlust oder Prozessende ist hier noch nicht angeschlossen.

Die Hosttransaktionen serialisieren Zustände und sind noch kein nachgewiesener Echtzeitpfad auf Android. CPU-/Speicherbedarf, tatsächliche Ausgabelatenz und flüssige Wiedergabe müssen am Gerät geprüft beziehungsweise weiter optimiert werden. Größenlimits allein sind keine Leistungszusage.

## Nachweise

- Elf neue gemischte Rust-Testgruppen: Paket-/Metadatenprüfung, Render-Vorprüfung, Queue-/Folgenummerngrenzen, Actor-/Listenerfortlauf, wartende Befehle bei kaltem Restore, Transportepoche/Restbudget, terminaler Backendfehler, ungültiges Manifest und tatsächliches OpenAL mit Quellenende/Nachfüllung/Pausenaufbau.
- Zwei native ABI-Gruppen für falschen Sitzungstyp und ungültigen Paketstart ohne verlorenen Besitzer.
- Zwei zusätzliche Java-Produzentengruppen für verkürztes und bereits ausgeschöpftes Restbudget. Insgesamt vier Paket-, zwölf Pump- und acht Live-Gruppen.
- Je **1.424 Debug-/Release-Bibliothekstests**, Doctests, Materialimport, Clippy, Java- und Bauvertragsprüfungen.
- Tatsächlicher Windows-Mixer: eine Sekunde vor dem Snapshot und neun Sekunden nach dem Neuaufbau; insgesamt 480.000 Stereo-Frames / 48 kHz. Zwei Pausebefehle bleiben bis zum Speichern wartend und wirken nach dem Restore einmal. Nach dem Einschwingen liegt der pausierte RMS nahe 0,5 Sampleeinheiten; aktive Abschnitte liegen deutlich darüber. Ein Actor- und ein Listenerauftrag sowie zwei Fortsetzen-Aufträge werden anschließend übernommen. Die Stream-Servicezahl steigt von 6 auf 52.
- Android-Bau für arm64 und x86_64; ELF-/APK-Prüfung umfasst auch die neuen `rc_mixed_*`- und JNI-Exporte. Bibliotheken werden nach Gradles Strip-Stufe bytegleich mit dem APK verglichen. Android-JNI, Dateispeicherung und Bedienung wurden noch nicht zur Laufzeit ausgeführt.

[Zentralcheck](../build/audio-mixed-live/check/validation.json) · [Android-Bau](../build/audio-mixed-live/android/validation.json) · [Echter C-ABI-/Mixerablauf](../build/audio-mixed-live/real/real-mixed.json) · [Gesamtnachweis](../build/audio-mixed-live/verification-summary.json)

## Später am Gerät prüfen

1. [APK 0.63.0 installieren](../build/audio-mixed-live/android/artifacts/republic-commando-0.63.0-mixed-audio.apk).
2. [Audiotestpaket](../build/audio-mixed-live/mission-test.rcmix) übertragen und unter **Gemischten Audiohost prüfen** importieren. Es verwendet einen Original-WAV-Auszug und zwei Instanzen derselben kurzen Original-Ogg-Datei aus der lokalen Installation. Es enthält keine neue Missionsszene.
3. Starten, Stream 1001 pausieren/fortsetzen und während der Ausgabe **Audiozustand speichern** wählen. Danach stoppen und **Audiozustand fortsetzen** wählen; bei den höchstens zehn Sekunden Budget entsprechend früh speichern.
4. Wiederholte Starts, Fokusverlust, Hintergrundwechsel, beschädigten Diagnosezustand, vollständiges Ende und Klang/Leistung prüfen. Die vorhandene Mission anschließend getrennt auf Regressionen prüfen.

Ein Testpaket oder Snapshot wird bei Start erneut geladen; ein importierter Snapshot beginnt an seiner gespeicherten logischen Stelle. Die Links/Rechts-Aktionen dienen im aktuellen 2D-Paket dem Eingabe-/Speicherpfad.

## Reproduktion

```powershell
./build.ps1 -Target Check -OutputDirectory build/audio-mixed-live/check
./build.ps1 -Target Android -OutputDirectory build/audio-mixed-live/android -Abis arm64-v8a,x86_64
# Optionales neuntes Programmargument: zusätzliches gemischtes Startpaket
cargo run --release --locked --offline -p rc-inspect --bin rc-audio-openal-probe -- tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll fixtures/audio-vorbis/host-tick.json build/audio-buffer/real-gen/00.wav 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData/Music/musgen_vodeostorch_lp.ogg' 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData/Music/musgen_vodeostorch_lp.ogg' build/audio-mixed-live/export.wav build/audio-mixed-live/export.json build/audio-mixed-live/mission-test.rcmix
python scripts/Test-MixedLiveAudio.py target/release/rc_native.dll tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll build/audio-mixed-live/mission-test.rcmix build/audio-mixed-live/real
```

Die Windows-Probe benötigt die lokalen Originalassets; sie werden nicht als versionierte Fixtures übernommen. Die neuen Rusttests verwenden vorhandene synthetische WAV-/Vorbis-Fixtures. Die Android-Dateispeicherung nutzt den offiziellen [AtomicFile-Vertrag](https://developer.android.com/reference/android/util/AtomicFile); tatsächliche Prozessabbruch-/Dateisystemtests auf Android stehen aus.

## Nächster Abschnitt

Dynamische Play-/Stop-Ereignisse und Soundbankauflösung aus der Szene anbinden, physische Listenerpose und passende portable 3D-Prioritätspolitik ergänzen, Hot-Path-Serialisierung reduzieren sowie Kartenwechsel und Missionscheckpoint gemeinsam orchestrieren. Erst danach einen ausgewählten Missionssound auf diesen Pfad umstellen und mit dem bestehenden Host vergleichen. Keine neuen Originalfunktionen oder DSP-Bitparität werden aus diesem Adapterblock abgeleitet.

## Nachfolgender Missionsanschluss

Der [Missions-WAV-Pfad 0.64.0](MISSION_NATIVE_AUDIO.md) verbindet nun die vorhandene Spielsoundbank und Ereignisse mit dem gemeinsamen nativen Worker, physischer Listenerpose und Karten-/Checkpointlebenszyklus. Er vermeidet vollständige Hostserialisierung pro Audioblock; der allgemeine gemischte Vorbis-Host bleibt separat erhalten. Vollständiger Windows-Missionsablauf und kalte Restores bestehen; Android-Laufzeit-/Hörabnahme bleibt offen.
