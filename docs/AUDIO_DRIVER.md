# FALSource-Steuerung und Abschlussregeln

Stand: 24.10.2026. Fünf weitere **interne** Pakete nach [AL-Upload und PlaySource-Befehlsfolge](AUDIO_UPLOAD.md). Der neue Kern `audio_driver::Session` rekonstruiert die echten **FALSource-Methoden**, deren Aufrufe zuvor durch Basismethoden oder vorgegebene Antworten ersetzt waren. OpenAL selbst bleibt eine kontrollierte Auftrags-/Antwortgrenze. Es wird kein Audiogerät geöffnet und kein Ton ausgegeben; spielbare Referenz bleibt **0.60.0**.

## Die fünf Pakete

| Paket | Implementierung | Interner Nachweis |
|---|---|---|
| 1 · Play/Pause | Belegungs-Gates, ursprüngliche Schreibreihenfolge um AL-Play/-Pause, roher Pausenwert und Resume über virtuelles Play. | Ganze originale Methoden, einschließlich negativer Pausenwerte, wiederholtem Play, inaktiver Quelle und Callback-Feldzuständen vor/nach dem Aufruf. |
| 2 · Stop/Freigabe | AL-Stop, Einzelbuffer lösen oder exakt vier Einträge unqueuen, zurückgelieferte Buffer-IDs übernehmen, bedingtes Sound-SetHandle(0), Bufferreferenz nullen und originales Clear. | Normale Rückkehr durch die ganze Methode; Speicheränderungen und Reihenfolge einschließlich SetHandle vor dem Nullen der Bufferreferenz. |
| 3 · Statusabfragen | Originale lokale IsPlaying-/IsPaused-/IsQueued-Werte und separate IsStopped-Abfrage gegen AL_SOURCE_STATE. | Alle Getter sowie ganze IsStopped-Methode, rohe nichtkanonische Statuswerte und unabhängige AL-Antworten. |
| 4 · Abschlussregeln | Ganze IsFinished-Entscheidung für fehlende/inaktive Quelle, Buffer-Token, Source-/Soundobjektflags, Fade-Modus/-Grenze, Sounddauer und AL-Status. | 80 Einzelvergleiche plus wiederholte Abfragen in längeren Originalfolgen; getrennte Dauer-, AL- und frühe Rückkehrzweige. |
| 5 · Persistente Befehlsfolgen | Quellenzustand samt Hardware-/Bufferreferenz, Buffer-Backing und Soundmetadaten gemeinsam speichern; kontrollierte AL-Antworten verarbeiten, kaltes Fortsetzen und atomare Stapelübernahme. | 96 originale Folgen mit je 20 Schritten, kalter Wiederaufnahme nach jedem Schritt sowie gleicher Ausgabe bei gemeinsamer Stapelverarbeitung; späte Fehler ohne Teilübernahme. |

Code: [audio_driver.rs](../crates/rc-package/src/audio_driver.rs), [Tests](../crates/rc-package/src/audio_driver_tests.rs), [Originalprüfer](../scripts/Test-AudioDriverOriginal.py), eingefrorene synthetische Daten [audio-driver-native.json](../fixtures/audio-driver-native.json). Originalbinärbytes und Dekompilate bleiben lokal.

## Originalmethoden und beobachteter Vertrag

Geprüfte `alaudio.dll`: SHA-256 `60e0c8495a94b297c36bc96f29e4c64a208f020e1ac55f5d6848d5697cff0dcd`; zusätzliche Engine-/CRT-Hashes stehen im Fixture. Die FALSource-VTable liegt bei `1000b0b4`. Die Methodenadressen sind anhand dieser Tabelle und der Originalinstruktionen belegt, nicht aus angenommener Methodennamensreihenfolge abgeleitet.

| Methode | Adresse | VTable-Offset / Befund |
|---|---|---|
| Play | `10004f90` | `+08`; bei inaktiver Quelle Rückgabe 0, sonst AL-Play, Pause auf 0, laufenden Marker auf 1, Rückgabe 1. |
| Pause | `10005040` | `+0c`; inaktiv ohne Änderung 0. Aktiv zuerst rohen Request schreiben; ungleich 0 AL-Pause, sonst virtuelles Play. Rückgabe 1. |
| Stop | `10006760` | `+10`; inaktiv ohne Änderung 0. Aktiv AL-Stop und Freigabefolge, Rückgabe 1. |
| IsPlaying | `10004f40` | `+14`; liefert Quellenwort `+34` unverändert. |
| IsFinished | `100051c0` | `+18`; Regeln unten. |
| IsPaused | `10004f60` | `+1c`; liefert Quellenwort `+38` unverändert. |
| IsQueued | `10004f50` | `+20`; FALSource liefert stets 0. |
| IsStopped | `10005110` | direkt durch IsFinished; inaktiv 1, sonst AL_SOURCE_STATE `1010` abfragen und ausschließlich `AL_STOPPED=1014` als wahr werten. |

**Feldbezeichnung:** Das bisher im portablen `Source`-Snapshot als `cursor` gespeicherte Wort `+34` ist bei FALSource der lokale laufende Marker. Play setzt es auf 1, IsPlaying gibt es roh zurück. Es wird weder umbenannt noch als PCM-Position interpretiert. Das Wort kann in synthetischen Altzuständen auch 3 oder `ffffffff` enthalten; die Rückgabe bleibt unverändert. `requested_pause` bleibt ebenfalls ein roher Integer. Diese lokalen Marker sind von der tatsächlichen AL-Statusantwort getrennt.

### Stop und Pufferbesitz

Die ursprünglichen oberen drei Count-Tagbits werden für die Entscheidung maskiert. Bei logischer Bufferanzahl 1 wird `AL_BUFFER=0` gesetzt. In **jedem anderen Zweig** fordert Stop genau vier Unqueue-Einträge an, nicht die gespeicherte logische Anzahl. Der kontrollierte Callback schreibt vier zurückgelieferte IDs in das vorhandene Backing. Der Port trennt deshalb `count_word` und `storage` ausdrücklich. Synthetische Counts 0–5 besitzen hierfür genügend Speicher; damit wird die Argument-/Speicherfolge geprüft, keine reale AL-Gültigkeit einer leeren oder zu kurzen Gerätequeue behauptet.

Soundobjekt-Byte `+30`, Bit 1 (`2`) löst den virtuellen `SetHandle(0)`-Aufruf aus. Zu diesem Zeitpunkt sind Quelle und Bufferreferenz noch gesetzt. Danach wird die Quellenbufferreferenz genullt und die originale Basis-Clear-Feldlogik ausgeführt. Der originale `USound::SetHandle`-Körper (`103d06c0`, Sound-VTable `+94`) setzt Soundwort `+64` auf 0; diese Änderung ist in den gespeicherten Soundmetadaten enthalten. Die Namenszuordnung wurde gegen die exportierte USound-VTable geprüft: `Load` liegt dagegen bei `+ac` und wird hier nicht aufgerufen. Hardware-ID, alte Bufferbeschreibung und deren zurückgelieferte IDs bleiben im Kernzustand erhalten. Das ist kein Beleg für tatsächliche Ressourcenfreigabe durch OpenAL; deren Implementierungen bleiben externe Grenzen.

### IsFinished

1. Nicht belegt oder Bufferreferenz null: 1, ohne AL-/Soundabfrage.
2. Sourceflag `4` nicht gesetzt: IsStopped ausführen.
3. Sourceflag `4` gesetzt und Bufferwort `+4` null: virtuelle Sounddauer abfragen. Nur wenn das gespeicherte double bei Source `+18` **größer** als binary32-Sounddauer plus 0,5 ist, folgt 1. Gleichheit ist noch nicht abgeschlossen. AL-Status wird in diesem Zweig nicht abgefragt.
4. Sourceflag `4` gesetzt und Bufferwort `+4` ungleich null: Sourceflag `4000`, gehaltener Fade kleiner/gleich binary32 `38d1b717` und Fade-Modus ungleich 1 ergeben sofort 1. Sonst hält Soundobjekt-Byte `+30`, Bit 0 die Quelle mit Rückgabe 0 offen. Andernfalls IsStopped ausführen.

Bufferwort `+4` heißt hier bewusst nur `stream_token`: seine genaue Pointer-/Objektklasse wird nicht behauptet. Sourceflags, Soundobjektflags und Paramsflags sind verschiedene Felder. Diese Methode füllt keine Streambuffer nach. Die Quelle `+18` wird als vorhandener double-Wert gelesen; ihr allgemeiner Update-Zeitproduzent ist in diesem Block nicht rekonstruiert. Die unterstützte Sounddauer ist 0 oder hat einen Betrag zwischen `2^-29` und 1.000.000. Damit ist die Addition von 0,5 in binary64 exakt und liefert dieselbe Vergleichsschwelle wie der hier ausgeführte x87-Pfad. Kleinere nichtnullige Dauern werden ausdrücklich abgewiesen: Dort könnten Rundung an der Schwelle und x87-Zwischenpräzision auseinanderfallen. Diese Vertragsgrenze ist eine Portvalidierung, kein behauptetes Originalgate.

## Zustands- und Fehlergrenzen

`Session` speichert einen physischen Quellenknoten und getrennte Buffer-/Soundbeschreibungen. `Reply` enthält die tatsächliche beziehungsweise im Test kontrollierte AL-Statusantwort und vier Unqueue-Ausgabe-IDs. Nicht abgefragte Antworten beeinflussen den Zustand nicht. Jede Methode beziehungsweise jeder Stapel wird zunächst vollständig auf einer Kopie geplant. Fehlende benötigte Ressourcen oder zu kurzes Unqueue-Backing lassen den bisherigen Zustand unverändert. Das kann bereits extern ausgeführte Geräteaktionen nicht zurückrollen.

Snapshot-Schema 1 ist ein eigenes internes Format. Es integriert sich nicht automatisch in App-Spielstände, `audio_handoff` oder `audio_upload::Session`. Unbekannte Felder, falsches Schema, nichtendliche Sound-/Quellenwerte, doppelte Handles, zu große Bänke oder Eingaben über 16 MiB werden abgewiesen. Grenzen: je 4.096 Buffer-/Soundbeschreibungen, maximal 4.096 Backing-Einträge pro Buffer und 65.536 insgesamt, maximal 4.096 Schritte pro Stapel. Inaktive Getter benötigen keine Soundbeschreibung. Für eine aktive IsFinished-Abfrage mit nichtnulliger Bufferreferenz verlangt der Port immer eine vorhandene Bufferbeschreibung, auch wenn der originale Nichtstreamzweig nur den AL-Status abfragt. Das ist eine zusätzliche Validierung der portablen Ressourcenbank; fehlende Metadaten sind kein nachgebildeter nativer Fehlerzustand.

## Original- und Quellprüfung

**800 ganze Einzelmethodenaufrufe plus 1.920 Schritte in 96 Originalfolgen**. Je 80 Einzelaufrufe für Play, Stop, IsPlaying, IsPaused, IsQueued, IsStopped und IsFinished sowie 240 Pauseaufrufe mit drei Requestvarianten. Die Abschlussfälle umfassen Dauerzweige, AL-Statuszweige und frühe Rückgaben (genaue Zählung im Coveragebericht); die aktiven Stopfälle decken alle logischen Bufferanzahlen 0–5 ab. Acht neue Rust-Testgruppen vergleichen Rückgaben, Aufträge, Callback-Feldzustände, Source-/Buffer-/Soundzustände, kalte Wiederaufnahme und Fehlerübernahme.

```bash
source scripts/env-linux.sh
python scripts/Test-AudioDriverOriginal.py --game-data "$RC_GAME_DATA" \
  --output build/audio-driver/original-replay.json \
  --verify-fixture fixtures/audio-driver-native.json
./build.sh -Target Check -OutputDirectory build/audio-driver/check
```

**Zentralcheck bestanden:** je **1.256 Debug-/Release-Bibliothekstests**, Format, Materialimport-/Dokumentationstests, Clippy mit Warnungen als Fehler, Native-/Inspector-Builds, Java und Buildvertrag. Alle **1.001** erfassten Quell-/Skript-/Konfigurations-/Fixtureeingaben blieben während des endgültigen Checks unverändert und waren vor dessen Beginn geschrieben. Der frische Originalvergleich bestätigt das endgültige Fixture vollständig.

Lokale Nachweise: `build/audio-driver/original-replay.validation.json`, `coverage.json`, `check/validation.json`, `source-inputs.json` und `verification-summary.json`. Disassemblierung und private Analyse: `build/audio-driver/research/driver-disassembly.txt` sowie `analysis/decompiled/audio-driver.c`. Der frische Fixturevergleich erfolgt nach dem Einfrieren; der Abschlussbericht hält die tatsächlichen Zentralcheckergebnisse und unveränderten Quelleingaben fest.

## Weiter offen und Windows-Abnahme

OpenAL-API, GetState-/Unqueue-Antworten und Sound.GetDuration sind kontrollierte Grenzen. Der Originalprüfer führt die echten FALSource-Methodenkörper, USound::SetHandle und Basis-Clear aus; der Rustkern erzeugt die entsprechenden Befehle und Zustände. Das bestätigt noch keinen laufenden PC-/Android-Audiotreiber. Ursprüngliche SEH-Fehler-/Unwindpfade, echte OpenAL-Fehler und Nebenwirkungen anderer Soundüberschreibungen sind nicht geprüft.

Offen bleiben PCM-/Bufferanlage und -upload, tatsächlicher Ressourcenbesitz/-abbau, Streamingnachfüllung, FALSource-Update und dessen Uhr-/Mixerpfad, Zusammenführung dieser Methoden mit Pool/Upload/Census, globale Update-Sortierung/-Budgets und allgemeiner Spiel-/Androidanschluss. Als nächste interne Kandidaten bieten sich Bufferlebensdauer, Streamnachfüllung und die FALSource-Update-Regeln an; sie brauchen eigene Originalvergleiche.

Keine neue APK, kein Missions-/Androidlauf und keine neue Hörabnahme. Die auf Nutzerwunsch zurückgestellte [Windows-Prüfliste](AUDIO_GEOMETRY_RUNTIME.md#windows-abnahme-bewusst-zurückgestellt) bleibt offen. Nach späterer Treiberanbindung zusätzlich Play/Pause/Resume, Stop während Loop/Stream, natürlichem Ende, Fadeende und kaltem Missionsfortsetzen prüfen und hören. Spielbare Referenz bleibt **0.60.0**.
