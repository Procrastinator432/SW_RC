# Originale SoundMultiple-Auswahl

Der eingefrorene Stand 0.57.0 wurde am 23.10.2026 als Commit `55afa77` nach `origin/main` gepusht. Die anschließenden [originalen Audioparameter / 0.58.0](AUDIO_PARAMETERS.md) sind ein eigener Abschnitt.



Stand: 22.10.2026 · Zielversion 0.57.0-sound-selection



Die beiden Rüstungs-Notifies des DC-15s-Run-Clips wählen jetzt aus allen acht Originalsamples von `GEN_Sound.Foley_Movement.cc_armor_walk`. Der erste Kindton wird nicht mehr dauerhaft wiederholt. Der direkte Ruheton und die bestehenden Granatenereignisse behalten ihre bisherigen Hosts.



## Fünf umgesetzte Punkte



1. **Native Auswahl rekonstruiert:** `USoundMultiple::SelectSound` (engine.dll `103d1f80`) und `appSRand` (core.dll `10110250`) nachgebildet. Zufalls-/Reihenfolgeauswahl, FirstSound, Nullkinder, Gewichte, Cache, inklusive Kampagnengrenzen, Kampfkontext, strikte Gewichtsschwelle und originales Rückfallverhalten sind berücksichtigt.

2. **Alle Rüstungsvarianten importiert:** Die native Kindliste und SelectionStyle werden vollständig gelesen. Likelihood/BattleContext/MinCampaign/MaxCampaign stammen aus aufgelösten Originaldefaults und Instanzwerten; Quellnachweise stehen in `weapon-notifies.json`. Die aktuelle Gruppe hat acht gleich gewichtete, uneingeschränkte Kinder und keinen FirstSound. Unbekannte Layouts, verschachtelte Gruppen und hier nicht unterstützte Einschränkungen werden beim Import abgewiesen.

3. **AnimNotify mit Auswahl verbunden:** Beide Run-Notifies teilen sich den Zustand derselben Gruppe. Die Soundbank hängt acht originale Rüstungs-PCM-Samples und den Ruheton an; bestehende Rollen bleiben unverändert. Rendering verbraucht keinen Zufall.

4. **Fortsetzen bewahrt die Geräuschfolge:** Seed, native Cacheflags/Gesamtgewicht, Reihenfolgezähler und letzte Auswahl werden gespeichert. Neue Visualzustände initialisieren die optionalen Gruppen konsistent; Gegnerfabrik-Neuanlagen bewahren den vorhandenen Waffenzustand. Beschädigte oder fehlende Gruppenzustände werden vor einer Zustandsübernahme abgewiesen. Alte Szenen ohne Gruppen behalten ihre bisherige Serialisierung.

5. **Originalvergleich und Integrationsprüfung:** 960 vorbereitete Fälle führen den tatsächlichen x86-Auswahlcode und dessen originalen RNG-Aufruf aus. Rust vergleicht Auswahl, Zufallsverbrauch, Folgeseed, Flags, Zähler, letzte Auswahl und Gesamtgewicht bitgenau. Der bestehende Vier-Karten-Foley-Prüfer prüft zusätzlich alle acht tatsächlich ausgewählten Varianten, unveränderte Fortsetzungsfolgen und fünf atomar abgewiesene beschädigte Gruppenzustände.



## Reproduzierbarkeit



`cargo test --offline --locked -p rc-package --lib sound_multiple` prüft die versionierte Ergebnisfixture `fixtures/sound-multiple-native.json`. Die Fixture enthält synthetische Eingaben und Ergebnisse, keine Originalbinärdateien. Zur Neugewinnung:



```powershell

python scripts/Test-SoundMultipleOriginal.py --game-data 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' --output fixtures/sound-multiple-native.json

```



Dafür werden lokal `unicorn==2.1.4` und `pefile==2024.8.26` benötigt. Die Referenz-SHA-256 stehen in der Fixture: engine.dll `f8b3fc9d62414828a1c31d8b5f3b2bcb951f7c0f142fdf2a9846c98d7b5d35a5`, core.dll `4f69fc7aae44560b692538565e41885e79b4ee4afe93bc8400fd13ba207c8709`. Beim Replay wird ausschließlich die Diagnose der leeren Gruppe übersprungen; Auswahl und Zufallszahlen stammen aus Originalinstruktionen. Das ist keine Ausführung des vollständigen Spiels.



Der Zentralbau `build.ps1 -Target All` umfasst Bibliotheken, Clippy, Java, APK, Szenen, Mission, Checkpoints und alle vier Foley-Prüfläufe. `scripts/Test-CampaignAndroid.py --expect-semi-auto --expect-weapon-animation --expect-weapon-foley` prüft die normale APK einschließlich Originalbank, Touch-Rüstungsgeräuschen, Ruheton und kaltem Fortsetzen des Auswahlzustands.



Die abschließende APK-Prüfung dieser Lieferung wird mit expliziten, zusammengehörigen Artefakten aufgerufen (ADB-Gerät `emulator-5554`):



```powershell

python scripts/Test-CampaignAndroid.py --apk build/sound-selection/complete/artifacts/republic-commando-0.57.0-sound-selection.apk --package build/sound-selection/complete/artifacts/mission1-scenes.rcpack.zip --checkpoints build/sound-selection/complete/scenes/runtime-validation --output build/sound-selection/campaign-android --expect-semi-auto --expect-weapon-animation --expect-weapon-foley

```



Der Übergangsprüfer wartet auf die tatsächlich gespeicherte Zielkarte. Ein bloßer Zielkartenname im UI kann bereits während der Übergabe erscheinen und genügt deshalb nicht als Abschlussnachweis.



## Grenzen



Der Port verwendet einen expliziten, pro Waffeninstanz gespeicherten Zufallsstrom mit Startseed `0x52434631`. Die Arithmetik entspricht appSRand; die zeitliche Folge aller RNG-Aufrufe des vollständigen Originalspiels wird nicht rekonstruiert. Beim Start einer neuen Karte beginnt diese lokale Instanz neu. Originalgruppen sind gemeinsam genutzte Engine-Objekte; vollständige globale Gruppenlebensdauer, RefreshGroup, RenderSound-Rekursion und BattleWon/LostMusic sind nicht integriert. Die allgemein geprüften Kampagnen-/Kampffilter werden im ausgewählten Rüstungspfad nicht benötigt; der Import verlangt dort uneingeschränkte Originalkinder.



Originale Lautstärke-/Radius-/Pitchregeln, räumliche Audiohosts, materialabhängige Fußschritte, Zoom/Tween/Pawn und die vollständige Waffen-Zustandskette bleiben weitere Schritte. Erfolgreiches Android-SoundPool-Abspielen ersetzt keine subjektive Hörabnahme auf dem Handy.



## Prüfstand und Lieferung



Der vollständige Zentralbau besteht mit je **1.184 Bibliothekstests in Debug und Release**, Clippy aller Targets, Java-/Parserprüfungen, Android ARM64/x86_64 und 30 Szenenstufen. Der normale A/B/C→Fabrik-Missionslauf, 25 Checkpointproben, 39 Spieleransichten sowie die Karma- und Waffenanimationsprüfer bestehen. Alle vier Foley-Prüfläufe wählen sämtliche acht Originalvarianten und bestätigen exaktes Fortsetzen sowie die fünf atomaren Fehlerfälle.

Der native Audio-ABI-Lauf prüft alle Original-PCM-Fingerprints in den vier exportierten Banken (A: 102, B: 42, C: 56, Fabrik: 10 Samples). Zwanzig ältere Spielstände aus 0.52–0.56 und fünf historische Einstiegsframes bleiben exakt. Der tatsächliche Java-GPU-Parser liest die 39 Spielerframes und die native Welt unter 96 MiB Heap. Die 960 Originalinstruktionsfälle wurden erneut gewonnen und stimmen bytegenau mit der versionierten Fixture überein.

Die normale APK besteht auf dem x86_64-Emulator Paketimport, kleine Touch-Steuerung, Halbautomatik, Hintergrund-Prozessverlust, kaltes Fortsetzen und alle drei Kartenwechsel. SoundPool spielt die Rüstungsgeräusche im Zeitfenster tatsächlicher Touchbewegung und den Ruheton am originalen Animations-Eintrittstick. Die acht Varianten sind in der geladenen Bank vorhanden; der gespeicherte Auswahlzustand bleibt beim kalten Fortsetzen exakt. Dies ist kein vollständiger freier Handy-Durchlauf und keine subjektive Hörabnahme.

Zusammengehörige Prüflieferung **0.57.0-sound-selection**:

- [APK](../build/releases/0.57.0/republic-commando-0.57.0-sound-selection.apk)
- [Szenenpaket](../build/releases/0.57.0/mission1-scenes.rcpack.zip)
- [Prüfbericht](../build/releases/0.57.0/verification-summary.json) und [Quellzuordnung](../build/releases/0.57.0/source-and-delivery.json)

APK und Szenenpaket gemeinsam aktualisieren, Paket importieren und eine neue Mission beginnen. Alte Szene-/Spielstandpaare behalten ihren bisherigen Umfang; alte Spielstände werden nicht stillschweigend an neue Szenen gebunden. Die bisherigen eingefrorenen Lieferungen bleiben erhalten. Die Lieferung wurde zunächst mit Patch, ergänzten Quelldateien und Hashmanifest eingefroren und am 23.10.2026 als Commit `55afa77` nach `origin/main` gepusht.

Nachweise: `build/sound-selection/complete`, `original-replay.json`, `native-audio.json`, `native-world.json`, `legacy-052/053/054/055/056.json`, `gpu-java.log`, `campaign-android/validation.json` und `verification-summary.json`.

