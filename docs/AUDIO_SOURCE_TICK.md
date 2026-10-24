# Laufender FALSource-Tick und Streamfortlauf

**Nachfolgende Umsetzung 25.10.2026:** [PCM-Worker und byteführender Treiberdatenpfad](AUDIO_STREAM_DATA.md) ersetzt in einem neuen Bridge-Host die bisherigen reinen Streamantworten durch gespeicherten Workerzustand und echte Bytekopien. Vorbis-/Datei-/Gerätebereitstellung bleibt extern; die unten dokumentierte Tick-Komponente selbst bleibt unverändert.

Stand: 25.10.2026 · interner Quellbaustein nach der FALSource-Steuerung. Die [Windows-/Android-Abnahme von e6a2438](WINDOWS_VALIDATION_20261025.md) bleibt die zuletzt geprüfte App-Referenz. Dieser neue Kern ist noch nicht an den allgemeinen Spiel-/Androidtreiber angeschlossen.

## Fünf abgeschlossene Punkte

1. **Laufende Parameter:** Der ganze Treiber-Updatekörper `10006270` liefert Kategorie-/Gainabfrage, begrenzten Gain, originale X/Z/Y-Koordinatenumrechnung, Referenzdistanz und EAX-Anforderung in ursprünglicher Reihenfolge. Lokale Quellen und Editorbetrieb überspringen den räumlichen Teil. Der laufende EAX-Faktor verwendet aktuellen Radius geteilt durch die hochgeladene Referenzdistanz; das unterscheidet sich vom anfänglichen PlaySource-Upload. Pitch, Geschwindigkeit und Looping werden in dieser Methode nicht erneut hochgeladen.
2. **Streaming-Nachfüllung:** Drei AL-Statusabfragen und QueryStream bestimmen, ob genau ein Puffer abgearbeitet wird. Ein von 0 verschiedener Processed-Wert, auch -1 oder 5, führt bei Busy=0 zu genau einer Nachfüllung. Unqueue-Antwort, BufferData, Queue und RequestChunks behalten ihre ursprüngliche Reihenfolge. Der Read-Slot wird sofort mit der zurückgegebenen ID überschrieben; bei gleichem Read-/Write-Slot verwendet bereits der anschließende Upload diese neue ID. Beide Indizes laufen modulo der logischen, vom gepackten Countwort getrennten Anzahl weiter.
3. **Wiederanlauf und Uhr:** Der AL-Status `0x1014` startet bei den ausgewählten Streamzweigen die echte Play-Semantik: Pause wird 0 und Cursor wird 1. Die Lebenszeit wird dabei nicht zurückgesetzt. Jeder erfolgreiche Updatepfad addiert die f32-DeltaSeconds zum gespeicherten f64-Zeitwert. Nicht zugewiesene und pausierte Quellen geben 0 zurück, ohne Callback oder Zeitfortschritt. Der Original-Maschinencode wurde geprüft: Die dekompilierte vermeintliche f32-Verkürzung der bisherigen Lebenszeit findet dort nicht statt.
4. **Streamende:** Wenn der Nicht-Nachfüllzweig QueryStream=0 erhält, folgen DestroyStream(wait=0), Löschen des Streamtokens, Free des Stagingtokens, Löschen dieses Tokens und USound.SetHandle(0). Der eigentliche Buffer und die zugewiesene Quelle bleiben bestehen. Danach verwendet die vorhandene IsFinished-Regel den Nichtstream-/Dauerzweig. Die Testfolgen bestätigen auch, dass keine weiteren Streamabfragen für das gelöschte Token entstehen.
5. **Gespeicherter Gesamtfortlauf:** Die neue Session verbindet vorhandene Driver-Session, Streammetadaten und Bufferfortschritt mit einem eigenen Snapshot-Schema 1. Source-/Zeit-/Radiusbits und zurückgelieferte Puffer-IDs bleiben exakt erhalten. Einzelaufrufe und Stapel werden atomar geplant; fehlerhafte Zustände oder Antworten auf fehlende Ressourcen geben keine teilweise geplante Session frei. Kaltes Laden nach jedem der 1.536 Folgeschritte reproduziert die Originalergebnisse.

## Originalbefunde und Grenzen

Verwendet werden die in [AUDIO_DRIVER.md](AUDIO_DRIVER.md) und [AUDIO_UPLOAD.md](AUDIO_UPLOAD.md) festgehaltenen PC-DLLs. Der Prüfer kontrolliert ihre SHA-256 vor der Ausführung: Engine `f8b3fc9d62414828a1c31d8b5f3b2bcb951f7c0f142fdf2a9846c98d7b5d35a5`, CRT `8094af5ee310714caebccaeee7769ffb08048503ba478b879edfef5f1a24fefe`, ALAudio `60e0c8495a94b297c36bc96f29e4c64a208f020e1ac55f5d6848d5697cff0dcd`.

| Funktion | Adresse | Umfang |
|---|---|---|
| FALSource virtuelle Methode +0x28 | `10006270` | Ganzer nativer Treiber-Updatekörper. |
| FALSource virtuelle Methode +0x24 | `10004f70` | Untersucht: prüft Zuweisung und ruft FAbstractSource.UpdateBase. Nicht nochmals als neuer Gesamtquellenhost implementiert. |
| FALSource.Play | `10004f90` | Bei Wiederanlauf tatsächlich ausgeführte Originalmethode; Rust verwendet die vorhandene Driver-Komponente. |
| USound.SetHandle | `103d06c0` | Am Streamende tatsächlich ausgeführte Originalmethode. |
| QueryStream / RequestChunks | `10003050` / `10003020` | Kontrollierte Rückgaben beziehungsweise protokollierter Auftrag. Kein Thread-/Decoderhost. |
| DestroyStream | `100033d0` | Kontrollierte Ressourcenanforderung. Tatsächlicher Streammanager bleibt extern. |

Die Rustschnittstelle produziert geordnete Befehle. AL-Abfragen, zurückgegebene Unqueue-ID, QueryStream-Ergebnis/Busy-Wort, Streammanager und Allocator sind kontrollierte Grenzen des Originalprüfers. PCM-Inhalte werden nicht dekodiert, kopiert oder wirklich abgespielt. Der Stagingwert ist ein opaker Token. Die Bank hält Beschreibungen auch nach einer Zerstörungsanforderung; nur die aktiven Referenzen werden wie im Original gelöscht. Ein tatsächlicher Ressourcenbesitzer muss die ausgegebenen Aufträge noch ausführen und ihre Antworten liefern.

EAX und die Distanznähe verwenden die vorhandenen belegten Rechenregeln. Für die ursprüngliche RSQRTSS-Näherung verlangt die portable Schnittstelle bei Nichtnullabstand den tatsächlichen Seed als Eingabe. Das ist keine Zusicherung CPU-unabhängiger RSQRTSS-Bits. Ungültige beziehungsweise nichtendliche EAX-Verhältnisse bleiben außerhalb dieses begrenzten Vertrags und werden atomar abgewiesen.

Portgrenzen: maximal 4.096 Schritte, je 4.096 Metadateneinträge, die bereits bestehende Driverbankgrenze von 65.536 Backing-Werten, 16 MiB Snapshot/Chunkmetadaten, endliche nichtnegative DeltaSeconds bis 1.000.000 sowie gültige Indexbereiche. Ein Originalaufruf mit ungültigen Pointern oder Modulo-0 wird nicht durch nachgebildete native Abstürze ersetzt. Die Session ist kein App-Checkpoint und wird nicht automatisch in `audio_handoff`, `audio_source_update` oder Spielstände eingebaut.

## Prüfnachweis

**512 ganze Einzelaufrufe und 1.536 Schritte in 64 Originalfolgen**, zusammen 2.048 ganze Treiber-Updates. Acht neue Testgruppen vergleichen alle Befehle und vollständigen Zustände mit der eingefrorenen synthetischen Fixture. Die Originalausführung enthält unter anderem 125 Nachfüllungen, 94 Endfreigaben, 78 Wiederanläufe und 546 EAX-Aufrufe. Rückgaben, Parameterbits, Indizes, Soundhandle und Zeitwörter werden geprüft; der kalte Fortlauf vergleicht jeden einzelnen Schritt. Fehlerfälle prüfen auch, dass ein fehlerhafter zweiter Schritt den gesamten Stapel unverändert lässt.

Der zentrale Check besteht mit je **1.264 Debug-/Release-Bibliothekstests**, Materialimport-/Doc-Tests, Clippy aller Targets, Native-/Inspectorbau, Java und Buildvertrag. Die normale Gameplay-/Androidanbindung wurde in diesem Block nicht geändert. Die zuvor geprüften APK-/Szenenlieferungen bleiben unverändert; keine neue Android- oder Hörabnahme wird für diesen internen Kern behauptet.

```powershell
./build.ps1 -Target Check -OutputDirectory build/audio-source-tick/check
python -X utf8 scripts/Test-AudioSourceTickOriginal.py `
  --game-data 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' `
  --output build/audio-source-tick/original-replay.json `
  --verify-fixture fixtures/audio-source-tick-native.json
```

Der frische Originalvergleich benötigt die gepinnten Python-Analyseabhängigkeiten; die normalen Rusttests benötigen keine Originalinstallation. Die versionierte Fixture enthält nur synthetische Eingaben, Tokens und Ergebnisse. Private Dekompilate und Disassemblierung bleiben unter `build/audio-source-tick`.

[Gesamtbericht](../build/audio-source-tick/verification-summary.json) · [Quellzuordnung](../build/audio-source-tick/source-and-verification.json)

## Nächste Arbeit

Als Nächstes sind echter PCM-/Bufferbesitz und die Stream-Worker-/Chunkbereitstellung sinnvoll. Anschließend können UpdateBase, dieser Treibertick, Pool, Upload und Stimmenzählung in einen gemeinsamen Quellenhost eingebunden werden. Globale Sortierung/Budgets, tatsächliche AL-Fehler und allgemeine Android-/Mixeranbindung bleiben offen; neue hörbare Änderungen benötigen anschließend Missions-, Checkpoint- und Geräteprüfung.
