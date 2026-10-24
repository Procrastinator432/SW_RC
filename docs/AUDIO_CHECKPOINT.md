# Optionale Audiofortsetzung am Missionscheckpoint

**Folgestand 0.71.0:** [Belegte Pausenquittierung](PAUSE_RECEIPT.md) schließt die unten beschriebene Cursorlücke durch empfangene, bewusst verworfene nachlaufende Effekte. Der strikte Spielstandabgleich bleibt erhalten. Die folgenden Zahlen beschreiben 0.70.0.

Stand: 26.10.2026. **0.70.0-audio-checkpoint** speichert einen abgeschlossenen nativen Pausensnapshot optional neben dem vorhandenen Missionscheckpoint. Nach einem kalten Laden können aktive und wartende Dialoge einschließlich Quellenposition, NoOverride und Poolzählern wiederhergestellt werden. Der normale Spielstand bleibt unabhängig davon verwendbar.

## Fünf umgesetzte Punkte

1. **Abgeschlossene Pauseübergabe.** Jede Ausgabesitzung besitzt ein `SceneAudioCapture`. Bei expliziter Pause, manuellem Speichern oder Activity-Hintergrund wird das Ergebnis angefordert, bevor die Ausgabe angehalten wird. Der Produzent versiegelt/übergibt wartende Dialoge, liest Snapshot und nativen Ereigniszeiger auf seinem Besitzer und veröffentlicht das Ergebnis erst nach erfolgreicher Freigabe. Ein bereits angeforderter Export darf trotz späterer RAM-Cachelöschung durch Activity-Ende fertig werden; er füllt den neuen RAM-Cache nicht wieder. Das Executor-Ende ist dafür geordnet statt wartende Aufgaben abzubrechen. Fehler liefern kein Ergebnis. Drei Sekunden ohne fertige Übergabe führen zu `null`, ohne UI oder Gameplayworker warten zu lassen.
2. **Getrennter atomarer Audiospeicher.** `AudioCheckpointStore` schreibt appprivate `.audio`-Ergänzungen mit Version, Längen und SHA-256-Prüfsumme über den vorhandenen atomaren Schreibpfad. Ein nativer Snapshot ist auf 256 KiB begrenzt; höchstens zwölf zuletzt geschriebene Audiodateien werden behalten. Ältere optionale Fragmente dürfen dadurch entfallen, ihre Gameplaycheckpoints bleiben erhalten. Unvollständige temporäre Dateien werden nicht geladen und nach einem erfolgreichen Schreiben unter dem gemeinsamen Atomarschreib-Lock bereinigt. Schreibfehler im Audiospeicher verändern keinen primären, sicheren oder manuellen Gameplaycheckpoint.
3. **Eindeutige Zuordnung.** Dateiname/Inhalt binden die Szene und **exakte Checkpointbytes** über die vorhandene Spielstandidentität sowie die SHA-256-Soundbankkennung. Der native Spieladapter liefert mit Audioabfrage 8 ein schreibgeschütztes `RCCURS01`-Paket; dessen Eigentümerreihenfolge entspricht dem bestehenden Ereignisausgang. Nur wenn der vollständig übergebene Audio-Ereigniszeiger mit dem des gespeicherten Spiels übereinstimmt, wird die Ergänzung geschrieben. Eine neue Szene/Audio-Generation oder andere Bank verhindert die Zuordnung. Der Gameplaycheckpoint wird zuerst geschrieben; Audio folgt asynchron auf einem eigenen IO-Worker.
4. **Optionaler kalter Restore.** Beim Laden eines bestehenden Checkpoints wird dessen Ergänzung mit genau diesen Identitäten geprüft. Fehlende, beschädigte, veraltete oder nicht passende Ergänzungen werden ignoriert. Der native Produzent validiert den eigentlichen Snapshot und kontrolliert dessen tatsächlichen Ereigniszeiger vor der Bereitschaft zusätzlich gegen den geladenen Missionszeiger. Ein verworfener Snapshot führt zum vorhandenen frischen Audioaufbau. Aktuelle Lautstärkeregler und der erste aktuelle Loop-/Listenerzustand gelten weiter. Bestehende Gameplaycheckpointformate und Szenenpakete ändern sich nicht; SoundPool verwendet keine native Ergänzung.
5. **Fehler-/Prozesswechselprüfungen.** Vier Capturegruppen, acht Speichergruppen, zwei weitere Produktionsproduzentengruppen und ein Rust-Cursortest prüfen Kopienbesitz, Abschluss/Timeout, Cachelöschung, fehlgeschlagene Freigabe, frühen Abbruch, Prüfsummen, Bindungen, Grenzen, temporäre Dateien, Schreibfehler und Aufbewahrung. Ein zusätzlicher Integrationsprüfer spielt echte GEO_01A-Eingaben bis zu einer geschützten Dialogzeile, beendet den nativen Prozess, speichert/lädt mit den tatsächlichen Java-Speicherklassen in getrennten JVMs und stellt Spiel/Audio in einem neuen nativen Prozess wieder her.

## Konservative Grenzen

Die Ergänzung entsteht bei **abgeschlossener Pause-/Hintergrundübergabe**, nicht bei jedem laufenden Autosave. Wird der Prozess vor der fertigen Übergabe/Dateischreibung beendet, bleibt der Gameplaycheckpoint ohne neuen Audiostand erhalten. Sind Ereignisse noch nicht vollständig an den Audioadapter übergeben worden oder nur ausgelassene Effektenden im alten Cursor offen, kann der Zeigervergleich bewusst auf Audiofortsetzung verzichten. Es wird kein Ereigniszeiger blind vorgezogen, um ein Fragment passend zu machen. Ein späterer Autosave mit anderen Checkpointbytes übernimmt keinen älteren Audiostand.

Es bleibt eine Fortsetzung an der **Rendergrenze**. Bereits gerendertes, noch ungehörtes Queue-/AudioTrack-PCM sowie Resamplerhistorie werden nicht persistiert. Aktive Sprache setzt an der gespeicherten Quellenposition fort; noch wartende Sprache beginnt einmal. Kurze Waffen-/Foley-Nachklänge entfallen wie bisher. Das Spiel liefert die gewünschten Dauerquellen erneut; ein durch Gameplay-Suspend beendeter Bactavorgang wird nicht durch einen alten Audiostand wieder aktiviert.

Der Audiostand ist keine zweite Gameplayautorität. Fehlendes/defektes Audio löst keinen Spielstand-Reparaturdialog aus. Physische Treiberfehler können weiterhin den bestehenden Audiofehlerpfad auslösen; sie machen die gespeicherten Gameplaybytes nicht ungültig. Die zwölf Dateiplätze garantieren keine dauerhafte Audiohistorie jedes sehr alten manuellen Checkpoints.

## Nachweise

Der Zentralcheck umfasst je **1.479 Debug-/Release-Bibliothekstests**, Materialimport, Doctests, Clippy, Java und Buildvertrag. Die Java-Audioprüfung umfasst jetzt **70 Gruppen**. Beide Android-ABIs werden gebaut und auf Bibliotheken, bestehende JNI-Verträge und neue Speicher-/Captureklassen geprüft.

Der Prozessprüfer bindet reale Szenenpaket-/Replaydaten, native DLL und Testskript per Hash. Er prüft den schreibgeschützten Spielcursor, identische Bank, getrennte native Prozesskennungen, Java-Primärspielstand plus Audioergänzung, identische zurückgelesene Snapshotbytes und exakt wiederhergestellte Quellenparameter/-positionen. Nach Restore wird eine Sekunde echtes OpenAL-PCM weitergerendert: Sprache/NoOverride bleiben aktiv und der Gameplaycheckpoint verändert sich dadurch nicht. Das sind Windows-/JVM-Nachweise; kein simuliertes Android-Prozessende wird als Geräteabnahme ausgegeben.

Mit derselben DLL werden die bestehenden Pool-, Sprachbesitz-, Pegel-, Pausen- und Dialogprüfungen sowie der vollständige A/B/C→Fabrik-Lauf und 36 ältere Spielstände/neun Einstiegskameraframes erneut geprüft.

[Zentralcheck](../build/audio-checkpoint/check/validation.json) · [Getrennte Prozesse / Java-Speicher](../build/audio-checkpoint/process/audio-checkpoint.json) · [Pool](../build/audio-checkpoint/pool.json) · [Sprachbesitz](../build/audio-checkpoint/voice-owner.json) · [Pegel](../build/audio-checkpoint/mix.json) · [Pausenübergabe](../build/audio-checkpoint/pause-ingress.json) · [Dialogrestore](../build/audio-checkpoint/dialog-resume.json) · [Vollständige Mission](../build/audio-checkpoint/campaign/mission-audio.json) · [Ältere Spielstände](../build/audio-checkpoint/legacy/validation.json) · [Android-Build](../build/audio-checkpoint/android/validation.json) · [Gesamtnachweis](../build/audio-checkpoint/verification-summary.json)

## Lieferung und Geräteabnahme

[APK 0.70.0 für beide Architekturen](../build/audio-checkpoint/android/artifacts/republic-commando-0.70.0-audio-checkpoint.apk) · [Quellstand und Artefaktzuordnung](../build/audio-checkpoint/source-and-verification.json)

APK-SHA-256: `97cca239c50a66aac011ce5c30b253863f589257caf9f691eb0f45a5919eef1f`.

Bestehendes Szenenpaket und Missionscheckpoints weiterverwenden; im Startmenü den nativen Mixer wählen. Für die spätere Geräteabnahme mitten in einem Dialog pausieren, die abgeschlossene Speicherung abwarten, den Prozess beenden und denselben Checkpoint fortsetzen. Auch fehlende/beschädigte optionale Ergänzung, manuelles/sicheres/älteres Laden und Kartenwechsel sind zu prüfen. `RCCheckpoint` protokolliert erfolgreiche Audiozuordnung beziehungsweise verworfene Ergänzungen.

**Android-Lebenszyklus, tatsächliches OS-Prozessende, Storage-/Timeoutverhalten und Hören stehen noch aus.** Die neue Funktion ist durch getrennte Hostprozesse und Android-Bau vorbereitet. Originale Poolprioritäten, Ducking und Vorbis-Musik bleiben eigenständige offene Integrationen. Frühere Lieferungen werden erhalten; kein Commit, Tag oder Push wird erzeugt.

```powershell
./build.ps1 -Target Check -OutputDirectory build/audio-checkpoint/check
./build.ps1 -Target Android -OutputDirectory build/audio-checkpoint/android -Abis arm64-v8a,x86_64
python scripts/Test-AudioCheckpoint.py --dll target/release/rc_native.dll --library tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll --package build/linux-sync-20261025/package/artifacts/mission1-scenes.rcpack.zip --inputs build/linux-sync-20261025/complete/scenes/resolved-inputs.json --output build/audio-checkpoint/process --java tools/jdk21/jdk-21.0.12.1+1/bin/java.exe --classes build/audio-checkpoint/check/java/classes
```
