# Audio: Transient-Kanäle, Vorbereitung und Stimmenzählung

**Nachfolgender interner Block:** [AL-Upload und PlaySource-Befehlsfolge](AUDIO_UPLOAD.md) erweitert den hier ausdrücklich begrenzten Vorbereitungsrumpf. Echter Treiber-/Appanschluss bleibt offen.

Stand: 24.10.2026. Fünf weitere interne Pakete nach den [Pause-/Stop- und Queuebefehlen](AUDIO_COMMANDS.md) sind umgesetzt. Sie verwenden die installierte originale `alaudio.dll` zusätzlich zur Engine. Die spielbare Referenz bleibt 0.60.0; es wurde keine neue APK, Mission oder Hörabnahme durchgeführt.

## Fünf umgesetzte Punkte

| Paket | Implementierung | Nachweis |
|---|---|---|
| 1 · Quellenkopie und Transient-Tausch | `audio_activation::copy_source` und `Pool::set_transient`. Logische Quellenfelder einschließlich Unique-ID werden kopiert; physischer Kanal und Hardwarekennung bleiben getrennt. Der Transient-Tausch ersetzt den ersten passenden Slot durch den bisherigen Transient und stoppt den übergebenen Knoten. | 128 ganze Originalkopien (64 Zuweisungen/64 Kopierkonstruktoren), 128 ganze SetTransientSource-Aufrufe samt GetTransientSource. |
| 2 · Verfügbare Kanäle | `Pool::available` verwendet die virtuelle IsFinished-Antwort. Belegung und Priorität ersetzen diesen Test nicht. | 128 ganze originale GetAvailableSource-Aufrufe einschließlich originaler Anzahl-/Indexgetter. |
| 3 · Aktivierungsvorbereitung | `Pool::prepare`: Engine-/Soundvalidität, Precache-Ergebnis, Fehlerabfrage, Queue-/Pauseübergabe, Kopie in den Transient, Bufferzuordnung, FindSource-Auswahl und Rücksetzung bei Ablehnung. | 256 originale PlaySource-Vorbereitungen vom Funktionseingang bis zum Erfolgsausgang vor dem Parameterupload, beziehungsweise bis zur echten Rückkehr im Fehlerpfad. Originales FindSource und der AL-Transient-Tausch laufen dabei zusammen. |
| 4 · Cue-Status und Ducking-Zählung | `audio_census::Status::refresh` sowie `recount`: separater Cue-Refresh mit zwei IsPlaying-Abfragen, danach der späte Update-Zählblock mit Treiber-Update und Lesen der aktualisierten Soundparameter. | 128 ganze UpdateVoiceCueStatus-Aufrufe und 128 Ausführungen des originalen Update-Zählblocks. |
| 5 · Gespeicherte Übergabe mehrerer Queue-Anfragen | `audio_handoff::Handoff`: vom Aufrufer vorgegebene Anfragereihenfolge, physischer Poolgraph, Vorbereitung, Zählblock und anschließende Queue-Löschung als atomare reine Zustandsoperation. | 64 zusammengesetzte Zyklen mit je drei Vorbereitungsanfragen, anschließendem Original-Zählblock und originalem EmptyQueuedSources. Speichern/Laden vor und nach dem Zyklus, Fehlerabweisung ohne Teilübernahme. |

Acht neue Rust-Testgruppen prüfen sämtliche eingefrorenen Zustände und Ablaufmeldungen. Je **1.240 Bibliothekstests in Debug und Release** sowie der vollständige Zentralcheck mit Format, Clippy, Native-/Inspector-Build, Materialimport-/Dokumentationstests, Java und Buildvertrag bestehen. Der frische Originalvergleich bestätigt die Fixture vollständig; alle 992 erfassten Quell-/Skript-/Konfigurations-/Fixturedateien blieben seit Checkbeginn unverändert. Die Fälle enthalten auch Null-Transient, fehlenden Buffer, ungültigen Sound, leeren/vollen Pool, geschützte IDs, nichtpositionale Quellen, wechselnde IsPlaying-Antworten und unveränderte ausstehende Cue-Bits.

## Originalbereiche und kontrollierte Grenzen

| Bereich | Originaladresse |
|---|---|
| Base-Zuweisung / Kopierkonstruktor | Engine `103590a0` / `10358f70` |
| SetTransientSource / GetTransientSource | AL `10005890` / `100014a0` |
| GetAvailableSource / Anzahl / Index | AL `10004380` / `10001720` / `10001730` |
| PlaySource-Vorbereitung | AL `10008800` bis Erfolgspunkt `100088fb`; Fehlerzweige kehren regulär zurück. |
| FindSource | Engine `103d0d10`, hier mit echter AL-Transient-Implementierung gekoppelt. |
| UpdateVoiceCueStatus | Engine `103d0e40` |
| Später Update-Zählblock | Engine `103d99cc` bis ausschließlich `103d9a60` |
| Queue-Löschung im Zusammenschritt | Engine `103d66f0` |

Die PlaySource-Erfolgsausführung wird **vor** Gain-/Pitch-/EAX-Parametern, Bufferupload und dem virtuellen Play-Aufruf angehalten. Es handelt sich um die Vorbereitung einer Aktivierung, keinen vollständigen Port von PlaySource und keinen abgespielten Ton. Die gesamte UAudioSubsystem::Update-Funktion mit Sortierung, Reserven und vorangehenden Zulassungs-/Kurztongates wird ebenfalls nicht als portiert ausgewiesen.

Soundvalidität, Buffer-Precache, alGetError, IsFinished/IsPlaying/IsQueued, Listenerzeit und die Ergebnisse des virtuellen Quellen-Update sind kontrollierte Hostgrenzen. Die Testausführung verwendet einen synthetischen Treiber mit originaler Basis-Stop-/Clear-Wirkung. Reale FALSource-Treiberüberschreibungen und Musik-/Streamingausgabe bleiben offen. Die Klangdaten werden weder geladen noch abgespielt.

Im Zählblock führt der Prüfer das originale Schleifen-/Verzweigungsverhalten aus. Der virtuelle Update-Callback ändert gezielt Cursor, Quellenflag, Soundhandle und Priorität; sein tatsächlich geschriebener Zustand wird aufgezeichnet. Die Soundparameter werden anschließend anhand des aktualisierten Handles abgefragt. Damit ist der Zählkern geprüft, nicht die noch externe Implementierung des virtuellen Treiber-Updates.

## Erhaltene Originaldetails

- Base-Zuweisung und Kopierkonstruktor kopieren die bestehende Unique-ID. Sie erhöhen den globalen Erzeugungszähler nicht. Der physische Hardwarekanal bei `+ac` und die Bufferzelle `+b0` liegen außerhalb der kopierten Base-Felder; der vorbereitende AL-Pfad setzt die Bufferzelle danach ausdrücklich.
- SetTransientSource sucht nach physischer Objektidentität, nicht nach der logischen Sound-/Unique-ID. Nur der erste passende Slot wird ausgetauscht. Auch ein übergebener Knoten außerhalb der Slotliste wird neuer Transient und gestoppt. Einzelne Originalfunktionen können dadurch ungewöhnliche Alias-/Null-Layouts erzeugen; der vollständige Handoff verlangt dagegen eindeutig besessene, nichtnullige Slots und einen separaten Transient.
- Bei erfolgreichem FindSource wird der ausgewählte verdrängte Kanal zweimal gestoppt: zuerst in FindSource, anschließend erneut in SetTransientSource. Diese Reihenfolge bleibt als zwei Stop-Meldungen erhalten.
- Die lokale Zielquelle bleibt der zuvor befüllte Transient, auch wenn die globale Transient-Referenz beim Tausch auf einen anderen Knoten wechselt. Der befüllte Knoten ist danach im aktiven Slotgraphen. Ein abgelehnter Versuch verwendet Clear auf dem befüllten Knoten; dies ist kein weiterer Stop.
- Für nichtpositionale Quellen (Flag `0x100`) wird die kopierte Position auf null gesetzt. Die vorgemerkte Pause stammt aus der eingehenden Quelle nur dann, wenn deren IsQueued-Antwort ungleich 0 ist. Die gespeicherte Anfrage und die virtuelle Queueantwort bleiben getrennt.
- Der eigenständige Cue-Refresh beginnt mit dem vollständigen Pending-Wort `audio+104`. Für anfänglich spielende Quellen fragt er IsPlaying ein zweites Mal ab. Nur wenn auch diese Antwort ungleich 0 ist und das Quellenflag `+3c` gesetzt ist, wird Bit 0 ergänzt. Andere Pending-Bits bleiben erhalten; Cursor und Ducking-Zähler werden in diesem eigenständigen Refresh nicht verwendet.
- Der späte Update-Zählblock fragt IsPlaying dagegen nur vor dem virtuellen Update ab. Für jede so ausgewählte Quelle folgt Update mit der Listenerzeit. Danach setzt `cursor != 0 && Quellenflag` das Cue-Bit. `cursor` ist hier das bestehende portable Feldlabel für Quelle `+34`; damit ist keine originale PCM-Sampleposition belegt. Für den Ducking-Zähler zählt ausschließlich **Bit 0 der danach gelesenen Soundparameter**, unabhängig von der veralteten Kopie des Quellenflags oder einer vermuteten Soundkategorie.
- AnyPlaying beschreibt die anfängliche IsPlaying-Auswahl dieses Blocks. Eine neue IsPlaying-Prüfung nach Update wird nicht erfunden. Der gezählte Wert steht als Eingang für den nächsten Ducking-Zeitschritt bereit; der schon am Frameanfang ausgeführte Duckingcontroller wird damit nicht rückwirkend verändert.

## Physischer Graph und gespeicherter Fortlauf

`Pool` besitzt `nodes`, `slots` und `transient`. Jeder `Node` enthält den bekannten portablen `Source`-Zustand sowie undurchsichtige Hardware-/Bufferhandles. Slot- und Transientverweise sind validierte Knotenindizes; sie sind von der kopierbaren logischen Quellenidentität getrennt. Voraussetzung ist ein gemeinsamer, vom Aufrufer verwalteter Audiokontext. Native Audiokontext-/VTablezeiger werden nicht als portable Speicheradressen übernommen.

`Handoff` ergänzt wartende Quellen, den bisherigen Erzeugungszähler und Cue-/Ducking-Status. Sein eigenes Format `{schema:1,state:…}` speichert den vollständigen Graphen. Falsche Referenzen, nicht eindeutig besessene aktive Slots, ein zugleich aktiver Transient, unbekannte Felder/Versionen und beschädigte Quellen werden vor einer Übernahme abgewiesen. Limits: 4.096 Slots und wartende Quellen, 4.097 physische Knoten, 16 MiB Snapshotdatei.

Die Reihenfolge ist eine vom Aufrufer gelieferte vollständige Permutation der Queue. Dieser Baustein erfindet keinen Ersatz für die noch nicht portierte originale Gesamtsortierung. Alle Anfragen werden vorbereitet; danach werden die expliziten Treiberzustände gezählt und die wartenden Beschreibungen in ihrer ursprünglichen Queue-Reihenfolge gelöscht. Der Erzeugungszähler bleibt dabei unverändert. Die Tests vergleichen auch die tatsächlich aufgezeichneten nativen Destruktoranforderungen.

Der Zyklus arbeitet auf einer vorbereiteten Kopie und gibt Aktionsmeldungen erst bei erfolgreicher Zustandsübernahme zurück. Ein Fehler beim späteren Zählen lässt auch vorherige vorbereitete Aktivierungen, Queue und Status unverändert. Externe Aktionen eines späteren Treiberadapters würden dadurch nicht automatisch rückgängig gemacht. `Ready` meldet die erreichte Vorbereitungsgrenze und bestätigt keinen erfolgreichen Upload/Play.

Dieses Format ersetzt weder den vorherigen Command-Snapshot noch das App-Checkpointschema. Ein allgemeiner gemeinsamer Game-/Treiberadapter sowie die Übernahme zwischen den älteren Kernzuständen sind weitere Integration.

## Reproduktion und Belege

`fixtures/audio-activation-native.json` enthält synthetische Eingaben und Ergebnisse, keine Originalinstruktionen, PCM- oder Kartendaten. `scripts/Test-AudioActivationOriginal.py` nutzt die vorhandenen Prüfer und lädt beide originalen Bibliotheken. Unterstützter AL-Hash:

`alaudio.dll`: `60e0c8495a94b297c36bc96f29e4c64a208f020e1ac55f5d6848d5697cff0dcd`.

Engine-/CRT-Hashes entsprechen den [allgemeinen Controllern](AUDIO_CONTROLLERS.md). Vor Ausführung werden sie geprüft. Private Dekompilate und Instruktionslisten bleiben unter `analysis/` beziehungsweise `build/`.

Linux:

```bash
source scripts/env-linux.sh
cargo test --locked --offline -p rc-package --lib audio_activation_tests
python scripts/Test-AudioActivationOriginal.py --game-data "$RC_GAME_DATA" \
  --verify-fixture fixtures/audio-activation-native.json \
  --output build/audio-activation/original-replay.json
./build.sh -Target Check -OutputDirectory build/audio-activation/check
```

Windows unter PowerShell 7 mit eingerichteten Werkzeugen:

```powershell
cargo test --locked --offline -p rc-package --lib audio_activation_tests
python scripts/Test-AudioActivationOriginal.py --game-data $env:RC_GAME_DATA `
  --verify-fixture fixtures/audio-activation-native.json `
  --output build/audio-activation/original-replay.json
./build.ps1 -Target Check -OutputDirectory build/audio-activation/check
```

Die Rusttests benötigen ausschließlich die versionierten Fixtures. Nur der frische Originalvergleich benötigt die lokale Installation und `config/analysis-requirements.txt`.

Lokale Belege unter `build/audio-activation/`: `original-replay.validation.json`, `check/validation.json` und `verification-summary.json`.

## Weiter offen

Die fünf beschriebenen internen Pakete sind umgesetzt. Offen bleiben insbesondere der restliche AL-PlaySource-Pfad mit Upload/Play, die originale globale Update-Sortierung samt Zulassungsbudget, echte Treiber-/Streamingzustände und die allgemeine Game-/App-Checkpointanbindung. Der neue Zählkern ersetzt keinen vollständigen Audiotreiber.

Die vollständigen Windows-Missionsläufe und historischen Szene-/Spielstandpaare des vorherigen Geometrieblocks bleiben auf ausdrücklichen Nutzerwunsch offen. [Windows-Prüfliste](AUDIO_GEOMETRY_RUNTIME.md#windows-abnahme-bewusst-zurückgestellt). Nach tatsächlicher App-/Treiberintegration sind neue Missions-, Android- und Hörprüfungen erforderlich.
