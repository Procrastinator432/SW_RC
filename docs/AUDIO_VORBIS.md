# Echter Vorbis-Dateipfad und portable Streamregistrierung

**Nachfolgende Umsetzung 25.10.2026:** [WAV-/Bufferressourcen](AUDIO_BUFFERS.md) ergänzen den statischen PCM-Pfad und den expliziten vollständigen Dispose-Ablauf dieses Vorbis-Hosts. Alte Snapshots behalten ihr Schema. Die unten genannten offenen Ressourcenarbeiten beschreiben den damaligen Ausgangspunkt.

Stand: 25.10.2026. Fortsetzung des [PCM-Workers](AUDIO_STREAM_DATA.md). Der neue Host dekodiert echte Dateien und verbindet sie mit dem originalgeprüften Worker und Treibertick. Die allgemeine Spiel-/Androidanbindung bleibt offen; keine neue APK.

## Fünf abgeschlossene Punkte

1. **Dateibereitstellung und Decoder:** `audio_vorbis::Provider` öffnet begrenzt gelesene Dateien oder eigene komprimierte Bytearrays und dekodiert mit gepinntem `lewton = 0.10.2`. Er hält die komprimierte Datei und ein laufendes Paket, statt das gesamte Stück als PCM zu speichern. Die Ausgabe ist vorzeichenbehaftetes, verschachteltes 16-Bit-Little-Endian-PCM. [Verwendete Decoder-API](https://docs.rs/lewton/0.10.2/lewton/inside_ogg/struct.OggStreamReader.html).
2. **Format und Dateigrenzen:** Mono wird auf `0x1101`, Stereo auf `0x1103` abgebildet; die Chunkgröße ist wie im Original `0x10000`. Frequenz, Kanäle und Dauer stammen aus der Datei. Vollständige Seiten, CRC, Seriennummer, fortlaufende Seitennummern und Endseite werden geprüft. Verkettete, multiplexe, abgebrochene und mehrkanalige Dateien werden abgewiesen. Die letzte Granule begrenzt die PCM-Samplezahl auch bei kurzen Einseitenclips.
3. **Anschluss an den Worker:** Der reale Decoder liefert nun Daten an denselben `ReadChunks`-Kern, der zuvor nur vorgegebene Antworten beziehungsweise fertige PCM-Arrays erhielt. Paketgrenzen und ungerade Teillesungen bleiben erhalten. Der Originalkern führt Nullfüllung, Endflag und Loopzähler aus. Im Loop wird der tatsächliche Decoder wieder an den Anfang gesetzt. Laufende Services und nachfolgende FALSource-Ticks arbeiten mit den tatsächlich gefüllten Stagingbytes.
4. **Vorbefüllung und Besitz:** `audio_vorbis_host::Session::register` erzeugt vier Uploadbelege mit je 64 KiB Daten, setzt Sounddauer und Bufferhandle, verwaltet eine eigene Stagingallokation und hinterlässt genau einen offenen Chunkauftrag. Ein kurzes Stück wird mit Nullbytes ergänzt; sein Endflag bleibt erhalten. Der Host verwendet vier vom Aufrufer gelieferte AL-IDs. Nach der originalen Endfreigabe entfernt er Decoder und Stagingbesitz; bereits hochgeladene Bytes bleiben wie im vorherigen Bridge-Vertrag erhalten.
5. **Gespeicherte Wiederaufnahme:** Eigene Snapshots speichern komprimierte Quelldaten, genaue dekodierte Byteposition, Worker, Ringindizes, Treiberzustand, Staging und Uploadbytes. Beim Laden wird der Decoder rekonstruiert und bis zur gespeicherten Byteposition nachdekodiert. Das funktioniert auch mitten in einem Sample beziehungsweise Paket. Ein fehlgeschlagener Hostbefehl stellt Bridge und Decoderposition wieder her. Auch geschlossene Streams lassen sich laden.

## Nachweise

Neun neue Rust-Testgruppen prüfen unabhängige Referenzsamples, Samplezahl, beliebige Teillesungen, Seek-/Ladegrenzen, beschädigte Dateien, Vorbefüllung, Busy-Verhalten, Fehlerrücknahme, Loopuploads und kalten Fortlauf. In acht Service-/Tickrunden werden hochgeladene Bytes exakt gegen die fortlaufende reale Dekodierung verglichen. Kaltes Laden nach jeder Runde erzeugt dieselben Ereignisse und Bytes. Ein kurzer nicht schleifender Stream wird einschließlich Decoder-/Stagingfreigabe geprüft.

Die Fixture-Dateien unter `fixtures/audio-vorbis/` sind ausschließlich synthetische Töne beziehungsweise Stille. Mono, Stereo, übersteuerte Samples und sehr kurze Clips besitzen eingefrorene Referenzbytes aus `soundfile 0.13.1` / `libsndfile 1.2.2`; Hashes und Referenzumrechnung stehen in `manifest.json`. Die Tests vergleichen jeden Samplewert mit höchstens einer Stufe Abweichung. Das ist ein Decodervergleich und keine behauptete Bitparität zum historischen Vorbisdecoder.

Die zusätzliche Dateiprüfung dekodiert **vier synthetische und vier originale Dateien**, darunter `anticipation 1.ogg` mit **22.112.384 PCM-Bytes**. Für alle acht Dateien stimmen Kanäle, Frequenz und Samplezahl mit der unabhängigen Bibliothek überein. Die größte gemessene Sampleabweichung beträgt **1**, innerhalb der festgelegten Prüfschranke 2. Jede Datei besteht außerdem einen erneuten Decoderaufbau mit exaktem Vergleich des letzten Abschnitts. Originaldaten und dekodierte Musik liegen ausschließlich in ignorierten lokalen Buildausgaben.

Für den Referenzvergleich wird unabhängig dekodiertes Float-PCM mit `round-to-even(x * 32768)` auf signed 16 Bit begrenzt. Der direkte int16-Ausgabepfad der Referenzbibliothek zeigte bei einem übersteuerten Originalsample einen Wraparound. Die explizite Sättigung beseitigt diesen Fehler im Prüfer; der Rustdecoder sättigte bereits korrekt. Ein zusätzlicher synthetischer Clip mit 720 übersteuerten Samples sichert diesen Fall ab. Die Prüftoleranz wurde dafür nicht erweitert.

**Originalmetadaten:** `Test-AudioVorbisInitOriginal.py` führt die ganze Funktion `FALAudioStreamOgg::Init` bei `10003170` in 128 synthetischen Fällen tatsächlich aus. fopen, Allokator und die Codecantworten werden kontrolliert ersetzt. Der Lauf bestätigt Mono-/Stereozuordnung, Abweisung anderer Kanäle, Chunkgröße, Frequenz und f64→f32-Dauerablage sowie die ursprünglichen frühen Fehlerzweige. Der Rusttest vergleicht die portierte Formatwahl und Chunkkonstante gegen die erfolgreich geöffneten Originalfälle. Der gesamte historische fopen-/Allokator-/ov_open-Lebenszyklus ist damit **nicht** als portiert ausgewiesen.

Die vier Vorabuploads, der anschließende Stagingauftrag und die Sounddauer folgen dem untersuchten Streamingzweig von `10008100`; diese gesamte RegisterSound-/Precache-Funktion wird durch diesen neuen Nachweis nicht als vollständig ausgeführt oder nachgebaut gewertet. Die Request-/ReadChunks-/Treibertick-Regeln besitzen weiterhin die [eigenen ganzen Originalvergleiche](AUDIO_STREAM_DATA.md#originalnachweis).

Der zentrale Check besteht mit je **1.283 Debug-/Release-Bibliothekstests**, Format, Materialimport-/Doc-Tests, Clippy aller Targets, Native-/Inspectorbau, Java und Buildvertrag. [Lokaler Prüfbericht](../build/audio-vorbis/verification-summary.json).

## Schnittstelle, Grenzen und nächste Arbeit

Die neuen öffentlichen Module sind `audio_vorbis` und `audio_vorbis_host`. `Provider::new` / `from_file` erstellen den Decoder; `read_bytes` liefert einen Paketabschnitt, nicht zwingend die gesamte angeforderte Länge. `seek_byte` positioniert durch atomaren Wiederaufbau und Nachdekodieren. `Session::register` nimmt einen frischen vorbereiteten Treibertick, Decoder, Ressourcentokens und Loopmodus entgegen. `state()` gibt eine unveränderliche Sicht auf den Bridgezustand. Befehle sind dieselben `Service`-/`Tick`-Werte wie im bisherigen Bridge-Modul.

Der Host hält seinen Bridgezustand privat. Dessen leere PCM-Struktur ist nur ein Kompatibilitätsmarker für den bisherigen Schema-1-Bridgevertrag; der neue Host ruft niemals dessen PCM-Service auf. Sämtliche Services gehen an den realen Provider. Bestehende PCM-/Bridge-Snapshots und Fixtures behalten ihr Schema.

Komprimierte Dateien sind auf 16 MiB begrenzt, deklarierte dekodierte Dauer auf 512 MiB PCM, Frequenzen auf 1–1.000.000 Hz. Eigene Hostressourcen und komprimierte Datei besitzen zusammen ein logisches 16-MiB-Budget. Kodierte Snapshots sind ebenfalls auf 16 MiB begrenzt; ihre JSON-Darstellung kann diese Grenze früher erreichen. Das ist keine Messung des maximalen temporären Speicherbedarfs des Decoders. Beim Laden und beim Loop-Neustart werden Header neu aufgebaut; Laden/Seek ist durch das Nachdekodieren linear zur Byteposition und gehört nicht in einen Echtzeitaudiocallback.

CRC-/Container-/Codecfehler werden als Fehler abgewiesen. Ein tatsächlicher Codecfehler im Host bricht dessen Schritt atomar ab. Das ist eine explizite portable Fehlergrenze; es wird keine identische Zuordnung aller Lewton-Fehler zu historischen negativen `ov_read`-Codes behauptet. Die Raw-Schicht erhält ihre bereits belegten negativen Rückgaberegeln. Für leere endlos schleifende Daten gilt weiterhin ihr begrenzter Fortschrittswächter.

Noch offen sind vollständige WAV-/RegisterSound-/UnregisterSound-Ressourcenverwaltung, echte AL-Erzeugung/-Löschung und API-Fehler, Quellenwrapper/UpdateBase mit dem allgemeinen Pool, globaler Streammanager und Betriebssystemthread sowie App-/Androidgeräteanbindung. Das neue Schema ist kein App-Checkpointformat. Es gibt keine neue Hör-/Handyabnahme für diesen unabhängigen Host.

## Reproduzieren

```powershell
./build.ps1 -Target Check -OutputDirectory build/audio-vorbis/check
cargo build -p rc-inspect --bin rc-vorbis-probe --release --locked --offline
python -X utf8 scripts/Test-AudioVorbis.py --binary target/release/rc-vorbis-probe.exe `
  --output build/audio-vorbis/decoder-check `
  --game-data 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData'
python -X utf8 scripts/Test-AudioVorbisInitOriginal.py `
  --game-data 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' `
  --output build/audio-vorbis/init-replay.json `
  --verify-fixture fixtures/audio-vorbis/init-native.json
```

Rusttests benötigen weder Originalinstallation noch Referenzdecoder. Der unabhängige Pythonvergleich benötigt `soundfile==0.13.1` und NumPy; die Originalausführung die bereits gepinnten Analyseabhängigkeiten. Der Aufruf `rc-vorbis-probe INPUT.ogg REPORT.json OUTPUT.s16le` lässt sich auch einzeln verwenden. Dieses Werkzeug gibt echte PCM-Dateien aus und spielt sie nicht selbst ab.
