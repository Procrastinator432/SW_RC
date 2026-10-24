# Sichtbares Seil während der Abfahrt

**28.10.2026: Hände/Rüstung beim Herunterschauen ergänzt (0.77.0).** Eine explizite Armmaske zeigt 1.150 Originaldreiecke mit Delta-38-Material und hält Kopf/Torso aus der Kamera. Je 1.524 Bibliothekstests, zwei A-Blickvarianten, zehn Animationsrestores und vollständige Mission bestehen. Kamera und Gameplay bleiben erhalten. Neue APK und neues Paket gemeinsam verwenden; vollständige Original-Ego-/Receiverbindung und freie Handyabnahme bleiben offen. [Bericht und Lieferung](RAPPEL_ARMS.md).

Stand: 28.10.2026 · **0.76.0-rappel-visual / Code 80**.

Das Seil wird während der Abseilfahrt nun ausgerollt dargestellt. Bisher zeigte der Port bis zur ursprünglichen Zerstörungsaktion ausschließlich die Ruhepose `ActionBreathe`. Die Seiltextur war vorhanden und bereits deckend; die fehlende Darstellung erforderte keine weitere Alphakorrektur.

[APK für ARM64 und x86_64](../build/rappel-visual/android/artifacts/republic-commando-0.76.0-rappel-visual.apk) · [Neues Szenenpaket](../build/rappel-visual/package/mission1-scenes.rcpack.zip) · [Prüfzuordnung](../build/rappel-visual/verification-summary.json)

**APK und Paket gemeinsam aktualisieren**, das Paket importieren und **Neue Mission starten** wählen. Das neue Paket benötigt 0.76. Alte Pakete und ihre Checkpoints bleiben mit 0.76 lesbar und behalten die bisherige Seildarstellung. Eine alte gespeicherte Szene wird durch Fortsetzen nicht aktualisiert.

## Änderung und Grenze zur Originalrekonstruktion

Dies ist ein ausdrücklich gewählter **Portadapter für die Darstellung**. Er koppelt den vorhandenen Original-Seilclip `CloneEntrance600` an den gespeicherten Kanalstand des Spielerclips `EntranceRope600`. Der Loader prüft ihre übereinstimmenden 141 Frames und 30 Hz. Es gibt keinen zweiten Animationstimer und keinen zusätzlichen Sound-/Notify-Dispatch.

Die Paketoption `rappel.synchronize_rope` schaltet die Korrektur gezielt ein. Fehlt sie, gilt das bisherige Verhalten. Vor dem Start bleibt die Ruhepose stehen; während der Fahrt folgt das Seil dem Spielerkanal. Die letzte Pose bleibt gegebenenfalls bis zum unveränderten ursprünglichen Zerstörungsereignis erhalten. Ein restaurierter Kanal bestimmt sofort dieselbe Seilpose.

**Der ursprüngliche Receiver ist weiterhin nicht vollständig rekonstruiert.** Der Kartenwert `Rope7.AnimTriggered=CloneEntrance1760` belegt keinen Aufruf von `CloneEntrance600`. Diese Lieferung behauptet deshalb keine neue Originalaufruf-Parität. Die neue Bindung macht die vorhandenen passenden Assets im ausgewählten Portablauf sichtbar.

Kamera, Spielerbewegung, Kollision, Landung, Skriptzeiten und sämtliche Tonbindungen bleiben erhalten. Es werden keine Meshes, Texturen oder Musik-/WAV-Daten ersetzt. Die Hände und Rüstung des eigenen Körpers sind **weiterhin offen**; der eigene vollständige Körper wird hier nicht eingeblendet.

## Spieleransicht

[Vorher bei Tick 420](../build/rappel-visual/views/a-420-before.png) · [Nachher bei Tick 420](../build/rappel-visual/views/a-420-after.png) · [Android mitten in der Fahrt](../build/rappel-visual/android-runtime/rappel-paused.png)

Im festen A-Replay verändern sich ausschließlich die beobachteten aktiven Abseilansichten. Beim Einstieg sowie nach der ursprünglichen Seilzerstörung sind die Vergleichsbilder gleich. Das sichtbare Seil folgt der vorhandenen Weltposition und Rotation des Originalobjekts.

## Prüfungen

- Zentralcheck: **je 1.521 Debug-/Release-Bibliothekstests**, Materialimport, Doctests, Clippy, alle Inspectorprogramme, Java und Buildvertrag. Zwei neue Gruppen prüfen die gespeicherte Seilphase und das optionale alte Paketformat.
- **19.319 A-Eingaben**, 93 identische Gameplayzustände und **459 identische Audioereignisse** im unmittelbaren Vorher-/Nachherlauf ohne zusätzliche Suspend-Eingriffe. Die Bilder werden während der Fahrt, bei Landung und nach der Seilzerstörung verglichen.
- **Fünf kalte Wiederherstellungen** liefern exakt dieselben Checkpointbytes und Spielerbilder. Auch der jeweils nächste echte Input erzeugt dieselben Zustände, Bilder und Audioereignisse wie im ununterbrochenen Lauf. Ein zusätzlicher tatsächlich suspendierter Zwischenstand hält die Seilphase und das Bild unverändert.
- Vollständige Mission durch Windows-C-ABI/OpenAL: **49.795 Eingaben**, 1.230 WAV-Ereignisse, drei Kartenübergänge und vier kalte Wiederherstellungen. Dieser Lauf enthält normale Suspend-/Fortsetzungsprüfungen; seine Ereigniszahl ist daher von der direkten A-Gegenüberstellung getrennt.
- **36 ältere Checkpoints und neun Einstiegskameraframes** bestehen weiter. Wiederholtes Erstellen des neuen Pakets erzeugt dasselbe ZIP bytegenau.
- Zusätzlich lässt sich der echte Spiel-/Audiostand aus 0.75 mitten in Scorchs Satz mit der neuen DLL unverändert wiederherstellen; Sprache und Sampleposition bleiben erhalten.
- Android-Build und ELF-/JNI-Verträge bestehen für ARM64 und x86_64.

[Zentralcheck](../build/rappel-visual/check/validation.json) · [Bilder, Zustände und Restores](../build/rappel-visual/views/validation.json) · [Mission](../build/rappel-visual/campaign/mission-audio.json) · [Ältere Checkpoints](../build/rappel-visual/legacy/validation.json) · [Android-Build](../build/rappel-visual/android/validation.json) · [ELF-/JNI-Prüfung](../build/rappel-visual/android-libraries.json) · [Android-Laufzeit](../build/rappel-visual/android-runtime/validation.json)

Die Android-Prüfung importiert das produktive Paket und lädt einen echten, bei Tick 330 suspendierten Windows-Replaystand in der Fahrt. Sie prüft GPU-Laden, unveränderten pausierten Checkpoint nach Prozessverlust und anschließenden Animationsfortschritt. Danach folgen B/C-Musik, Pause und Prozess-Fortsetzung. Dies ist keine freie physische Handy-/Hörabnahme.

Auch die vollständigen pausierten Android-Screenshots vor und nach Prozessverlust sind pixelgleich. Nach Fortsetzen schreitet der geprüfte Kanal von Tick 330 auf Tick 371 voran. [Bildvergleich](../build/rappel-visual/android-runtime/rappel-frame-restore.json).

Die bisherigen Materialkorrekturen und alle 42 Sprachzuordnungen des ausgewählten A-Hosts sind im Paket enthalten. Die freie Abnahme gemeldeter Sprachüberlagerungen und die anfangs vermisste Musik auf A bleiben offen.

## Reproduzieren

```powershell
cargo build --release --offline --locked -p rc-inspect --bin rc-rappel-visual-build
python scripts/Build-RappelVisual.py --builder target/release/rc-rappel-visual-build.exe --package build/voice-context/package/mission1-scenes.rcpack.zip --output build/rappel-visual/package
./build.ps1 -Target Check -OutputDirectory build/rappel-visual/check
./build.ps1 -Target Android -OutputDirectory build/rappel-visual/android -Abis arm64-v8a,x86_64
```

Der Builder arbeitet mit einem expliziten selbständigen Eingabepaket und benötigt keine historischen Analyseordner. Frühere Lieferungen bleiben erhalten. Kein Commit, Tag oder Push wurde erzeugt.
