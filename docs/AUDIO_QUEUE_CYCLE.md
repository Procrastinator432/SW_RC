# Statische Warteschlange und begrenzter Audio-Updatezyklus

**Fortsetzung 26.10.2026:** [Statische Audiobefehle und Sonderreservierung](AUDIO_QUEUE_CONTROL.md) ergänzt jetzt den namensabhängigen räumlichen Reservierungszweig, tatsächliche Pause-/Stop-/Fadebefehle, Cue-/Ressourcenverwaltung und gespeicherte Konfiguration. Die nachfolgende pauschale Ablehnung räumlicher Sonderquellen beschreibt den Ausgangsstand; nun sind diese mit geprüften Namensmetadaten zulässig. Stream-/Geräteintegration bleibt offen.

Stand: 25.10.2026. Fortsetzung des [Playbackhosts](AUDIO_PLAYBACK_HOST.md). Die fünf ausgewählten Punkte verbinden statische Quellen innerhalb eines belegten Ausschnitts von `UAudioSubsystem::Update`. Allgemeine Streamaktivierung, räumliche Sonderreservierung und Geräte-/Appanbindung bleiben offen. Keine neue APK.

## Fünf umgesetzte Punkte

1. **Natürliche Enden bereinigen.** `cleanup_finished` fragt jeden aktuellen Hardwarekanal mit dem portierten `IsFinished` ab und führt bei Abschluss den tatsächlichen Treiber-Stop aus. Auch inaktive Kanäle werden geprüft. Geräteantworten bleiben explizite Eingaben; Cursor oder vergangene Zeit ersetzen keine AL-Statusabfrage. Die PCM-Registrierung bleibt für spätere Wiederverwendung erhalten.
2. **Originale Prioritätsreihenfolge.** `audio_queue_order` bildet den vollständigen Sortierer einschließlich seiner instabilen Gleichstandsreihenfolge nach. Bis acht Elemente verwendet das Original eine Auswahl-/Tauschschleife, darüber eine iterative Partitionierung. Der Überlast-Cutoff ist die gegen null abgeschnittene Priorität des ersten Elements außerhalb des Kanalbudgets. Gleiche oder niedrigere Prioritäten werden bei der Aufnahme übersprungen; geschützte laufende Quellen bleiben erhalten.
3. **Eigene wartende Quellen.** `audio_queue_cycle::Host` initialisiert Quellen mit dem vorhandenen originalgeprüften Sourcekern, besitzt ihre Metadaten und speichert den überlaufenden ID-Zähler. Angeforderte Startpause verhindert das Basisupdate einer wartenden Quelle nicht: Deren ursprüngliche Basisvtable meldet `IsPaused=0`, `IsPlaying=1`, `IsQueued=1`. Die Startpause geht erst nach Aktivierung an den tatsächlichen Treiber. Abgebrochene Anforderungen bleiben bis zur anschließenden Queueleerung im Besitz des Hosts.
4. **Zusammenhängender statischer Zyklus.** Ducking wird einmal mit dem vorherigen Census und auf 0–1 begrenztem globalem Delta aktualisiert. Dann folgen Endbereinigung, Queue und spielende Hardwarequellen in ursprünglicher Sammelreihenfolge, Basisupdates mit individuellen Listenerdeltas, Sortierung/Cutoff, Stops unter dem Budget, Aufnahmeversuche, tatsächlicher später Treibertick/Census und vollständige Queueleerung. Abschließend werden `ProcessContext` und `SuspendContext` als geordnete Plattformaufträge ausgegeben. Es gibt keine Nachfüllaufnahme jenseits der ersten N sortierten Kandidaten. Die originale kleine räumliche Quelle wird bei Dauer >2 Sekunden und Radius <0,001 vor dem Play-Aufruf gestoppt.
5. **Gesamten Zyklus speichern und zurücknehmen.** Schema 1 enthält Playback-/Ressourcenzustand, wartende Quellen und Zähler. Wiederaufnahme vor und nach Aktivierungen liefert identische Ereignisse und Folgezustände. Ein später Fehler im Census nimmt bereits berechnete Endbereinigung, Aufnahme, Treiberzeit und Ducking zurück; beschädigte Snapshots ersetzen keinen gültigen Host.

## Originalbefund und Nachweise

Die vollständige `CleanupFinishedSounds`-Funktion bei `100043d0` wird in 96 synthetischen Fällen mit den tatsächlichen originalen `IsFinished`-/`Stop`-Methoden ausgeführt. Die Rusttests vergleichen jede geordnete Status-/Stop-/AL-Aktion und den vollständigen anschließenden physischen Pool. Die Fälle enthalten freie/belegte Kanäle, Cursor-/Pausezustände und AL-Zustände Initial, Playing, Paused, Stopped sowie 0. AL-Antworten, Soundmetadaten und bereits registrierte Einzelbuffer sind kontrollierte Grenzen.

Der vollständige Sortierer bei `103d49d0` wird in weiteren 96 Fällen ausgeführt: 0, 1, 2, 7, 8, 9, 10, 17, 32, 64, 128 und 256 Elemente mit gleichen, bereits geordneten und gemischten Prioritäten, Bruchteilen, negativen Werten und vorzeichenbehafteter Null. Verglichen wird die gesamte Indexpermutation, nicht nur eine absteigende Wertefolge. Engine-/CRT-/ALAudio-Hashes werden vom bestehenden Originalprüfer kontrolliert. Fixtures enthalten ausschließlich synthetische Daten.

Die Reihenfolge und Grenzen stammen aus dem Assembler von `UAudioSubsystem::Update` bei `103d9160`; `PlatformUpdate` bei `10004420` enthält nur die beiden Kontextaufrufe. Das ältere Update-Dekompilat stellt dynamische Quelllisten teilweise fälschlich als leere Schleifen dar. Es wurde deshalb für diese Schleifen durch Disassemblybefunde ersetzt. Die gesamte globale Updatefunktion wurde in diesem Block nicht als Blackbox emuliert.

Vierzehn neue Testgruppen prüfen zusätzlich Schutzbit, Queueverwerfung, abgeschnittenen Cutoff, fehlendes Backfill, strikte Dauer-/Radiusgrenzen, einmaliges Ducking, Startpause, ID-Überlauf, abgewiesene Initialisierung, beschädigtes Laden, späte Fehleratomarität und acht kalte Wiederaufnahmen. Die vorhandenen Source-Originalfälle werden frisch regressionsgeprüft.

`rc-audio-cycle-probe` verarbeitet je eine echte WAV aus `gen_sound.uax` und `geo_sound.uax`: zwei Kanäle plus Transient, fünf Queueanforderungen, vier erfolgreiche Aktivierungen, acht kalt verglichene Runden und zwei Abschlusszyklen. Ein absichtlich fehlerhafter später Census muss vollständig zurückrollen. Abschließend sind alle Quellen beendet und das PCM bleibt gecacht. Dies prüft Daten und Steuerungsaufträge ohne Audioausgabe.

Der zentrale Check besteht mit je **1.327 Debug-/Release-Bibliothekstests**, Format-, Material-/Doc-Tests, Clippy aller Targets, Native-/Inspectorbau, Java und Buildvertrag. Er wird im [lokalen Prüfverzeichnis](../build/audio-queue-cycle/check/validation.json) dokumentiert. Der [Gesamtbeleg](../build/audio-queue-cycle/verification-summary.json) verknüpft Original-Replays, praktische Läufe und Quellhashes.

## API und verbleibende Grenzen

Zuerst PCM im Playbackhost registrieren, dann den Queuehost mit diesem konsistenten Besitzer erzeugen. `enqueue` erhält Source-Init und statische Soundmetadaten; `pause` und `cancel` ändern wartende Anforderungen. `cycle` erhält je aktuellem Hardwarekanal eine Abschlussantwort und Basisdaten, je wartender Quelle Basis-/Umgebungsdaten und je physischem Node einen späteren Treiberframe. Letztere werden nach Slot-/Transienttausch neu zugeordnet. `checkpoint`, `restore` und `load` speichern und prüfen die gesamten Beziehungen.

Der Ausschnitt unterstützt statische PCM-Quellen und die Hauptlistener-Zuordnung. **Quellen mit dem räumlichen Sonderreservierungszweig `(flags & 0x140) == 0x40` werden ausdrücklich abgewiesen.** Dieser Originalzweig benötigt originale Soundnamen, Präfixprüfung und die zusätzliche Reservierungsregel. Allgemeines Listener-/Level-/Ambient-/Filmupdate, Untertitel-/Dialog-Nebeneffekte, Cueverwaltung, Mischung mit Ressourcenhost-Streams und ein gemeinsamer globaler Census sind noch nicht verbunden. Diese Arbeit ergänzt somit den bisherigen Playbackhost und ersetzt nicht die vollständige globale Engineorchestrierung.

Alle Hardware-, Sound- und Kontextwerte bleiben logische Tokens. Ereignismarker für Methodenaufrufe sind keine zusätzlichen Gerätebefehle; ein Executor darf Stop/Play/Pause dadurch nicht doppelt ausführen. Prozessverlust erfordert weiterhin eine separate Neuerzeugung realer Geräteobjekte. Zustandsrollback betrifft den berechneten Plan, keine bereits ausgeführten externen Aktionen oder Math-Policy-Nebeneffekte.

Prioritäten sind auf endliche Beträge bis 1.000.000 begrenzt, Queue und kombinierte Sortierliste auf 4.096 Elemente. Eigene Ressourcen und JSON-Snapshot behalten ihre getrennten 16-MiB-Grenzen. Strukturierte Zustandssicherungen vermeiden JSON im normalen Zyklus, sind aber kein Nachweis für einen Echtzeitaudiocallback. Der Spielstand der App, akzeptierte APK und Szenenpaket bleiben unverändert.

## Reproduzieren

```powershell
$env:PYTHONPATH=(Resolve-Path build/sound-selection/python-deps).Path
python scripts/Test-AudioQueueOrderOriginal.py --game-data 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' --verify-fixture fixtures/audio-queue-order-native.json --output build/audio-queue-cycle/order-replay.json
python scripts/Test-AudioCleanupOriginal.py --game-data 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' --verify-fixture fixtures/audio-cleanup-native.json --output build/audio-queue-cycle/cleanup-replay.json
cargo test -p rc-package --lib audio_queue_cycle_tests
cargo run -p rc-inspect --bin rc-audio-cycle-probe --release -- build/audio-buffer/real-gen/00.wav build/audio-queue-cycle/real-gen.json
cargo run -p rc-inspect --bin rc-audio-cycle-probe --release -- build/audio-buffer/real-geo/00.wav build/audio-queue-cycle/real-geo.json
./build.ps1 -Target Check -OutputDirectory build/audio-queue-cycle/check
```

Nächste sinnvolle Arbeit: Sonderreservierung mit belegten Soundnamen, Streamaufnahme in denselben Besitzer, gemeinsame Stream-/Staticzählung, vollständige Queuebefehlsanbindung und zuletzt Geräte-/Appadapter samt Hörabnahme.
