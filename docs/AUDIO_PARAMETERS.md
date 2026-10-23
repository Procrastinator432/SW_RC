# Originale Audioparameter für Waffen-Foley

Stand: 23.10.2026 · Zielversion 0.58.0-audio-params

Rüstungsgeräusche und der DC-15s-Ruheton verwenden die originalen PC-Audioparameter. Die Werte werden beim Animationsereignis einmalig gewählt, mit der Audioqueue gespeichert und an Android weitergereicht. Der bereits geprüfte Stand 0.57.0 wurde vor Beginn dieses Blocks als Commit `55afa77` nach `origin/main` gepusht.

## Fünf Punkte

1. **Originalrechnung:** Den ausgewählten Abschnitt `UAudioSubsystem::PlayUSound` (`103d5e56` bis `103d5f2c`) rekonstruiert: erst Lautstärke aus Min/Max wählen, auf 1 begrenzen, bei höchstens 0,0001 abbrechen; andernfalls Pitch wählen und auf 0,5–2 begrenzen. Auch feste Intervalle verbrauchen Zufallszahlen. Der gemeinsame gespeicherte lokale Strom berücksichtigt zuerst die Gruppenwahl, dann Lautstärke und Pitch.
2. **Originalimport:** `USound::GetParams` für das tatsächlich ausgewählte Blatt auflösen. Eine leere Params-Referenz verwendet die Originaldefaults von `Engine.AudioParams`. Min/MaxVolume, Min/MaxPitch, Radius, Kategorie und Positionierungs-/Falloff-/Occlusionflags stammen aus dem aufgelösten Originalobjekt; Nachweise stehen in `weapon-notifies.json`. Der PC-Pfad verwendet keine XBox-Overrides.
3. **Native Weitergabe:** Neues Audioformat `RCAUD003` über `rc_game_audio(kind=4)`. Alte Bank- und Ereignisformate bleiben unterstützt. Größenabfrage und kurze Puffer verbrauchen nichts; eine vollständige Kopie entfernt Ereignisse und zugehörige Parameter gemeinsam.
4. **Android-Wiedergabe:** SoundPool erhält den ursprünglichen Lautstärkefaktor und Pitch. Der vorhandene Host-Gain 0,35 bleibt als übergeordneter Faktor bestehen. Ältere Szenen erhalten weiterhin Gainfaktor 1 und Pitch 1. Ungültige Parameter werden vor dem Abspielen abgewiesen.
5. **Fortsetzen und Nachweis:** Gewählte Werte gehören zur Ereignissequenz; Laden, Rendern und Abfragen würfeln sie nicht erneut aus. Queuebegrenzung und Konsum entfernen zugehörige Datensätze, der Zufallszustand bleibt erhalten. Beschädigte Parameter, fehlende Datensätze und falsche Bindungen an die Originalprofile werden atomar abgewiesen. Originalfälle, vier Karten, alte Speicherstände und die normale APK werden getrennt geprüft.

## Importierte Werte

| Geräusch | Originalobjekt | Lautstärke | Pitch | Radius |
|---|---|---:|---:|---:|
| Alle acht Rüstungsvarianten | Params_SFX.Sound.cc_armor_small | 0,25 | 0,95–1,05 | 100 |
| DC-15s-Ruheton | Params_SFX.Sound.Foley_HudArms | 1,0 | 1,0 | 15 |

Für die Rüstung ergibt sich im bestehenden Androidhost Gain 0,0875 je Kanal; für den Ruheton 0,35. Das ist die originale relative Skalierung innerhalb des vorhandenen Mixers. Die vollständigen ursprünglichen Master-/Duck-/Kategorienregeln sind damit nicht rekonstruiert.

Das Original erzwingt für dem lokalen Spieler gehörende Quellen eine lokale Wiedergabe im PlayUSound-Pfad. Diese ausgewählten HUD-Geräusche erhalten daher keine erfundene Distanzdämpfung. Der Originalradius bleibt als Profilwert und Ereignismetadatum nachvollziehbar. Allgemeine Weltquellen, räumliches Panning, Distanzdämpfung, Occlusion und Doppler bleiben offen.

## Protokoll und Kompatibilität

`RCAUD003`: acht Byte Kennung, u32 Anzahl, anschließend je 48 Byte: u64 Sequenz, u64 Tick, u32 Soundrolle, i32 Owner, u32 Kategorie, u32 bisherige Eigentümerflags, f32 Lautstärke, f32 Pitch, f32 Radius, u32 Parameterflag. Parameterflag 1 bezeichnet die gespeicherten Originalwerte; 0 bezeichnet die unveränderten Legacywerte 1/1/0. Ereignisse behalten ihre bisherigen Soundrollen.

Neue optionale Felder werden bei alten Szenen und Spielständen nicht serialisiert. Der alte Gruppenstrom aus 0.57 behält seinen bisherigen Zufallsverbrauch; zusätzliche Parameterziehungen beginnen ausschließlich mit den neuen passenden Szenen. Alte Szene-/Spielstandpaare bleiben zusammengehörig; es findet keine stille Migration in neue Szenen statt.

## Reproduzieren

Die Bibliothekstests lesen `fixtures/audio-params-native.json`: 800 Fälle mit dem tatsächlich ausgeführten Originalabschnitt, einschließlich 101 stummer Fälle. Sie vergleichen Annahme, Lautstärke-/Pitchbits, Folgeseed und Zufallsverbrauch. Die bestehenden 960 Originalfälle für die Gruppenwahl bleiben Bestandteil der Tests. Die neuen Ergebnisdaten enthalten keine Originalbinärbytes.

```powershell
python scripts/Test-AudioParamsOriginal.py --game-data 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' --output fixtures/audio-params-native.json
```

Für die Neugewinnung werden `unicorn==2.1.4` und `pefile==2024.8.26` benötigt. Das Skript verlangt die bekannten SHA-256 der Original-DLLs und führt den ausgewählten Rechenabschnitt samt ursprünglichem appSRand aus. Engine-/Source-Allokation, Mixer und das vollständige Originalspiel werden dabei nicht ausgeführt.

```powershell
./build.ps1 -Target All -SceneInputs local-assets/mission1/scene-inputs.json -GameData 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' -OutputDirectory build/audio-params/complete
python scripts/Test-WeaponAudioParameters.py --dll target/release/rc_native.dll --scenes build/audio-params/complete/scenes --output build/audio-params/native-parameters.json
python scripts/Test-CampaignAndroid.py --apk build/audio-params/complete/artifacts/republic-commando-0.58.0-audio-params.apk --package build/audio-params/complete/artifacts/mission1-scenes.rcpack.zip --checkpoints build/audio-params/complete/scenes/runtime-validation --output build/audio-params/campaign-android --expect-semi-auto --expect-weapon-animation --expect-weapon-foley --expect-audio-params
```

Der ABI-Prüfer spielt zusätzlich die ersten 1.930 normalen GEO_01A-Eingaben ab und prüft dann den gespeicherten Parameterzustand. Damit bleibt der gefundene Dispatch-Fehler dauerhaft abgedeckt: Der aktualisierte Visualzustand muss vor dem Audio-Dispatch wieder an der Spielinstanz hängen. Die kontrollierten Animationsprüfer führen denselben Dispatch aus.

Die Foley-/ABI-Proben verwenden ausdrücklich kontrollierte Originalanimationen. Der normale Missionslauf bleibt eine zusätzliche unabhängige Prüfung. Android-SoundPool-Aufrufe ersetzen keine subjektive Hörabnahme oder einen freien physischen Handy-Gesamtlauf. Die Zufallsarithmetik entspricht dem Original; der lokale Strom bildet weiterhin nicht die gesamte globale Aufrufgeschichte des Originalspiels ab.

## Prüfstand und Lieferung

Der zentrale Gesamtbau besteht mit je **1.185 Bibliothekstests in Debug und Release**, Clippy aller Targets, Java-/Parameterprüfungen, Android ARM64/x86_64 und 30 Szenenstufen. Der normale Missionslauf A/B/C→Fabrik, 25 Checkpointproben, 39 Spieleransichten, Karma- und Waffenanimationsprüfer bestehen. Alle vier Foley-Prüfer bestätigen originale gespeicherte Wiedergabewerte und je fünf atomar abgewiesene beschädigte Parameterzustände.

Der eigenständige native ABI-Lauf besteht für alle vier Szenen: wiederholte Größenabfrage und zu kleiner Puffer unverändert, bitgenaue Parameterkopie, einmaliger Konsum aller drei Ereignisformate und erhaltener Zufallszustand. Alle Original-PCM-Samples bleiben bitgenau. 24 historische Spielstände aus 0.52–0.57 und sechs alte Einstiegsbilder bleiben exakt. Der tatsächliche Java-GPU-Parser liest alle 39 Spielerframes und die native Welt unter 96 MiB Heap. Die 800 neu gewonnenen Originalfälle stimmen bytegenau mit der versionierten Fixture überein.

Die normale APK besteht auf dem x86_64-Emulator Paketimport, kleine Touch-Steuerung, Halbautomatik, echten Hintergrund-Prozessverlust, kaltes Fortsetzen und alle drei Kartenwechsel. Tatsächliche SoundPool-Aufrufe erhalten für Rüstungsgeräusche in den Touch-Bewegungsticks Faktor 0,25 und Pitch 0,95–1,05, für den Ruheton am originalen Animations-Eintrittstick jeweils 1. Das ist keine subjektive Hörabnahme und kein frei gespielter vollständiger Touch-Durchlauf; die Übergänge verwenden echte Replay-Endspielstände.

Zusammengehörige Prüflieferung **0.58.0-audio-params**:

- [APK](../build/releases/0.58.0/republic-commando-0.58.0-audio-params.apk)
- [Szenenpaket](../build/releases/0.58.0/mission1-scenes.rcpack.zip)
- [Prüfbericht](../build/releases/0.58.0/verification-summary.json) und [Quellzuordnung](../build/releases/0.58.0/source-and-delivery.json)

APK und Szenenpaket gemeinsam aktualisieren, Paket importieren und eine neue Mission starten. Ältere Szene-/Spielstandpaare behalten ihren bisherigen Umfang. Der geprüfte Vorgänger 0.57.0 ist als `55afa77` gepusht; die hier anschließend erarbeiteten 0.58.0-Änderungen sind als Arbeitsstand mit Patch, zusätzlichen Quelldateien und Hashmanifest gesichert.

Nachweise unter `build/audio-params`: `complete`, `native-parameters.json`, `native-audio.json`, `native-world.json`, `original-replay.json`, `legacy-052/053/054/055/056/057.json`, `gpu-java.log`, `campaign-android/validation.json` und `verification-summary.json`.
