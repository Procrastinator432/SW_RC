# WAV-Registrierung, Bufferbesitz und Freigabe

**Nachfolgende Umsetzung 25.10.2026:** [Gemeinsamer Audioressourcenhost](AUDIO_RESOURCE_HOST.md) führt statische Bank und Vorbisbesitz zusammen, einschließlich mehrerer statischer Quellen, begrenzter Streamrunden, atomarem Shutdown und gemeinsamer Persistenz. Die unten genannten offenen Zusammenführungsarbeiten beschreiben den damaligen Ausgangspunkt; allgemeiner Pool/Update und Geräte-/Appanschluss bleiben offen.

Stand: 25.10.2026. Fortsetzung des [Vorbis-Dateipfads](AUDIO_VORBIS.md). Die fünf Aufgaben sind intern umgesetzt; allgemeiner Quellenhost, reale Audio-API und Appanbindung bleiben offen. Keine neue APK.

## Fünf abgeschlossene Punkte

1. **WAV-Auslesen:** `audio_wave::scan` findet wie `FWaveModInfo::ReadWaveInfo` die jeweils ersten fmt-, data- und smpl-Chunks, berücksichtigt ungerade Chunkgrößen und liest die ursprünglichen Format-/Samplerfelder. Der Zugriff ist auf eigene Slices begrenzt. `Pcm::from_wave` ergänzt eine strengere PCM-Zulassung: RIFF-Signatur und Länge, PCM-Tag 1, Mono/Stereo, 8/16 Bit, gültige Rate, Bytefolge, Blockgröße und vollständige Sampleframes. Die Datenbytes bleiben unverändert.
2. **PCM-Registrierung:** `audio_buffer_bank::Bank` besitzt registrierte statische Sounds einschließlich PCM und Bufferbeschreibung. Alle vier AL-Formate `0x1100`–`0x1103` werden abgedeckt. Stereo benötigt dieselbe explizite nicht räumliche Erlaubnis wie im untersuchten RegisterSound-Pfad. Erzeugung, Upload, Prüfergebnisse, Entladen und Handlezuweisung werden als geordnete Ereignisse ausgegeben. Generierte AL-IDs und Prüfergebnisse liefert der Geräteadapter; der neue Kern erzeugt keine realen AL-Objekte.
3. **Wiederverwendung:** Ein bereits registrierter Sound liefert seinen vorhandenen Handle zurück, bevor neue WAV-Daten oder Geräteantworten gelesen werden. So entstehen keine doppelten PCM-Bestände oder Buffer. Die Bank prüft eindeutige Sound-, Buffer-, Speicher- und AL-Tokens. Bei einem fehlgeschlagenen Upload wird kein Eintrag veröffentlicht und die Löschung des erzeugten AL-Buffers geplant.
4. **Originale Freigabereihenfolge:** `audio_buffer` bildet Konstruktion, Release, Destruktor und den äußeren UnregisterSound-Ablauf ab. Release fragt bei nichtleerem Buffer die Quellen in Reihenfolge ab, stoppt alle passenden Bindungen, berücksichtigt deren zurückgelieferte Bufferwörter und löscht anschließend jeden logischen AL-Buffer. Danach folgen Arrayverkleinerung, Soundhandle 0, DestroyStream ohne Warten und Stagingfreigabe. Der Destruktor behandelt auch die ursprünglichen Besitz-/Kapazitätsflags. Unregister entfernt alle passenden Registryeinträge vor der Freigabe und gibt danach das Bufferobjekt frei.
5. **Persistenz und Vorbis-Abbau:** Die PCM-Bank speichert eigene Bytes, Registrierungen und Quellenbindungen in Schema 1. Laden und Freigabe sind atomar; Wiederverwendung, Abmelden und erneutes Registrieren laufen nach kaltem Laden identisch weiter. Der bestehende `audio_vorbis_host` besitzt zusätzlich `dispose`: FALSource-Stop, aktuelle Unqueue-IDs, AL-Löschung sowie Stream-/Staging-/Decoderabbau werden zusammen geplant. Nach natürlichem Streamende wird der Stream nicht erneut zerstört; wiederholtes Dispose ist leer. Alte Vorbis-Snapshots behalten Schema 1.

## Originalnachweise

Die Hash-geprüften Originaldateien sind dieselben wie im [bisherigen Audioprüfrahmen](AUDIO_SOURCE_TICK.md). Die Fixtures enthalten ausschließlich synthetische Daten und Ergebnisse, keine ausführbaren Originalbytes.

| Originalfunktion | Adresse | Frisch ausgeführter Umfang |
|---|---|---|
| FWaveModInfo::ReadWaveInfo | `103d2240` | 192 synthetische WAVE-Dateien; zusätzlich 48 echte Soundassets. |
| FALBuffer-Konstruktor | `10006cf0` | 32 Einzelaufrufe; außerdem innerhalb des WAV-Precachepfads. |
| Buffer-Release / Destruktor | `10007380` / `10007f10` | Zusammen 256 Fälle mit unterschiedlichen Quellen, Stopantworten, Bufferzahlen und Besitzflags. Der Arrayhelfer `10006a10` läuft ebenfalls tatsächlich. |
| RegisterSound-Wrapper | `10008770` | 64 Aufrufe mit leerem oder vorhandenem Soundhandle; Precache ist an dieser Wrappergrenze eine kontrollierte Antwort. |
| UnregisterSound | `10008050` | 64 ganze Aufrufe einschließlich tatsächlichem Release/Destruktor. Registry-RemoveAll ist als kontrollierter Helfer ersetzt; dessen untersuchte Semantik entfernt alle gleichen Einträge. |
| WAV-Zweig von Precache/RegisterSound | `10008100` | 128 ganze Aufrufe mit tatsächlichem WAV-Parser, Bufferkonstruktor und Arraywachstum. Datei-/Lazy-Load, Params, Logausgabe, Allokator und AL-Aufrufe sind kontrollierte Grenzen. |

Das sind **736 synthetische äußere Originalaufrufe**, zuzüglich der 48 echten WAV-Auslesevorgänge und der verschachtelten Aufrufe. Die Rusttests vergleichen 192 Parserfälle, 32 Konstruktorfälle, 256 Release-/Destruktorfälle und 64 Unregisterfolgen vollständig innerhalb ihrer beschriebenen Callbackverträge. Beim WAV-Precache vergleichen sie die übernommenen Zulassungs-/Handleentscheidungen und tatsächlichen Uploadbytes; die 64 Registerwrapperaufrufe dienen zusätzlich als Befund für die Handle-/Precache-Grenze.

**Elf neue Testgruppen** prüfen diese Regeln sowie Cachetreffer ohne erneutes Lesen, Stereoablehnung, beschädigte Größen, ungültige PCM-Layouts, fehlende Stopantworten, doppelte Ressourcen, Ladefehler, aktive Bindungen und vollständigen Vorbis-Abbau. Die drei zusätzlichen Vorbistests vergleichen auch nach kaltem Laden Stop→Delete→Destroy/Free, idempotenten Abbau und abgewiesene Unqueue-Antworten mit verlorenen Buffer-IDs.

**48 echte Soundassets** aus `gen_sound.uax` und `geo_sound.uax` sind separat geprüft: **8.634.946 PCM-Bytes**, unveränderte Uploaddaten, Cachetreffer und identische Freigabe nach kaltem Laden. Ihre Metadaten stimmen mit dem tatsächlich ausgeführten Original-WAV-Parser überein. Pythons unabhängiger WAVE-Leser bestätigt Format und Samplebytes ebenfalls. Die extrahierten Originaldateien bleiben ausschließlich unter `build/audio-buffer/`.

Der zentrale Check besteht mit je **1.294 Debug-/Release-Bibliothekstests**, Format, Materialimport-/Doc-Tests, Clippy aller Targets, Native-/Inspectorbau, Java und Buildvertrag. [Lokaler Gesamtbericht](../build/audio-buffer/verification-summary.json).

## Bewusste Unterschiede und Grenzen

Der rohe Originalparser prüft beispielsweise die Kennung WAVE, aber nicht RIFF, und kann bei beschädigten Chunkgrößen außerhalb des Arrays lesen. `scan` erhält die belegte Feldauswahl für gültige Daten und weist unzulässige Slicezugriffe ab. `Pcm::from_wave` ist eine zusätzliche portable Zulassungsgrenze; das native Mapping behandelt jeden Nicht-8-Bit-Wert als AL-16-Bit und jeden Nicht-Mono-Wert als Stereo. Die strikte PCM-Bank akzeptiert solche fremden Formate nicht stillschweigend.

Im Original-WAVpfad wird der Rückgabewert der ersten kontrollierten AL-Prüfung ignoriert; entscheidend ist die spätere Uploadprüfung. Bei deren Fehler bleibt der bereits generierte AL-Buffer im beobachteten Originalablauf ohne Delete-Aufruf. Zwölf synthetische Fälle belegen diesen Weg. Der portable Besitzer ergänzt dafür ausdrücklich einen Delete-Auftrag. Das ist eine beabsichtigte Ressourcenbereinigung und keine behauptete vollständige Fehlerparität. Die tatsächliche Implementierung von `alError` oder einer fatalen API-Fehlerbehandlung wird dadurch nicht emuliert.

Raw-Stopantworten bleiben extern und dürfen innerhalb des begrenzten Modells Bufferwörter verändern oder eine inaktive Bindung stehenlassen. Die besitzende PCM-Bank ist strenger: ihre aktiven Bindungen müssen nach Stop gelöst sein, die eigene AL-ID muss zur Löschung erhalten bleiben und die Nullgrößen-Reallokation muss 0 liefern. Sonst wird der gesamte Plan abgewiesen. Vorbis-Dispose prüft ebenfalls, dass Unqueue dieselben eigenen IDs als Menge zurückliefert. Eine echte Audio-API wird dabei noch nicht aufgerufen; Atomarität gilt für Zustand und Plan, nicht für bereits ausgeführte fremde Geräteseiteneffekte.

`dispose` benötigt explizite opake Tokens für Subsystem, Bufferlistenallokation und Quelle. Seine Source-/Stop-Ereignisse markieren die Aufrufgrenze; die eingefügten Driverereignisse beschreiben die eigentlichen AL-/Quellenaktionen. Tokens sind keine von Rust dereferenzierten Pointer. Wiederhergestellte AL-Tokens müssen in einem künftigen Geräteadapter neu auf reale Objekte abgebildet werden; die neuen Snapshots sind keine Android-App-Checkpoints.

Grenzen: 16 MiB eigene PCM-Daten beziehungsweise kodierter Snapshot, maximal 4.096 Bankeinträge, Quellen und Bufferwörter je Rohbuffer. Die JSON-Darstellung kann die Snapshotgrenze früher erreichen. Decoderinterner temporärer Speicher ist davon getrennt. Allgemeine Audioquellen, Registryeinträge und Streams sind noch nicht zu einem globalen Host zusammengeführt. Globale Thread-/Listenlebensdauer, Geräteobjekte, vollständige Fehlerbehandlung und App-/Androidanschluss bleiben offen.

## Reproduzieren

```powershell
./build.ps1 -Target Check -OutputDirectory build/audio-buffer/final-check
cargo build -p rc-inspect --bin rc-wave-bank-probe --release --locked --offline
target/release/rc-wave-bank-probe.exe 'GAME_DATA/Sounds/gen_sound.uax' build/audio-buffer/real-gen
target/release/rc-wave-bank-probe.exe 'GAME_DATA/Sounds/geo_sound.uax' build/audio-buffer/real-geo
python -X utf8 scripts/Test-AudioBufferOriginal.py --game-data GAME_DATA `
  --output build/audio-buffer/original-replay.json --verify-fixture fixtures/audio-buffer-native.json
python -X utf8 scripts/Test-AudioWaveRegistrationOriginal.py --game-data GAME_DATA `
  --output build/audio-buffer/wave-registration-replay.json --verify-fixture fixtures/audio-wave-registration-native.json
python -X utf8 scripts/Test-AudioWaveAssetsOriginal.py --game-data GAME_DATA `
  --output build/audio-buffer/real-assets-validation.json build/audio-buffer/real-gen build/audio-buffer/real-geo
```

`GAME_DATA` ist durch das lokale GameData-Verzeichnis zu ersetzen. Originalausführung benötigt die bereits gepinnten Analyseabhängigkeiten; Rusttests benötigen keine Originalinstallation. Der neue CLI-Prüfer schreibt ausschließlich in den angegebenen Ausgabeordner.

## Nächster Integrationsabschnitt

Statische und gestreamte Ressourcen in einem gemeinsamen Quellenhost zusammenführen; danach Wrapper/UpdateBase, Pool, Upload und Stimmenzählung verbinden. Anschließend reale Geräteobjekte und deren Wiederaufbau, Workerplanung/-shutdown sowie die allgemeine Appanbindung umsetzen. Eine neue Android-/Hörabnahme gehört hinter diese Integration.
