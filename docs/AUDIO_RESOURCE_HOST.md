# Gemeinsamer Audioressourcenhost

**Nachfolgende Umsetzung 25.10.2026:** [Statischer Quellenpool und Playbackhost](AUDIO_PLAYBACK_HOST.md) verbindet diesen Besitzer für statische Quellen mit Aktivierung, vollständigem Parameterupload, tatsächlicher Treibersteuerung, UpdateBase und spätem Census. Gemeinsame globale Stream-/Queueorchestrierung und Geräte-/Appintegration bleiben offen; die unten genannten nächsten Schritte beschreiben den vorherigen Ausgangspunkt.

Stand: 25.10.2026. Fortsetzung der [WAV-/Bufferverwaltung](AUDIO_BUFFERS.md). `audio_resource_host::Host` führt die bisher getrennten PCM- und Vorbisbesitzer zusammen. Dies ist ein portabler Ressourcen- und Steuerungshost; Geräteadapter, allgemeiner Quellenpool und Appanschluss bleiben offen. Keine neue APK.

## Fünf abgeschlossene Punkte

1. **Gemeinsame Registrierung:** Ein Host besitzt statische Soundregistrierungen, deren Quellen und mehrere Vorbis-Streams. Cachetreffer laden weder WAV noch Ogg erneut und erzeugen keine zweiten Uploads. Sound-, Buffer-, Allokations-, Decoder-, Stream-, Quellen- und Hardwarekennungen werden auf Konflikte in ihrem jeweiligen Namensraum geprüft. Statische und gestreamte AL-Buffer teilen eine Besitzprüfung und ein gemeinsames logisches 16-MiB-Bytebudget. Ein fehlgeschlagener PCM-Upload darf insbesondere keinen bereits einem Stream gehörenden Buffer zur Löschung einplanen.
2. **Statische Quellen und Treibersteuerung:** Mehrere vorbereitete Quellen können dieselbe PCM-Registrierung verwenden; die Samplebytes werden nur einmal gespeichert. Attach liefert die Bufferlösung und direkte AL-Bindung als Aufträge. Play, Pause, Stop und Status verwenden den bereits originalgeprüften FALSource-Treiberkern. `detach_static` löst eine Quelle und macht Schlüssel und Hardwarekanal wieder verfügbar, während die PCM-Registrierung im Cache bleibt. Vorbis-Quellen verwenden ebenfalls die Treibersteuerung; ihr Stop läuft über die vollständige Ressourcenfreigabe. Neue Streams liefern ihre Vorabuploads und anschließend Bufferlösung, Queuebindung und Abschaltung des geräteseitigen Loopings; das Wiederholen erledigt der Decoderworker.
3. **Begrenzte Streamversorgung:** `service_round` bedient jeden angeforderten Stream höchstens einmal pro Runde und höchstens 64 Chunks pro Aufruf. Ein gespeicherter umlaufender Cursor verhindert, dass kleine Budgets ständig dieselben Streams bevorzugen. Fertige/auftragslose Streams werden übersprungen. Die normalen Services dekodieren am laufenden Cursor weiter. `tick_stream` verbindet die tatsächlichen Stagingbytes mit dem bestehenden originalgeprüften Treibertick und prüft die Unqueue-Antwort gegen die eigene Ringreihenfolge. Pause und natürliche Streamenden wirken auf die jeweilige Quelle; andere Ressourcen bleiben erhalten.
4. **Gemeinsames Abmelden und Shutdown:** Statisches Unregister stoppt alle gebundenen Treiber in Quellenreihenfolge, fügt deren tatsächliche Steueraufträge an den Stop-Grenzen ein und löscht den geteilten Buffer anschließend genau einmal. Stream-Unregister verwendet den vorhandenen vollständigen Vorbis-Disposepfad. Entfernte Quellen und Streams verschwinden aus der Bindungsübersicht. `shutdown` plant die gesamte Freigabe atomar: Fehlt eine Antwort oder verliert eine späte Unqueue-Antwort eigene IDs, bleibt der ursprüngliche Host vollständig erhalten und kein Auftragspräfix wird zurückgegeben. Erfolgreicher Shutdown ist wiederholbar und sperrt neue Arbeit.
5. **Gemeinsames Speichern und Fortsetzen:** Schema 1 enthält PCM einmal je Registrierung, Quellen-/Treiberzustände, komprimierte Streamdaten, Decoderbytepositionen, Staging/Uploads, Besitzkennungen, Rundencursor und Shutdownzustand. Laden rekonstruiert Decoder und prüft alle Verknüpfungen erneut. Fehler ersetzen den bisherigen Host nicht. Eine Versorgungsrunde nimmt bei einem Fehler auch die bereits bedienten Streams zurück. Der seltene Shutdown verwendet eine begrenzte strukturierte Kopie statt einer JSON-Zwischendatei; dadurch blockiert eine große, gültige Ressourcenmenge ihre Freigabe nicht am separaten JSON-Snapshotlimit.

## Nachweise

**Zehn neue Integrationstestgruppen** prüfen gemischte Registrierung und Cachetreffer, gemeinsame PCM-Nutzung, erneute Hardwarebelegung nach Detach, Quellensteuerung, begrenzte Rotation samt kaltem Fortlauf, globale Token-/Speichergrenzen, Pause/Streamende, vollständigen Shutdown und beschädigte Snapshots.

Dabei laufen **17 vorhandene Originaltreiberfälle durch den neuen Host**. Die ausgegebenen Treiberereignisse und der anschließende Quellzustand stimmen mit den eingefrorenen Originalergebnissen überein. Der Originalprüfer wurde zusätzlich frisch ausgeführt und bestätigt weiterhin alle **800 Einzelaufrufe plus 1.920 Schritte in 96 Folgen** gegen diese Fixture. Das ist eine neue Integrationsprüfung vorhandener Originalkerne und keine neue Zählung dekompilierter Funktionen.

Die Freigabegrundlage wurde ebenfalls frisch gegen das Original bestätigt: 192 WAV-Auslesevorgänge, 256 Release-/Destruktoraufrufe, 32 Konstruktionen und je 64 Register-/Unregisterwrapper. Ihre Callbackgrenzen bleiben im [Bufferbericht](AUDIO_BUFFERS.md) beschrieben. Der gemeinsame Host ist eine portable Komposition dieser Verträge; es wird keine bitgenaue Nachbildung eines bisher ungetesteten globalen Engine-Updates behauptet.

Ein gezielter Fehlerfall verändert das letzte Audiopaket einer synthetischen Ogg-Datei und berechnet eine gültige Seiten-CRC neu. Header und Vorbefüllung sind damit lesbar, eine spätere Service-Runde scheitert im zweiten Decoder. Der Test bestätigt, dass auch der erste bereits bediente Stream und der Rundencursor exakt auf dem vorherigen Stand bleiben. Ein weiterer Test scheitert erst beim zweiten Stream im Shutdown und erhält trotzdem die vorherigen PCM-Quellen und beide Streams vollständig.

**Zwei gemischte Originalasset-Szenarien** verwenden jeweils einen echten WAV-Sound mit zwei Quellen und zwei echte Ogg-Dateien. Zusammen sind vier Musikdateien abgedeckt, darunter `anticipation 1.ogg` und kurze schleifende Overlays. Pro Szenario laufen acht Versorgungs-/Treiberrunden mit 16 Services und 16 Byteuploads. Nach jeder Runde wird der gesamte Host kalt rekonstruiert und gegen den laufenden Zustand verglichen. Auch gemeinsames PCM-Unregister und abschließender Shutdown erzeugen identische Aufträge und gespeicherte Endzustände. Die Berichte enthalten keine Original-PCM-Arrays.

Der zentrale Check besteht mit je **1.304 Debug-/Release-Bibliothekstests**, Format, Materialimport-/Doc-Tests, Clippy aller Targets, Native-/Inspectorbau, Java und Buildvertrag. [Lokaler Gesamtbericht](../build/audio-resource-host/verification-summary.json).

## Verwendung und Grenzen

`Host::new(owner)` beginnt leer. `register_pcm` übernimmt WAV und explizite Gerätekontrollantworten; `attach_static` übernimmt eine vorbereitete Quelle und deren Soundmetadaten. `register_stream` übernimmt Ogg-Bytes, einen vorbereiteten Treibertick und Ressourcentokens. Bei neuer Streamregistrierung sind zuerst `Registered.uploads` und danach die Bindungsaufträge in `Registered.events` zu verarbeiten. Cachetreffer liefern keine zweite Bindung oder Uploads.

`control_static` / `control_stream` liefern die Treiberergebnisse; `service_round` liefert Workerfortschritt und `tick_stream` die Nachfüll-/Treiberaufträge. `unregister` arbeitet nach Soundkennung; für Streams benötigt es eine aktuelle AL-Unqueue-Antwort. `shutdown` benötigt genau eine Antwort je registriertem Stream. Die Ausgabe enthält auch semantische Callback-/Stopmarkierungen. Diese markieren die bereits aufgelösten Grenzen und sind keine zusätzlich auszuführenden Geräteaufrufe.

Ein gestoppter statischer Treiberslot bleibt bis zum Detach oder Unregister bekannt. Ein gestreamter Sound besitzt in diesem Host genau eine Registrierung/Quelle. Ein beendeter Stream muss vor erneuter Registrierung abgemeldet werden; automatisches Ersetzen eines noch auslaufenden Streams oder Crossfades mit mehreren Generationen desselben Sounds werden nicht erfunden. Quellen sind bereits vorbereitete Slots. Poolauswahl, Aktivierung, Initialparameter-/EAX-Upload, UpdateBase, globale Stimmenzählung und deren Originalreihenfolge werden in diesem Abschnitt nicht automatisch ergänzt.

Die Rotation und das Chunkbudget sind eine portable Hostentscheidung. Der untersuchte Originalmanager besucht seine Liste, ruft je Stream ReadChunks(1) auf und wartet zwischen Durchgängen 50 ms. Hier werden kein Betriebssystemthread, keine Wartezeit und keine Echtzeitdeadline implementiert. Vorbisdecoder, ursprünglicher ReadChunks-Kern und FALSource-Tick behalten ihre eigenen beschriebenen Verträge. Die normale Runde kopiert Wiederherstellungsdaten; bei Fehlern, Laden oder Shutdown kann Nachdekodieren nötig sein. Dieser Host gehört nicht unverändert in einen Echtzeitaudiocallback.

Das Gesamtbudget beträgt 16 MiB für eigene PCM-/komprimierte-/Staging-/Uploadbytes, maximal 4.096 Registrierungen und Quellen insgesamt, höchstens 64 Chunkservices pro Runde. JSON-Snapshots besitzen ein separates 16-MiB-Limit und können dieses vor dem logischen Bytebudget erreichen. Freigabe und Shutdown funktionieren auch dann. Decoderinterner temporärer Speicher ist keine durch dieses logische Budget gemessene Spitzenbelegung.

Alle Kennungen bleiben opake Tokens. Es werden keine echten AL-Objekte angelegt, abgespielt oder zerstört. Geräteseiteneffekte können nach Ausführung nicht durch die Zustandsatomarität rückgängig gemacht werden. Ein späterer Adapter muss Antworten/Fehler behandeln und nach Prozessverlust neue Geräteobjekte anlegen und logischen Ressourcen zuordnen. Der neue Snapshot ist kein App-Checkpointformat. Eine neue Android-/Hörabnahme bleibt nach der Geräte-/Appintegration erforderlich.

## Reproduzieren

```powershell
./build.ps1 -Target Check -OutputDirectory build/audio-resource-host/final-check
cargo build -p rc-inspect --bin rc-audio-host-probe --release --locked --offline
target/release/rc-audio-host-probe.exe fixtures/audio-vorbis/host-tick.json `
  build/audio-buffer/real-gen/00.wav 'GAME_DATA/Music/musgen_vodeostorch_lp.ogg' `
  'GAME_DATA/Music/anticipation 1.ogg' build/audio-resource-host/mixed-original-a.json
python -X utf8 scripts/Test-AudioDriverOriginal.py --game-data GAME_DATA `
  --output build/audio-resource-host/driver-replay.json --verify-fixture fixtures/audio-driver-native.json
python -X utf8 scripts/Test-AudioBufferOriginal.py --game-data GAME_DATA `
  --output build/audio-resource-host/buffer-replay.json --verify-fixture fixtures/audio-buffer-native.json
```

`GAME_DATA` bezeichnet die lokale Installation. Die WAV-Dateien entstehen durch den im [Bufferbericht](AUDIO_BUFFERS.md#reproduzieren) dokumentierten Probe-Lauf. Das zweite gemischte Szenario verwendet `real-geo/00.wav`, `musgen_kotewspr_ol.ogg` und `musyyy_overlay21_ol.ogg`. Rusttests benötigen keine Originalinstallation; frische Originalausführung die gepinnten Python-Analyseabhängigkeiten.

## Nächster Integrationsabschnitt

Den gemeinsamen Besitzer mit Aktivierung/Pool, UpdateBase, Parameterupload und Stimmenzählung verbinden. Danach Geräteadapter samt Objektwiederaufbau und Fehlerbehandlung, tatsächliche Workerlebensdauer und App-/Androidanbindung ergänzen. Die zusammengeführten Ressourcen sind die Grundlage dafür; diese weiteren Systeme sind noch offen.
