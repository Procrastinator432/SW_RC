# Sprachzuordnung nach dem Handytest

**28.10.2026: A-Sprachkontexte vervollständigt.** Alle 42 Sprachclips des ausgewählten A-Hosts sind zugeordnet. Elf zusätzliche Bindungen erfassen Scorchs gemeinsame Identität, ausgewählte Granaten-/Rendezvousaktionen sowie die zweite Trooper- und Warrior-Stimme. 19.319 A-Eingaben ergeben 81 unveränderte Spielzustände; physischer Scorch-Restore und vollständige Mission bestehen. Neues Paket für die unveränderte APK 0.75. Ego-/Seildarstellung und freie Hörabnahme bleiben offen. [Bericht und Paket](SELECTED_VOICE_CONTEXTS.md).

Stand: 28.10.2026 · **0.75.0-dialog-routing / Code 79**.

Ein konkreter Fehler hinter möglichen Sprachüberlagerungen ist behoben: Spätere GEO_01A-Missionsadapter übergaben Sprachzeilen bisher teilweise als Effekt (Kategorie 2) mit unbekanntem Besitzer. Dadurch wurden weder die Sprachmischgruppe und ihr Ducking noch die bestehende Ersetzungsregel für denselben Sprachbesitzer angewendet.

[APK für ARM64 und x86_64](../build/rappel-dialog/android/artifacts/republic-commando-0.75.0-dialog-routing.apk) · [Szenenpaket mit Materialkorrekturen](../build/rappel-dialog/package/mission1-scenes.rcpack.zip) · [Prüfzuordnung](../build/rappel-dialog/verification-summary.json)

APK und Paket gemeinsam verwenden, das Paket importieren und **Neue Mission starten** wählen. Alte Checkpoints behalten ihre ursprünglichen Szenen und damit deren Tonzuordnungen. Die neue APK migriert solche Szenen nicht.

## Änderung und Originalbezug

Der neue Prüfer `rc-mission-voice-audit` liest die ursprünglichen GEO_01A-Aktionen, Scriptbesitzer, SoundSourceTag, AudioParams.ActorTag, Kategorie und bDontOverrideOtherSounds. Er ergänzt ausschließlich eindeutige Zuordnungen: Alle gefundenen Verwendungen einer zusätzlich übernommenen Soundreferenz müssen denselben belegten Besitzer und dieselbe Schutzregel besitzen. Unaufgelöste Pawn-Besitzer von ScriptedSequences und widersprüchlich wiederverwendete Clips bleiben unverändert.

So kommen **18 Bindungen** zu den bisherigen 13 hinzu. Beispiele sind die beiden Zeilen des Absturzskripts mit Besitzer 739, die Pistolensequenz mit Besitzer 40 und die Türzeilen mit Besitzer 32. Auch die erste Trooperstimme mit dem expliziten Besitzer 453 erhält ihre richtige Kategorie. Bereits festgelegte Besitzer werden nicht durch andere Akteure ersetzt; ausschließlich der fehlende Wert −1 darf aus der Bindung ergänzt werden. Andere negative Kennungen bleiben erhalten.

Die Zuordnung erfolgt bei der Audioausgabe. Gameplayereignisse, Skriptzeiten, Kamera, Animationen, Materialien und Checkpointformat bleiben erhalten. Der physische Mixer verwendet seine bereits geprüfte Regel: Eine ungeschützte laufende Stimme desselben Besitzers wird ersetzt, eine geschützte bleibt erhalten. Verschiedene ursprüngliche Besitzer können weiterhin gleichzeitig sprechen. Es gibt keine neue globale Dialogwarteschlange.

Der Audit zählt 42 Sprachclips in der ausgewählten A-Soundbank. Die 31 vorhandenen Bindungen ergeben **keine vollständige Sprachintegration**: Elf Clips besitzen in diesem Pfad weiterhin keine eindeutige übernommene Zuordnung. Der konkrete schnelle Spielerweg aus dem Handytest wurde nicht reproduziert; sämtliche gemeldeten Überlagerungen sind daher noch nicht als behoben abgenommen.

## Prüfungen

- Zentralcheck: **je 1.519 Debug-/Release-Bibliothekstests**, Materialimport, Doctests, Clippy, Java und Buildvertrag. Die beiden neuen Inspectorprogramme bestehen zusätzlich Format/Clippy nach der letzten Idempotenzkorrektur.
- Direkter C-ABI-Vergleich: **19.319 A-Eingaben**, 81 identische Spielzustände und 18 korrigierte tatsächlich ausgegebene Sprachereignisse. Innere A-Szenenbytes und alle B-/C-/Fabrikszenen bleiben bytegleich. Geändert werden nur A-Tonbindungen und ihre Herkunftsnachweise.
- Vollständige Mission: **49.795 Eingaben**, 1.230 WAV-Ereignisse, drei Kartenübergänge und vier kalte Wiederherstellungen durch Windows-C-ABI und echtes OpenAL.
- Tatsächlicher OpenAL-Sprachbesitztest: alle vier Schutzflagkombinationen, unterschiedliche Besitzer, aktiver/wartender Restore und natürliches Clipende bestehen.
- **36 ältere Checkpoints und neun Einstiegskameraframes** bestehen weiterhin. Eine wiederholte Paketkorrektur erzeugt dasselbe ZIP bytegenau.

[Zentralcheck](../build/rappel-dialog/check/validation.json) · [A-Vergleich](../build/rappel-dialog/routing.json) · [Mission](../build/rappel-dialog/campaign/mission-audio.json) · [Sprachbesitz](../build/rappel-dialog/voice-owner.json) · [Ältere Stände](../build/rappel-dialog/legacy/validation.json) · [Android-Build](../build/rappel-dialog/android/validation.json) · [Android-Laufzeit](../build/rappel-dialog/android-runtime/validation.json)

Die Android-Laufzeitprüfung verwendet den genehmigten x86_64-Emulator: produktiver Paketimport, B/C mit echten Replaycheckpoints, Audio, Pause und Prozess-Fortsetzung. Sie ist kein Hörtest und keine freie physische Handyabnahme der A-Dialoge.

## Abseilen: aktueller Befund

Die Seiltextur ist vorhanden und bereits deckend. `Rope7` erbt `AnimHealthy=ActionBreathe`; sein Kartenwert `AnimTriggered=CloneEntrance1760` ist jetzt direkt aus dem ursprünglichen Struct dekodiert. Der ausgewählte Spielerclip heißt dagegen `EntranceRope600`. Eine Übereinstimmung der Namen mit dem ebenfalls verfügbaren Seilclip `CloneEntrance600` beweist keinen ursprünglichen Aufruf.

Sechs isolierte Modellansichten zeigen den Unterschied zwischen Ruhepose und ausgerolltem Seil. Originalaktionen, drei auf Rappel wartende Skripte, sieben Spielernotifies und die bisherigen Kamera-/PlayAnim-Untersuchungen belegen den fehlenden Verbindungsaufruf noch nicht. Eine versuchsweise Darstellung des vollständigen eigenen Körpers verdeckte die Kamera und wurde bereits im vorherigen Korrekturblock verworfen. **Hände/Rüstung und die aktive Seildarstellung sind weiterhin offen.** Dieser Build ändert sie nicht.

[Dekodierte Originalwerte und Aktionen](../build/rappel-dialog/research/original-receivers.json) · [Isolierte Modellansichten](../build/rappel-dialog/research/poses.json) · [Sprachquellen-Audit](../build/rappel-dialog/package/voice-owners.json)

Die nächsten Schritte sind der ursprüngliche Ego-/Seil-Animationsreceiver, die elf verbleibenden A-Sprachzuordnungen mit aufgelösten Aktionskontexten sowie ein freier Handy-/Hörtest mit schnellem Vorlaufen. Die anfangs vermisste Musik bleibt getrennt: GEO_01A startet im aktuellen belegten Pfad ohne Musikzuweisung; Menü-/Filmvererbung ist noch offen.

## Reproduzieren

```powershell
cargo build --release --offline --locked -p rc-inspect --bin rc-mission-voice-audit
python scripts/Build-MissionVoiceRouting.py --builder target/release/rc-mission-voice-audit.exe --game-data "D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData" --package build/material-coverage/package/mission1-scenes.rcpack.zip --output build/rappel-dialog/package
./build.ps1 -Target Check -OutputDirectory build/rappel-dialog/check
./build.ps1 -Target Android -OutputDirectory build/rappel-dialog/android -Abis arm64-v8a,x86_64
```

Die Quelldaten und expliziten Paketargumente sind erforderlich; ausgeblendete historische Analyseordner sind keine Buildereingaben. Es wurde kein Commit, Tag oder Push erzeugt.
