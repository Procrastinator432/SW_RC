# Statischer Quellenpool, Aktivierung und laufendes Audio-Update

**Fortsetzung 25.10.2026:** Der [Queue-/Updatezyklus](AUDIO_QUEUE_CYCLE.md) ergänzt jetzt automatische IsFinished-Endbereinigung, exakte Prioritätssortierung, wartende Quellen und die belegte statische Phasenfolge. Die nachstehenden offenen Queue-/Endpunkte beschreiben den Ausgangsstand dieses Playbackhost-Blocks; globale Stream-/Geräteintegration bleibt offen.

Stand: 25.10.2026. Fortsetzung des [gemeinsamen Ressourcenhosts](AUDIO_RESOURCE_HOST.md). Die fünf Aufgaben sind für statische Quellen umgesetzt. Streamversorgung bleibt im Ressourcenhost; globale Engine-Reihenfolge, gemeinsame Stream-/Queueaktivierung, Geräteadapter und Appintegration bleiben offen. Keine neue APK.

## Fünf abgeschlossene Punkte

1. **Pool und Aktivierung:** `audio_playback_host::Host` verbindet die vorhandene physische Quellenstruktur mit der PCM-Bank. Der letzte Kanal beginnt als Transient; FindSource entscheidet über freie Slots, gleiche Identitäten, Schutzbit und streng niedrigere Prioritäten. Die Originalvorbereitung tauscht Slots und Transient. Die Stop-Grenzen rufen jetzt die tatsächlichen portierten FALSource-Treibermethoden auf; nach dem ersten Stop enthält der zweite Aufruf keine erfundenen AL-Aktionen. Geschützte Quellen und Gleichstände werden wie im Original behandelt. Bereits registrierte PCM-Daten werden geteilt.
2. **Parameterupload und tatsächliches Play/Pause:** Gain, Pitch, Looping, Rolloff, Position, Radius, Relative/EAX und Bufferbindung kommen aus dem vorhandenen vollständigen Uploadkern. Seine virtuellen Play-/Pause-Grenzen sind nun mit FALSource verbunden. Der Host übernimmt den ursprünglichen Lebenszeitwert beim Play-Aufruf, setzt ihn anschließend auf 0 und führt eine vorgesehene Startpause danach aus. Physischer Kanal, Sound, Buffer und Treiberzustand bleiben zwischen Ressourcenbesitzer und Pool synchron. Attach-Aufträge des Ressourcenhosts werden nicht ein zweites Mal ausgegeben; die vollständige Uploadfolge enthält bereits die Bindung.
3. **UpdateBase und Controller:** Die explizite Basisphase verwendet die bestehenden Bewegungs-, Ducking-, Fade-, Radius- und Prioritätskerne. Pause wird aus dem eigenen Treiberzustand gelesen; Actor-/Listenerdaten und Geometrieantworten bleiben Eingaben. Ein Volume-/Fade-Stop geht jetzt an den wirklichen Treiber, bevor die nachfolgenden Radius-/Prioritätscallbacks weiterlaufen. Die berechneten Sourcefelder werden in Pool und Treiber übernommen. Der öffentliche bisherige Runtime-Aufruf behält seine Standard-Clear-Semantik; der neue Stopanschluss ist eine zusätzliche interne Variante.
4. **Treibertick und Stimmenzählung:** Die separate späte Phase verwendet die tatsächlichen IsPlaying-Werte und FALSource-Tick-Ergebnisse der eigenen statischen Treiber. Dadurch stammen Gain-/Positionsaktualisierungen, f64-Lebenszeit und der nachfolgende Sourcezustand nicht mehr aus vorgegebenen synthetischen After-Werten. Der originale späte Census erhält diese Ergebnisse und die expliziten Params-Flags. Seine Stimmenzahl steuert das Ducking in der nächsten Basisphase. Die Cue-Aktualisierung verwendet ebenfalls die eigenen Cursorwerte. Pausierte Cursor werden nach den belegten Regeln gezählt.
5. **Gemeinsamer gespeicherter Fortlauf:** Schema 1 enthält Ressourcen, physische Nodes, Slot-/Transientzuordnung, unveränderliche Kanal-/Quellschlüssel, Uploadbuffer, EAX-Cache, Ducking und Censusstatus. Aktivierung und beide Updatephasen sind Zustands-/Plantransaktionen. Ein später Fehler nimmt auch vorher berechnete Stops, Parameter und Lebenszeitänderungen zurück. Laden prüft die Beziehungen zwischen Pool, Treibern und PCM-Bank; beschädigte oder widersprüchliche Snapshots ersetzen keinen gültigen Host.

## Neuer Originalvergleich

`Test-AudioPlaybackOriginal.py` führt **192 ganze originale PlaySource-Aufrufe bei `10008800`** aus, diesmal mit den tatsächlichen FALSource-Overrides für Stop (`10006760`), Play (`10004f90`) und Pause (`10005040`). Darunter sind **140 erfolgreiche Aktivierungen**. Die Fälle umfassen freie/voll belegte Pools, passende geschützte Identitäten, Verdrängung, erfolglose Aufnahme, räumliche/relative Quellen, EAX 2/3, RSQRTSS-Seeds, Startpause und alte Lebenszeitwerte.

Die Rusttests vergleichen **jede geordnete Aktivierungs-, Parameter- und Treiberaktion**, den vollständigen anschließenden physischen Quellgraphen und den EAX-Cache gegen diese Ergebnisse. Die alten reinen Basis-Play-/Pausegrenzen wurden für diesen neuen Nachweis nicht als tatsächliche FALSource-Ausführung ausgegeben.

Engine, CRT und ALAudio werden wie im bestehenden Prüfer per SHA-256 kontrolliert. Soundgültigkeit, gecachtes Precache-Ergebnis, Params-/Listenerantworten, AL-Aufrufe und Fehlerprüfungen sind kontrollierte Grenzen. Es werden keine echten Gerätepuffer angelegt oder Samples gehört. Die versionierte Fixture `fixtures/audio-playback-native.json` enthält nur synthetische Zustände, Tokens und Ergebnisse; Originalinstruktionen bleiben lokal.

Die bestehende Sourcegrundlage wurde zusätzlich frisch regressionsgeprüft: 384 Initialisierungen, 512 Base-Updates, 480 Lebenszyklusschritte und 768 zusammengesetzte Schritte sowie Such-/Matchingproben stimmen weiterhin mit ihrer eingefrorenen Fixture überein. Die zusätzliche Stopvariante verändert den bisherigen öffentlichen Runtimevertrag nicht.

## Integration und praktische Prüfung

**Neun neue Testgruppen** prüfen den neuen Originalvergleich, Transienttausch/Verdrängung/Schutz, echte Startpause mit Uhrreset, eigene Pause-/Actorbewegungsdaten, Treiber-Stop vor Radius/Priorität, reale Treiberzeit und Census-/Duckingübergabe, späte Fehleratomarität, kaltes Fortsetzen und beschädigte Verknüpfungen. Ein Verdrängungstest sichert ausdrücklich ab: gleiche Priorität verdrängt keine andere Identität; erst die höhere Priorität darf übernehmen.

Die Fehlerprüfungen erzeugen beispielsweise zuerst einen tatsächlichen Fade-Stop und lassen danach eine zweite Quellenberechnung scheitern. Pool, Treiber und Controller bleiben dabei exakt auf dem vorherigen Stand. Fehlende Params im Census nehmen auch bereits berechnete Treiberzeiten und Parameteraufträge zurück.

**Zwei praktische Läufe mit Original-WAV-Dateien** aus `gen_sound.uax` und `geo_sound.uax` verwenden jeweils zwei Slots plus Transient, drei Aktivierungen, acht Basis-/Treiberrunden und kaltes Laden nach jeder Runde. Sie bestätigen zwei gezählte Stimmen, dadurch aktives Ducking, spätere Verdrängung, einen tatsächlichen Fade-Stop und identischen vollständigen PCM-/Quellenabbau. Pro Lauf werden 40 Basis- und 96 späte Ereignisse geprüft. Das ist eine Steuerungs-/Datenprüfung ohne tatsächliche Audioausgabe.

Der vollständige zentrale Check besteht mit je **1.313 Debug-/Release-Bibliothekstests**, Format, Materialimport-/Doc-Tests, Clippy aller Targets, Native-/Inspectorbau, Java und Buildvertrag. [Lokaler Gesamtbericht](../build/audio-playback-host/verification-summary.json).

## API, Reihenfolge und Grenzen

`Host::new` beginnt mit freien Hardwarekanälen; der letzte ist der Transient. `from_prepared` übernimmt einen bereits konsistenten statischen Quellgraphen samt Ressourcen und EAX-Cache. `register_pcm` pflegt die gemeinsame Bank und ihre Uploadbeschreibungen. `play` erzeugt die komplette belegte statische Aktivierungs-/Upload-/Treiberfolge für einen bereits registrierten Sound. Fehlendes Precache oder ungültiger Sound erzeugt keine erfolgreiche Aktivierung.

`update_base` erhält je aktuellem Poolslot Actor-/Listener-/Geometrie-/Kategorieeingaben. Die beiden Pauseantworten kommen in diesem sequenziellen Host aus demselben eigenen Treiberzustand; es wird kein gleichzeitiger fremder Zustandswechsel simuliert. `late_tick` erhält pro Slot Umgebungs-/Deltawerte und die Sound-Params-Flags für den Census. `refresh_cues` sowie `set_pending` bilden die separate Cuephase ab. `unregister` stoppt die betreffenden Treiber und entfernt PCM-/Uploadbeschreibungen und verbleibende inaktive Buffertokens.

Die Reihenfolge dieser öffentlichen Phasen ist **explizit vom Aufrufer bestimmt**. Dieser Abschnitt behauptet nicht, die gesamte ursprüngliche `UAudioSubsystem::Update`-Orchestrierung, Sortierung, wartende Queue oder die Aktivierung von Streams vollständig verbunden zu haben. Der Census zählt die statischen Poolslots dieses Hosts; er ist noch keine globale Zusammenfassung mit allen Ressourcenhost-Streams. Auch das automatische Prüfen von IsFinished und Freigeben natürlich beendeter Einmalquellen benötigt die spätere globale Steuerung mit tatsächlichen Geräteantworten. Das vorhandene Driver-IsFinished ist dadurch nicht ersetzt.

Basisereignisse verwenden Slotpositionen, Treiberereignisse physische Nodeindizes. Aktivierungs-/Play-/Pausemarkierungen beschreiben die bereits aufgelösten virtuellen Grenzen; die eingefügten Driverereignisse enthalten die eigentlichen Geräteaufträge. Ein künftiger Executor darf diese Markierungen nicht nochmals als fremde Methodenaufrufe ausführen.

Die API ist zunächst auf statische PCM-Quellen begrenzt. Bereits vorhandene gestreamte Ressourcen werden nicht verdeckt in einen statischen Pool aufgenommen. Die Transient-/Buffer-/Schutzregeln und die Leer-/Nichtaufnahmezweige bleiben erhalten. Geometrieabfragen, Kategorien-/Paramswerte, Warteschlangenstatus und originale CPU-Näherungsseeds sind noch Aufrufergrenzen. Nichtlineare CurveMath-Präzision behält die bereits dokumentierten CRT-/Portabilitätsgrenzen. Externe Math-Policy-Nebeneffekte werden durch die Hosttransaktion nicht zurückgenommen.

Die eigenen Ressourcen und JSON-Snapshots behalten ihr jeweiliges 16-MiB-Limit. Maximal 4.096 statische Slots plus ein Transient sind zulässig. Ressourcenbytes stehen einmal in der Bank; Pool und Treiber halten synchron geprüfte Metadatenkopien. Die Transaktion verwendet eine strukturierte Sicherung und benötigt bei normalem Erfolg kein JSON-Neuladen. Kopien, Validierung und Fehlerwiederaufbau gehören dennoch nicht unverändert in einen Echtzeitaudiocallback.

Hardware-/Bufferwerte bleiben logische Tokens. Reale Objekterzeugung, API-Erfolg/Fehler, Wiederaufbau nach Prozessverlust und Android-App-Checkpointintegration bleiben offen. Der jetzige Snapshot ist ein internes Schema. Die zuletzt geprüfte spielbare App und eingefrorene Releases bleiben unverändert.

## Reproduzieren

```powershell
./build.ps1 -Target Check -OutputDirectory build/audio-playback-host/check
cargo build -p rc-inspect --bin rc-audio-playback-probe --release --locked --offline
target/release/rc-audio-playback-probe.exe build/audio-buffer/real-gen/00.wav build/audio-playback-host/real-gen.json
target/release/rc-audio-playback-probe.exe build/audio-buffer/real-geo/00.wav build/audio-playback-host/real-geo.json
python -X utf8 scripts/Test-AudioPlaybackOriginal.py --game-data GAME_DATA `
  --output build/audio-playback-host/original-replay.json --verify-fixture fixtures/audio-playback-native.json
python -X utf8 scripts/Test-AudioSourcesOriginal.py --game-data GAME_DATA `
  --output build/audio-playback-host/sources-replay.json --verify-fixture fixtures/audio-sources-native.json
```

`GAME_DATA` bezeichnet die lokale Originalinstallation. Die zwei WAV-Eingaben entstehen im [Buffer-Prüflauf](AUDIO_BUFFERS.md#reproduzieren). Rusttests laufen mit den synthetischen Fixtures ohne Originalinstallation; die frische Originalausführung benötigt die gepinnten Python-Analyseabhängigkeiten.

## Nächster Abschnitt

Natürliche Quellenenden und globale Update-Reihenfolge prüfen, statischen Pool mit Stream-/Queueaktivierung und gemeinsamem Census zusammenführen. Danach Geräteadapter mit tatsächlichen Antworten und Objektwiederaufbau, Workerlebensdauer sowie App-/Androidintegration und Hörabnahme ergänzen.
