# Audio: Pause, Stop und Warteschlange

**Quellfortsetzung 24.10.2026:** [Transient-Kanäle, Aktivierungsvorbereitung und Stimmenzählung](AUDIO_ACTIVATION.md), fünf weitere intern umgesetzte Pakete und acht neue Testgruppen. Die AL-Upload-/Playgrenze, globale Update-Sortierung und der allgemeine App-/Treiberanschluss bleiben ausdrücklich offen.

Stand: 24.10.2026. Fünf weitere interne Pakete nach der [Quellenverwaltung](AUDIO_SOURCE_RUNTIME.md) sind in `rc-package::audio_commands` umgesetzt. Sie bearbeiten eigene Quellenzustände und liefern geordnete Aktionsmeldungen. Ein Android-/Musiktreiber führt diese Aktionen noch nicht aus. Die spielbare Referenz bleibt 0.60.0; keine neue APK oder Missions-/Hörabnahme.

## Fünf umgesetzte Punkte

| Punkt | Vertrag | Originalnachweis |
|---|---|---|
| 1 · Einzelpause und Pauseabfrage | `pause_sound` für passende aktive Quellen mit gepackter Identität ungleich 0. `is_sound_paused` sucht aktive/gegebenenfalls wartende Quellen und gibt die separate virtuelle Treiberantwort als i32 zurück. | Ganze `PauseSound` `103d2e90` und `IsSoundPaused` `103d69f0`; zusammen 128 unabhängige Fälle. |
| 2 · Globale Pause | `pause_all`: Musik-Pauseanforderung zuerst, danach aktive und wartende Quellen in Reihenfolge. Beim Pausieren wirkt die Ausschlussmaske; beim Fortsetzen mit Anfrage 0 wird sie ignoriert. | Ganze `PauseAllSounds` `103d30a0`; 64 Fälle. |
| 3 · Vollständiges StopSound | Wildcards/Actor-/Soundmatching, −1 als Anfrage nach Standard-Fade, gestreamte Kurvenwahl, aktiver Fade oder sofortiger Stop, sofortiger Stop passender wartender Quellen und originaler Rückgabewert. | Ganze `StopSound` `103d6b10`; 64 unabhängige Fälle plus Befehlsfolgen. Der zuvor isoliert geprüfte Dispatch ist damit um die Suchschleifen ergänzt. |
| 4 · Globale Stop-/Stimmenfilter | `stop_all` mit Flagmaske, Musikbedingung und Erhaltung einer aktuellen Cue-ID; `stop_voices` mit exaktem Actor- und Kategorie-1-Filter. Die verschiedenen Zeitpunkte der Rücksetzung des Quellenflags `+3c` bleiben erhalten. | Ganze `StopAllSounds` `103d62e0` und `StopVoices` `103d13e0`; zusammen 128 Fälle. |
| 5 · Queuefreigabe und fortgesetzte Kennungen | `empty_waiting` liefert Löschmeldungen in Listenreihenfolge. `enqueue` besitzt den Konstruktorzähler samt Überlauf und initiiert eine eigene neue wartende Quelle. Snapshots speichern aktive/wartende Quellen, Zähler und Cue-ID zusammen. | Ganze `EmptyQueuedSources` `103d66f0` und `GetNewQueuedSource` `103d8c70` einschließlich originaler Arraygrößenänderung `10368120`, Konstruktor und Init; zusammen 128 unabhängige Fälle. Speicherbereitstellung/Destruktor sind kontrollierte Grenzen. |

Zusätzlich laufen **64 gemischte Befehlsfolgen mit 576 Schritten**: globale/gezielte Pause, neue Queuequelle, Fade/Stop, Fortsetzen, Stopmaske, Leeren, erneute Anlage und Pauseabfrage. Nach jedem Schritt wird über die produktive Kernsnapshot-Schnittstelle gespeichert und geladen. Die Originalfälle belegen auch den Übergang `0xffffffff → 0` des Zählers und seine Erhaltung beim Leeren.

Die synthetische Fixture enthält **512 unabhängige Befehle plus 576 Folgeschritte**, zusammen 1.088 ganze originale Befehlsaufrufe. Acht neue Rust-Testgruppen prüfen sämtliche Quellfelder, Rückgaben, Aktionsreihenfolge, kalten Fortlauf und Fehlergrenzen. Jeweils **1.232 Bibliothekstests in Debug und Release** bestehen. Der vollständige Zentralcheck besteht einschließlich Format, Materialimport-/Dokumentationstests, Clippy, Native-/Inspector-Build, Java und Buildvertrag. Alle 986 erfassten Quell-/Skript-/Konfigurations-/Fixturedateien blieben seit Checkbeginn unverändert. Die bestehenden Audiofixtures bleiben zusätzlich als Regressionen erhalten.

## Korrektur der virtuellen Pause-/Queuezuordnung

Der direkte Export-/Tabellenabgleich an `FAbstractSource`-VTable `10666b68` zeigt:

| Virtueller Offset | Originalfunktion | Bedeutung |
|---|---|---|
| `+1c` | `10358f40`, `IsPaused` | Pauseabfrage; UpdateBase verwendet sie an zwei Stellen. |
| `+20` | `10358f50`, `IsQueued` | Queueabfrage; UpdateVolume verwendet sie vor dem automatischen Stop. |

Die früheren internen Schnittstellen hatten diese beiden **Bezeichnungen vertauscht**, obwohl die Prüfprogramme die originalen Offsets aufriefen und die Rechenergebnisse stimmten. Jetzt heißt der UpdateBase-Eingang `BaseInput::paused: [bool;2]`, der Volume-/Frame-Eingang `queued`. Eine Queueantwort verhindert den ausgewählten automatischen Lautstärkestop; eine Pauseantwort verhindert bereits den UpdateBase-Hauptpfad. Die gespeicherte Pauseanforderung `Source::requested_pause` bleibt ein dritter, getrennter Wert.

Die Korrektur verändert keine ursprünglichen Ergebnisbits und keine App-Spielstände. Alle drei betroffenen Originalvergleiche wurden frisch gegen die umbenannten Fixtures geprüft: 3.000 Welt-Audiofälle, die 7.048 Controllerfälle/-schritte sowie die Quellenvergleiche einschließlich 768 ganzer Quellenaktualisierungen. Ein neuer Regressionstest prüft die VTable-Ziele und hält Pause, Queue und gespeicherte Anfrage ausdrücklich auseinander.

## Belegte Unterschiede und Aktionsreihenfolge

- `PauseSound` bearbeitet nur aktive Quellen mit gepackter Identität ungleich 0. Auch die Anfrage Actor=0/Sound=0 pausiert keine wartenden Quellen. `PauseAllSounds` erfasst dagegen beide Listen, unabhängig von ihrer Belegungsmarkierung.
- Die Einzelpauseabfrage übernimmt die i32-Treiberantwort unverändert, auch Werte wie −3 oder 7. Sie leitet sie nicht aus dem zuletzt gespeicherten Pausewunsch ab. Der aktive Suchpfad kann einen Actor automatisch auflösen; der Queue-Fallback verwendet wieder die ursprünglichen Suchargumente.
- `StopSound(Actor=0,Sound=0,…)` ist ein Sonderpfad: Musik-Stopanforderung, sofortiger Stop aller aktiven Einträge, Rückgabe 1. Die wartende Liste bleibt in diesem Pfad unberührt, und eine angegebene Fadezeit wird ignoriert.
- Bei anderen StopSound-Anfragen bedeutet exakt −1: Standard-Fade des **angefragten** Sounds auslesen. Sound=0 hat keinen solchen Getter und stoppt dann sofort. Andere nichtpositive Zeiten stoppen sofort. Gestreamte Kurven werden aus dem angefragten Sound bestimmt, auch wenn die passende Quelle dessen zweite Soundbindung verwendet. Die Queue wird unabhängig von der Fadezeit sofort gestoppt, ohne ihre Einträge zu entfernen.
- Der aktive sofortige Einzelstop setzt `+3c` vor dem Stop zurück. Der Null-/Null-Sonderpfad und globale StopAll setzen es bei aktiven Einträgen erst danach zurück. Wartende StopSound-/StopAll-Einträge behalten dieses Flag beim Standard-Clear. Die Ereignisse enthalten den am Stopaufruf sichtbaren Wert, sodass diese Reihenfolge geprüft ist.
- StopAll maskiert die vollständigen Quellenflags. Bit `0x10` unterdrückt seine Musik-Stopanforderung. Bit `0x80` schützt bei Cue-ID ungleich 0 genau den ersten gefundenen Eintrag mit dieser Unique-ID, aktive Liste zuerst; dies ist kein Schutz aller Quellen derselben Kategorie oder gepackten Identität.
- StopVoices vergleicht den Actor exakt, ohne Actor=0 als Wildcard zu behandeln, und stoppt nur aktive Quellen mit Soundkategorie 1. Die Queue wird dabei nicht bearbeitet.
- EmptyQueuedSources fordert Löschung an, ohne zuvor für jeden Eintrag Stop aufzurufen. Der native Array-Resize läuft im Prüfer wirklich; nur GMalloc-Speicherbereitstellung und Freigabe werden kontrolliert. Das Rustmodul verwendet eigene Vektoren und emuliert keinen nativen Heap.

## Schnittstellen und Speichervertrag

`Sources` enthält `active`, `waiting`, `counter` und `cue_id`. Die Quellen verwenden weiterhin den geprüften portablen `Source`-Feldvertrag; Sound-/Actorwerte sind undurchsichtige Handles. `Sound` liefert validierte, externe Metadaten für Standard-Fade, Streamed-Klasse, Kurvenart und Kategorie. Der automatische Actorpfad und tatsächliche Pauseantworten bleiben explizite Hosteingaben.

Die geordneten `Event`-Werte sind **Aktionsanforderungen**. Musikhelfer, Treiberzustandsabfragen, tatsächliche Freigaben und das Verhalten überschreibender Treibermethoden sind kontrollierte Grenzen. Die Quellenänderungen verwenden Standard-Pause und Basis-Stop mit Clear. Die Tests führen diese originalen Standardmethoden aus. Es wird keine OpenAL-/Android- oder Musikabnahme behauptet.

`stop_sound`, `stop_voices` und `enqueue` bereiten fehlerfähige Änderungen vor und übernehmen sie erst bei Erfolg. Fehlende Metadaten einer späteren Quelle, fehlerhafte Mathematik oder fehlende Engine lassen weder einen Teilzustand noch eine erhöhte Erzeugungskennung zurück. Die Ereignisse werden ebenfalls erst bei erfolgreicher Rückkehr ausgegeben. Zustandsänderungen einer benutzerdefinierten Mathematikpolitik liegen außerhalb dieser Transaktion; ein späterer externer Treiber darf bereits ausgeführte Aktionen nicht als automatisch rückgängig gemacht betrachten.

Das eigene Queueformat `{schema:1,state:…}` besitzt getrennte aktive/wartende Listen, den bisher externen Konstruktorzähler und Cue-ID. Unbekannte Felder/Versionen und beschädigte Zahlen werden vor der Übernahme abgewiesen; jede Liste ist auf 4.096 Quellen und eine Eingabedatei auf 16 MiB begrenzt. Das Format ist von den vorherigen Runtime-Snapshots und von App-Checkpoints getrennt. Originale Erzeugungskennungen dürfen wie der native u32-Zähler umlaufen; eine zusätzliche Kollisionsvermeidungsregel wird nicht erfunden.

## Reproduktion und Prüfbelege

`fixtures/audio-commands-native.json` enthält ausschließlich synthetische Eingaben und Ergebnisse. `scripts/Test-AudioCommandsOriginal.py` nutzt den vorhandenen Source-/Controller-Prüfer und prüft dieselben originalen Engine-/CRT-Hashes. Private Dekompilate bleiben lokal unter `analysis/`.

Linux:

```bash
source scripts/env-linux.sh
cargo test --locked --offline -p rc-package --lib audio_command_tests
python scripts/Test-AudioCommandsOriginal.py --game-data "$RC_GAME_DATA" \
  --verify-fixture fixtures/audio-commands-native.json \
  --output build/audio-queue/original-replay.json
./build.sh -Target Check -OutputDirectory build/audio-queue/check
```

Windows unter PowerShell 7 mit eingerichteten Werkzeugen:

```powershell
cargo test --locked --offline -p rc-package --lib audio_command_tests
python scripts/Test-AudioCommandsOriginal.py --game-data $env:RC_GAME_DATA `
  --verify-fixture fixtures/audio-commands-native.json `
  --output build/audio-queue/original-replay.json
./build.ps1 -Target Check -OutputDirectory build/audio-queue/check
```

Die Rusttests benötigen nur die versionierten Fixtures. Frischer Originalvergleich benötigt die lokale Originalinstallation und `config/analysis-requirements.txt`. Alle Eingabepfade bleiben Argumente beziehungsweise Umgebungsvariablen.

Lokale Belege unter `build/audio-queue/`: `original-replay.validation.json`, die drei Regressions-Replays mit korrigierten Metadaten, `check/validation.json` und `verification-summary.json`.

## Weiter offen

Die fünf internen Pakete sind umgesetzt. Wartende Quellen werden damit angelegt, beeinflusst und entfernt; die eigentliche Übernahme einer Queuequelle in einen konkurrierenden Spiel-/Treiberpool, globale originale Stimmenzählung, Musik-/Streamingtreiber und allgemeine App-Checkpointanbindung bleiben weitere Arbeit. Nicht abgebildete native Nullpointeraufrufe (etwa ungültige aktive Pause-Suchargumente oder fehlender Sound bei StopVoices) werden portabel als Fehler behandelt.

Die zuvor auf Nutzerwunsch zurückgestellten vollständigen Windows-Missionsläufe und historischen Szene-/Spielstandpaare bleiben offen. [Windows-Prüfliste für die Geometrieanbindung](AUDIO_GEOMETRY_RUNTIME.md#windows-abnahme-bewusst-zurückgestellt). Nach tatsächlicher App-/Treiberintegration sind zusätzlich neue Missions-, Android- und Hörprüfungen erforderlich.
