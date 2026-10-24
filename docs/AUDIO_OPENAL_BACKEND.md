# Echter OpenAL-Soft-Backend mit kontrolliertem PCM-Renderpfad

Stand: 26.10.2026. Fortsetzung des [Executorvertrags](AUDIO_DEVICE_EXECUTOR.md). Die fünf Anschlussarbeiten sind für **Windows x64 und OpenAL-Soft-Loopback** umgesetzt. Hier läuft erstmals ein konkreter AL-Mixer mit fortschreitenden Quellen-/Bufferzuständen. Er rendert in lokale WAV-Dateien; es werden keine Lautsprecher geöffnet. Android-/Appintegration und Hörabnahme bleiben offen.

## Fünf abgeschlossene Punkte

1. **Explizites Laden und Kontextbesitz.** `audio_openal::OpenAl` lädt eine ausdrücklich angegebene DLL über einen aufgelösten absoluten Pfad. Abhängigkeiten werden aus deren Verzeichnis und System32 geladen; eine implizite Suche nach einer beliebigen OpenAL-DLL findet nicht statt. C-ABI-Funktionen werden typisiert aufgelöst. DLL, Loopbackgerät und Kontext haben gebundene Lebensdauern; Fehler bei teilweiser Initialisierung werden beim Drop bereinigt. Der konkrete Backend besitzt genau einen ALC-Kontext. Sein u32-Kontexttoken ist ein eigener Alias und niemals ein abgeschnittener 64-Bit-Zeiger.
2. **Threadlokale und getrennte Sitzungen.** Jede AL-Operation bindet den eigenen Kontext mit `ALC_EXT_thread_local_context` und stellt anschließend den vorherigen Threadkontext wieder her. Der Backend ist durch `Rc`-Phantommarker weder Send noch Sync; zusätzlich prüft er den Eigentümerthread zur Laufzeit. Der erste echte parallele Testlauf zeigte Konflikte mit der ursprünglich prozessweiten Kontextumschaltung. Die Umstellung auf Threadkontexte behebt sie; unabhängige Backendinstanzen und der gesamte parallele Testlauf bestehen. Ein Compile-fail-Doctest sichert die fehlende Send-Freigabe ab.
3. **Tatsächliche AL-Operationen und Antworten.** Buffer-/Quellenerzeugung, PCM-Upload, Parameter, Bindung, Queue, Play/Pause/Stop, Status-/Countabfragen, Unqueue und Delete gehen an die geladenen AL-Funktionen. AL-/ALC-Fehler werden geprüft und als Fehler an den Executor weitergereicht. Rückgaben stammen aus dem Treiber, nicht aus den erwarteten Werten. Die vorhandene Sitzungs-/Eigentumsprüfung bleibt davor erhalten. EAX und eigenständige Musikbefehle sind für diesen Backend ausdrücklich nicht unterstützt und werden vor einem Auftrag abgewiesen. Kontext-Process/Suspend ist verbunden.
4. **Kontrollierter Audiotakt und echter Queueverbrauch.** `Backend::render`, `Executor::render` und `Session::render` liefern Stereo-S16-PCM mit gemeldeter Samplerate. Nur ein Backend mit Renderunterstützung darf diesen Pfad verwenden. Eingabegrenzen und Rückgabelänge werden geprüft; Rendern macht vorbereitete Programme veraltet. OpenAL Soft mischt und resampelt die tatsächlich hochgeladenen Daten. Quellen erreichen AL_STOPPED, vier Queuebuffers werden als verarbeitet gemeldet, und einzelne Buffer werden tatsächlich unqueued, mit neuen Bytes befüllt und erneut eingereiht. Statusbeobachtung, Besitzerplanung und Ausführung liegen zwischen kontrollierten Renderblöcken; dadurch schreitet der Loopbacktakt nicht unbemerkt zwischen den Abfragen fort.
5. **Reset, Neuaufbau und reproduzierbarer Check.** Reset stoppt und löscht eigene Quellen/Buffer, zerstört den Kontext und schließt das Gerät. Danach werden neuer Kontext und logische Bindungen in einer neuen Epoche aufgebaut. Die vorhandene Sitzung lädt behaltene PCM-Bytes und rekonstruiert auch pausierte Quellen. Ein echter AL_INVALID_OPERATION beim Löschen eines noch gebundenen Buffers löst die Sperre aus; ein frischer Kontext kann anschließend wieder rendern. Setup und zentraler Check prüfen die fest versionierte Bibliothek vor den Tests. Offline-Checks laden nichts aus dem Netz nach.

## Nachweise

Zehn neue Windows-Testgruppen prüfen Loader-/Ratefehler, nichtleeres PCM und natürliches Quellenende, Pause-/Stoppsilence, Verbrauch und Freigabe von vier echten Buffern, partielle Nachfüllung, Gain-/Pitchwirkung im gerenderten Signal, echten AL-Fehler mit Epochenerholung, Wiederaufbau einer pausierten Sitzung, unabhängige Kontexte sowie wiederholten Reset/Drop. Zwei zusätzliche portable Gruppen prüfen abgewiesenes Rendern ohne Backendarbeit und Sperre bei einer falschen Renderquittung. Die bestehenden Executor-/Originaltests bleiben erhalten. Der vollständige zentrale Windows-Check besteht mit je **1.394 Debug-/Release-Bibliothekstests**, Dependency-/Formatcheck, Material-/Doc-Tests einschließlich Thread-Compile-fail, Clippy aller Targets, Native-/Inspectorbau, Java und Buildvertrag. Der neue Compile-fail-Doctest prüft den Threadvertrag.

Zwei Originaldatei-Szenarien mischen jeweils eine WAV und zwei Ogg-Dateien über Besitzer, Executor und den **echten OpenAL-Mixer**. Pro Szenario laufen 64 Render-/Versorgungsrunden vor der Pause und acht weitere nach dem Wiederaufbau. Der Census wechselt beim natürlichen WAV-Ende von drei auf zwei Quellen. Je 20 Unqueue-Aufrufe und 36 Datenuploads werden tatsächlich ausgeführt. Der Backend meldet `1.1 ALSOFT 1.25.2`.

Die Ausgaben sind je 182.400 Stereo-S16-Frames bei 48 kHz, also 3,8 Sekunden. Der Signal-RMS beträgt im gen-Szenario etwa 3.353 und im geo-Szenario etwa 2.360. Pausierte Abschnitte liegen bei etwa 0,502 Sampleeinheiten, dem gemessenen niedrigen Ditherboden. Aktive und pausierte Kontextrekonstruktion erhalten den logischen Besitzer. Cue-Schutz und Stimmenstop bestehen; zuletzt gibt es null eigene Audioressourcen und null AL-Buffer. Quellenkanäle bleiben bis zum Backenddrop für Wiederverwendung angelegt.

- [Gerendertes gen-Szenario](../build/audio-openal/real-a.wav)
- [Gerendertes geo-Szenario](../build/audio-openal/real-b.wav)
- [Szenariobericht gen](../build/audio-openal/real-a.json), [geo](../build/audio-openal/real-b.json)
- [Zentralcheck](../build/audio-openal/check/validation.json), [Gesamtnachweis](../build/audio-openal/verification-summary.json)

Die 128 vollständigen Original-Uploadfälle wurden zusätzlich frisch fixturegleich geprüft. Die Plattformanbindung ist neuer Rust-Adaptercode; das ist kein behaupteter bitgenauer Nachbau des damaligen OpenAL-Mixers oder ein vollständiger globaler Engine-Blackboxnachweis. Eine tatsächliche Hörabnahme wurde nicht durchgeführt.

## Abhängigkeit und Reproduktion

`scripts/Setup-OpenALSoft.ps1` bezieht die offizielle portable Version 1.25.2 und behält Lizenzdateien neben dem Archiv. Archiv und Win64-DLL werden mit den im Setup festgehaltenen SHA-256-Werten geprüft. Es erfolgt keine Systeminstallation und kein Austausch der Steam- oder Windows-DLLs.

| Datei | SHA-256 |
|---|---|
| `openal-soft-1.25.2-bin.zip` | `67a0c4b800bd860c93c04f38caf8cbe4875f9c84700ac430efc451f70e265434` |
| Win64 `soft_oal.dll` | `3963b06e319180700be0bcc0d027336664bb3a4975876b4a5442f10746fab5b4` |

Der zentrale Windows-Check hat einen eigenen `openal-dependency`-Schritt. Er prüft den Cache im gewählten Toolsverzeichnis und setzt nur während des Checks `RC_OPENAL_SOFT_DLL`; der vorherige Prozesswert wird anschließend wiederhergestellt. Bei leerem Cache nennt der Offlinecheck den erforderlichen Setupschritt. `-Online` erlaubt den initialen Download. Direkte Cargo-Tests verwenden den expliziten Umgebungswert oder den regulären Projektcache. Linux baut weiterhin die portablen Kerne; der konkrete DLL-Backend und seine zehn Tests sind dort nicht aktiviert.

```powershell
./scripts/Setup-OpenALSoft.ps1
cargo test -p rc-package --lib audio_openal_tests
cargo run -p rc-inspect --bin rc-audio-openal-probe --release -- tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll fixtures/audio-vorbis/host-tick.json build/audio-buffer/real-gen/00.wav 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData/Music/musgen_vodeostorch_lp.ogg' 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData/Music/anticipation 1.ogg' build/audio-openal/real-a.wav build/audio-openal/real-a.json
./build.ps1 -Target Check -OutputDirectory build/audio-openal/check
```

## API- und Integrationsgrenzen

Der native FFI-Rand verwendet besitzende DLL-/Geräteobjekte, überprüfte Funktionsadressen, begrenzte Slices und die C-Aufrufkonvention der offiziellen Header. Keine Adresse eines Rustobjekts wird als langlebiger AL-Handle gespeichert. Quell-/Bufferwerte sind AL-Namen; nur der Kontextalias wird auf einen besitzenden Zeiger aufgelöst. Dieser Backend ist auf den Eigentümerthread begrenzt und meldet tatsächliche Treiberfehler; nach ungewisser Ausführung ist weiterhin ein Neuaufbau erforderlich.

Loopback fortschreitet ausschließlich auf ausdrücklichen Renderaufruf. Dies ermöglicht überprüfte Phasengrenzen, löst aber noch nicht die zeitlichen Rennen eines frei laufenden Lautsprechergeräts. Der Renderaufruf ist auf eine Million Frames begrenzt und liefert zwei i16-Samples pro Frame. Stereo-S16-Ausgabe ist auf 8–192 kHz begrenzt; Quellraten werden vom AL-Mixer resampelt. EAX-/Musikfähigkeiten werden nicht durch stille No-ops vorgetäuscht.

Der bisherige Wiederanlaufvertrag bleibt bestehen: WAVs starten neu, Streams an behaltenen Ringgrenzen; exakte Sampleoffsets sind nicht gespeichert. Der OpenAL-Soft-DSP ist kein Beleg für originale Creative-EAX-Effekte. Android benötigt weiterhin eine passende Bibliotheks-/Audioausgabeanbindung, Thread-/Lifecycleführung und Szenenintegration. Eine APK und ein Lautsprechergerät wurden in diesem Block nicht gestartet.

## Primärquellen

- [OpenAL Soft: offizielle Downloads](https://openal-soft.org/openal-binaries/)
- [ALC_SOFT_loopback](https://openal-soft.org/openal-extensions/SOFT_loopback.txt): kontrollierter Renderpfad und Formatattribute.
- [ALC_EXT_thread_local_context](https://openal-soft.org/openal-extensions/EXT_thread_local_context.txt): threadlokale Bindung und Wiederherstellung.
- [OpenAL-Header Version 1.25.2](https://github.com/kcat/openal-soft/tree/1.25.2/include/AL): Typen und C-ABI.
- [Microsoft: LoadLibraryExW](https://learn.microsoft.com/en-us/windows/win32/api/libloaderapi/nf-libloaderapi-loadlibraryexw): absolute Pfade und eingeschränkte Importsuche.

Nächster sinnvoller Abschnitt: Ausgabe des vorhandenen kontrollierten PCM-Renderpfads über einen Plattform-Sink, danach Android-Thread/Lifecycle und der Anschluss an den spielbaren Szenenhost.

## Nachfolgender PCM-Ausgabeblock

Die [Android-PCM-Diagnose](AUDIO_PCM_OUTPUT.md) kann die hier erzeugten Audiodaten über einen eigenen AudioTrack-Worker ausgeben. Sie ist gebaut und hostseitig geprüft; Android-Laufzeit-/Hörabnahme und der native Live-Mixer-/Szenenanschluss bleiben offen.

## Nachfolgender nativer Live-Anschluss

Der [native Android-Live-Diagnosepfad](AUDIO_NATIVE_LIVE.md) baut OpenAL für beide Android-Architekturen und verbindet Rust-Besitzerworker, JNI, Reservierung und AudioTrack. Eine WAV kann damit direkt im Android-Prozess gerendert werden. Build-/Hostnachweise bestehen; Android-Laufzeit und Klang sowie der allgemeine Szenen-/Missionshostanschluss bleiben offen.
