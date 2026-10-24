# Byteführender PCM-Worker und Treiberdatenpfad

**Nachfolgende Umsetzung 25.10.2026:** [Realer Vorbis-Dateipfad](AUDIO_VORBIS.md) liefert nun in einem neuen privaten Host echte Decoderbytes an diesen Worker und die Treiberbridge. Die hier dokumentierten PCM-/Raw-Schnittstellen behalten ihre Verträge und ihr Schema; die unten genannten Codecgrenzen beschreiben diesen früheren Baustein.
Stand: 25.10.2026 · interner Datenpfad nach [AUDIO_SOURCE_TICK.md](AUDIO_SOURCE_TICK.md). Noch keine allgemeine App-/Androidanbindung und keine neue APK.

## Fünf abgeschlossene Punkte

1. **Anforderung und Status:** Ganze originale RequestChunks-/QueryStream-Regeln umgesetzt. Neue Anfragen ersetzen Anzahl und Zieladresse. Query meldet die offene Anzahl als Busy-Wort und genau dann Verfügbarkeit, wenn das Endwort 0 ist. Die ursprünglichen kritischen Abschnitte sind als Lock-/Unlock-Grenzen erfasst; der portable Host führt Befehle deterministisch nacheinander aus.
2. **Partielle PCM-Chunkfüllung:** Der ganze Ogg-ReadChunks-Steuerkörper schreibt echte Bytes in eigene Speicherregionen. Er zieht die angeforderte Chunkzahl ab, begrenzt die Restanforderung auf 0 und setzt den Zielcursor vor den Decoderaufrufen weiter. Positive Teillieferungen werden bis zur gewünschten Bytezahl zusammengesetzt. Nicht angeforderte Bytes und Guardbereiche bleiben erhalten. Die Raw-Schicht bewahrt auch die ursprünglichen Null-/negativen Zählzweige und 32-Bit-Arithmetik, solange kein unzulässiger Speicherzugriff verlangt wird.
3. **Loop, Seek, EOF und Fehler:** Nur Typwert 1 schleift. EOF löst dann einen Seek auf 0 aus und erhöht den Loopzähler; die Originalroutine ignoriert dessen Rückgabewert. Ohne Loop wird der Rest mit Nullbytes gefüllt und das Endwort auf 1 gesetzt. Ein negativer Decoderfehler füllt ebenfalls mit Nullbytes, setzt das Endwort aber nicht. Der explizite Seek reicht die f32-Zeit als f64 an den Decoder weiter, meldet Erfolg nur für Rückgabe 0 und löscht selbst keine Streamfelder.
4. **Bytebesitz und Treiberanschluss:** Der neue Bridge-Host verbindet den vorigen FALSource-Tick mit dem PCM-Worker. Query-/Request-Antworten entstehen nun aus dessen tatsächlichem Zustand. BufferData kopiert die Stagingbytes in eigene Uploadbuffer; die Original-Puffer-ID-Regeln bleiben erhalten. DestroyStream entfernt Worker und den eigenen PCM-Provider, Free entfernt anschließend die Stagingallokation. Bereits hochgeladene Bufferbytes bleiben erhalten, weil der originale Tick an dieser Stelle ebenfalls keine AL-Buffer löscht.
5. **Gespeicherter Datenfortlauf:** Eigene Snapshot-Schemata 1 enthalten PCM-Daten, Decoderposition, Stagingbytes, Uploadbuffer und sämtliche Kontroll-/Zeit-/Indexwerte. Service- und Treiberbefehle werden atomar ausgeführt; ein fehlgeschlagener Stapel gibt keine teilweise veränderten Daten frei. Kaltes Laden nach jedem Schritt bestätigt Steuerung und Byteinhalte gegen Originalfolgen. Verschobene Freigabetokens, ungültige Cursor, überlappende Speicherregionen und beschädigte Snapshots werden abgewiesen.

## Originalnachweis

Die Hash-geprüften PC-Dateien sind dieselben wie im [Treibertick-Nachweis](AUDIO_SOURCE_TICK.md). Engine, CRT und ALAudio werden vom gemeinsamen Prüfer geladen; seine drei SHA-256 werden vor Ausführung geprüft. Die neue versionierte Fixture enthält ausschließlich synthetische PCM-Muster, Zustände, opake Tokens und Ergebnisse.

| Funktion | Adresse | Nachweis |
|---|---|---|
| FALAudioStream.RequestChunks | `10003020` | Ganze Funktion einschließlich kritischem Abschnitt und überschriebenem Auftrag. |
| FALAudioStream.QueryStream | `10003050` | Ganze Funktion, Busy-Wort und Rückgabe. |
| FALAudioStreamOgg.ReadChunks | `100036a0` | Ganze Funktion, tatsächliche Zielbytes und alle relevanten Streamfelder. |
| FALAudioStreamOgg.Seek | `10003230` | Ganze Funktion mit kontrollierter Codecantwort. |
| FALSource-Treiberupdate | `10006270` | In den zusammengesetzten Folgen tatsächlich ausgeführt; QueryStream/RequestChunks werden dort nicht mehr durch vorgegebene Busy-/Available-Werte ersetzt. |
| Streammanager-Worker | `10003600` | Untersucht: ruft je Stream ReadChunks(1), ignoriert dessen Rückgabe und wartet zwischen Runden 50 ms. Kein neuer Betriebssystemthread oder Echtzeittimer im Port. |

**512 Einzelaufrufe plus 960 Schritte in 48 Originalfolgen**, zusätzlich **408 zusammengesetzte Schritte in 32 Folgen**: 204 originale ReadChunks-Services und 204 originale Treiberticks. Die zusammengesetzten Tests enthalten 188 tatsächliche PCM-Übergaben an den kontrollierten AL-BufferData-Aufruf und 16 Streamenden. Sie vergleichen auch die vom Original übergebenen Bytes vollständig. Zehn neue Testgruppen prüfen diese Ergebnisse, Bounds, Fehleratomarität, Busy-Verhalten und kalte Wiederaufnahme nach jedem Schritt.

Der zentrale Check besteht mit je **1.274 Debug-/Release-Bibliothekstests**, Format, Materialimport-/Doc-Tests, Clippy aller Targets, Native-/Inspectorbau, Java und Buildvertrag.

## Schnittstellen und Grenzen

`audio_stream_data::Pcm` liefert bereits dekodierte, eigene Bytearrays mit gespeichertem Cursor und begrenzter Teilleselänge. Es ist ein deterministischer PCM-Provider, kein Vorbisdecoder. Die zusammengesetzte Originalausführung ersetzt ov_read/ov_time_seek, AL-Status/-Unqueue und die tatsächlichen Gerätekopien durch protokollierte Antworten. Die Original-Steuerkörper und Speicheroperationen laufen tatsächlich. Synthetische Antworten enthalten auch ungerade Block-/Datenlängen zur Bytegrenzprüfung; daraus wird keine erfolgreiche Dekodierung einer echten Ogg-Datei abgeleitet.

Der Raw-Seek bildet den Decoderaufruf und die Rückgabe ab. Eine allgemeine zeitbasierte Positionierung des eigenen PCM-Providers wird damit nicht behauptet; dessen Loop-Rücksprung auf Byteposition 0 ist umgesetzt. Codecinterne Zustände, Vorbis-Dateihandles, reale kritische Abschnitte und Thread-Wartezeiten bleiben außerhalb dieses serialisierbaren Modells. Der Bridge-Host ist ein neuer portabler Ressourcenbesitzer und nicht die vollständige originale FALBuffer-/RegisterSound-Implementierung.

Die Reihenfolge bei Decoderfehlern ist bewusst erhalten: Weil der Originalmanager den ReadChunks-Rückgabewert ignoriert, wird ein Fehler nicht eigenmächtig in EOF umgedeutet. Eine leere endlos schleifende Quelle würde im Original ohne Fortschritt weiterlaufen; der Port bricht nach höchstens 65.536 Decoderaufrufen atomar ab. Er setzt dafür kein erfundenes Endflag.

Portgrenzen: 16 MiB für eigene Bytebestände und kodierte Snapshots, je 4.096 Speicher-/Bufferbeschreibungen beziehungsweise Stapelschritte, maximal 65.536 Decoderantworten und disjunkte nichtnullige 32-Bit-Adressregionen. Zugriffe müssen vollständig in einer eigenen Allokation liegen. Der Bridge-Stagingtoken muss deren Basis bezeichnen; offene Schreibcursor müssen adressierbar sein. Fehlende Ressourcen, Nullbuffer-Uploads und nichtendliche Seekwerte werden atomar abgewiesen. Tokens sind keine dereferenzierten nativen Pointer.

Die Bridge nutzt ein eigenes Schema und wird nicht automatisch in App-Checkpoints, UpdateBase, Pool, Upload oder Stimmenzählung eingebaut. Uploadbuffer sind eigene Bytekopien und keine tatsächlich angelegten OpenAL-Objekte. Der allgemeine AL-/Androidadapter und tatsächliche API-Fehler müssen separat angeschlossen werden. Die bisherigen App-Quellen und angenommenen Lieferdateien bleiben unverändert.

## Reproduzieren

```powershell
./build.ps1 -Target Check -OutputDirectory build/audio-stream-data/final-check
python -X utf8 scripts/Test-AudioStreamDataOriginal.py `
  --game-data 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' `
  --output build/audio-stream-data/original-replay.json `
  --verify-fixture fixtures/audio-stream-data-native.json
```

Für den frischen Originalvergleich gelten die gepinnten Python-Analyseabhängigkeiten. Rusttests laufen mit der synthetischen Fixture ohne Originalinstallation. [Gesamtbericht](../build/audio-stream-data/verification-summary.json) · [Quellzuordnung](../build/audio-stream-data/source-and-verification.json)

## Anschließende Arbeit

Als Nächstes: tatsächliche Vorbis-/Dateibereitstellung und originale Format-/Bufferregistrierung, danach Quellenwrapper/UpdateBase und den Datenpfad in einem gemeinsamen Host zusammenführen. Globale Streammanager-/Threadlebensdauer, AL-Buffererzeugung/-abbau, echte API-Fehler und allgemeine App-/Androidanbindung bleiben offen. Erst deren Integration erfordert eine neue hörbare APK-/Missionsabnahme; die letzte App-Prüfung bleibt der [Windows-/Androidlauf von e6a2438](WINDOWS_VALIDATION_20261025.md).
