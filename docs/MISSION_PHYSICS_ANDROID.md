# Stationskollision und bewegliche Barrieren – 0.51.0

Stand vom 18.10.2026, versionCode 55. Gesamtbau und Android-Abnahme sind unter `build/mission-collision` protokolliert. Die eingefrorene Lieferung 0.50.0 wird nicht überschrieben.

## Umsetzung

Die Bacta-Station verwendet im neuen Paket ihre ursprüngliche `Location`, `CollisionRadius` und `CollisionHeight`. Die analytische Abfrage zwischen den aufrechten Zylindern ersetzt dort den bisherigen Box-Adapter. Der Spieler erreicht den vom Originalskript berechneten Sollpunkt; Welt-BSP und andere Akteure behalten ihre vorhandenen Abfragen. Ältere Szenen ohne das optionale Binding behalten ihr früheres Verhalten.

Die vier Akteure 909/910/912/913 werden durch das vorhandene Ereignis `KnockOverWall` freigegeben. Ihre Masse, Reibung, Rückprall, Dämpfung und Schwerkraft stammen aus den ursprünglichen KParams. Schwerpunkt, Trägheitsmatrix und die acht Eckpunkte des konvexen Körpers werden aus `Arena_HR.ArenaWalls.Blockade03.KMeshProps1280` gelesen. Grundlage sind die Originalserialisierer `UKMeshProps::Serialize` bei `10386d10` und der Aggregatpfad `10386ad0`; andere Primitive und nicht unterstützte Transformvarianten werden abgewiesen.

Ein portabler Starrkörpersolver berechnet Bewegung und Rotation in acht Unterschritten je 60-Hz-Spielschritt. Er berücksichtigt Weltflächen, gegenseitige Kontakte und die kinematische Spielerhülle einschließlich Ducken. Die gespeicherte Pose steuert sowohl sichtbare Originalmeshes als auch die bewegten Kollisionsboxen. Nach einer Ruhephase schlafen die Körper ein; nahe Flächen werden über ein räumliches Raster gesucht. Das Raster wird gegen die vollständige Suche einschließlich Zellgrenzen und sehr großer Flächen geprüft.

Position, Quaternion, beide Geschwindigkeiten und Ruhezähler gehören zum vollständigen Checkpoint. Restaurierte Bewegung wird mit der ununterbrochenen Fortsetzung und demselben Bildpaket verglichen. Ungültige Quaternionen, fehlende Körper und widersprüchliche Freigabeflags müssen einen Speichervorgang unverändert zurückweisen. Alte Pakete und ihre Restpose-Spielstände bleiben lesbar.

Beim Springen auf die gedrehten Hüllen ergänzt eine geometrische Projektion der Geschwindigkeit die bisherige Kontaktkorrektur genau dann, wenn diese nach acht Iterationen nicht konvergiert. Funktionierende Bewegungsabläufe behalten dadurch ihre bisherige Rechnung; spitze Kontaktwinkel lösen den Konvergenzfehler nicht mehr aus.

Der bestehende Text `floor probe starts penetrating geometry` bleibt unverändert: Platzierungs- und Navigationssuchen behandeln genau diesen Fehler als verworfenen Kandidaten. Ein eigener Regressionstest prüft, dass die Suche Überlappungen überspringt, einen gültigen Boden findet und unvollständige Geometrie weiterhin als Fehler meldet.

## Prüfung und Aufzeichnung

Die neue Stations- und Barrierenkollision verändert den vorher aufgezeichneten Weg. Die beiden Replay-Builder fügen ausschließlich gewöhnliche Bewegungs-, Blick- und gegebenenfalls Sprungeingaben ein. Nach den Barrieren folgen sie den vorhandenen ursprünglichen Routenankern zu einem Stillstandspunkt. Gesundheit, Inventar, Gegner, Kollision und Position werden nicht gesetzt. Nur nach dem tatsächlich erreichten Sieg überzählige Eingabezeilen entfallen. Die Erzeugung prüft die vollständige neue Fortsetzung bis zum Sieg; der gesonderte Präsentationsprüfer führt anschließend den echten Wechsel nach GEO_01B durch.

Eine durch die geänderte Bewegung knapp verfehlte Auswahlreichweite der scharfen Sprengladung wird ebenfalls durch Annäherungs-, Blick- und Benutzeneingaben überbrückt. Der alte Restpose-Gegenlauf liefert ausschließlich den Anschlussweg, keine Abnahme des ausgelieferten Zustands. Die Missionsprüfung speichert ihren letzten Checkpoint am tatsächlichen Ende der Eingabeaufzeichnung; die Android-Übergangsprüfung sucht einen nachweislich abgeschlossenen Zustand statt einer fest eingetragenen Ticknummer.

```powershell
./build.ps1 -Target All -SceneInputs local-assets/mission1/scene-inputs.json `
  -GameData 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' `
  -OutputDirectory build/mission-collision/complete

./scripts/Test-PresentationAndroid.ps1 `
  -Inputs build/mission-collision/complete/scenes/resolved-inputs.json `
  -PresentationReport build/mission-collision/complete/scenes/player-presentation/validation.json `
  -OutputDirectory build/mission-collision/android

python -X utf8 scripts/Test-CampaignAndroid.py `
  --apk build/mission-collision/complete/artifacts/republic-commando-0.51.0-mission-presentation.apk `
  --package build/mission-collision/complete/artifacts/mission1-scenes.rcpack.zip `
  --checkpoints build/mission-collision/complete/scenes/runtime-validation `
  --output build/mission-collision/campaign-android
```

Der zweite Befehl baut ein getrenntes, lokales Abnahme-APK. Dessen zusätzliche Activity ist im normalen Liefer-APK nicht enthalten. Ein zusammenhängender Android-Nativlauf erzeugt die Zwischenstände; ausgewählte Abschnitte laufen danach über die echten JNI-, GLES- und SoundPool-Klassen der Anwendung. Der Prüfer sammelt Bilder, Audio-/Treiberprotokolle, Ergebnisdateien und Eingabe-/Bibliothekshashes. Voraussetzung ist ein bereits gestarteter Emulator oder ein explizit angegebenes Android-Gerät mit passender Bibliothek.

## Grenzen

Dies ist eine vollständige freie Körperbewegung für diese vier ausgewählten Barrieren, keine numerisch identische Rekonstruktion der MathEngine-Karma-Bibliothek. Allgemeine Karma-Akteure, Ragdolls, Geschossimpulse und allgemeines Schieben ruhender Körper sind nicht enthalten. Vollständige Spieleranimation und HandSignal-Waffenwechsel sind ebenfalls eigene Arbeitsbereiche.

Ein Emulatorlauf kann tatsächliche GLES-Aufrufe, SoundPool-Laden und Wiedergabeanforderungen bestätigen. Subjektive Hörqualität, Touch-Komfort und ARM64-Laufverhalten auf einem physischen Handy benötigen einen Gerätetest. Für die neuen Korrekturen das zusammengehörige APK und Szenenpaket verwenden, das Paket importieren und eine neue Mission starten: vorhandene Spielstände behalten bewusst ihre archivierten Szenen.

## Abschlussbelege

Der zentrale Offline-Gesamtbau besteht mit je **1.162 Bibliothekstests** im Debug- und Release-Modus, Clippy, Java-Prüfungen, beiden Android-ABIs, **23 Szenenstufen** und **25 Checkpointproben** durch A/B/C bis zur Fabrik. Die zusätzliche Präsentationsprüfung erreicht A→B mit **39 regulären Spieleransichten**, neun exakten Wiederherstellungs-/Fortlaufproben und drei atomar zurückgewiesenen beschädigten Physikzuständen. Alle vier Barrieren schlafen am Abschnittsende. Der tatsächliche Java-GPU-Parser liest alle 39 Frames und das 43.475.920-Byte-Weltpaket unter 96 MiB Heap.

Im Android-x86_64-Emulator erreicht der vollständige Nativlauf A→B nach **20.417 normalen Eingaben** mit 20 HP und führt 60 weitere Schritte in B aus. Acht echte Zwischenstände werden neu geladen und mit dem ursprünglichen Bildpaket verglichen. Danach laufen Intro, Verwundung, Bacta, Barrieren, deren Fallbewegung, Geschützstellung, **Scorchs spätere Sprengung** und Tür über die produktiven JNI-/GLES-/SoundPool-Klassen. **16 tatsächlich gerenderte PNGs** sind geprüft; die Protokolle bestätigen 34 gestartete Einmalwiedergaben und 13 Ambient-Streamstarts über die voneinander unabhängigen Prüfabschnitte. Das Prüf-APK bleibt getrennt; das Liefer-APK enthält weder diese Activity noch ihre DEX-Klasse.

Bacta-Abbruch, erneutes Benutzen, Notifies, Dauerton, Eingabesperren und Partikel-Checkpoints bestehen zusätzlich mit der endgültigen nativen DLL. Die 41 Cue-Referenzen auf 29 Originalcues und die 7.610.358-Byte-Soundbank sind geprüft. Am endgültigen ZIP bestehen Rampenalpha, ursprüngliche Gegner-PCM-Daten und vier historische Spielstände.

Die verbindlichen Berichte liegen unter `build/mission-collision/complete/validation.json`, `build/mission-collision/android/validation.json`, `build/mission-collision/bacta-lifecycle.json` und `build/mission-collision/package-validation.json`. Die Bedienprüfung des normalen APK verwendet `build/mission-collision/campaign-android`.

Auch diese Bedienprüfung besteht am identischen Liefer-APK und ZIP: tatsächlicher Paketdialog und nativer Import, unveränderte Spielstände beim Import, atomare Ablehnung eines unvollständigen Pakets, Kaltstart, exakte Wiederaufnahme der ursprünglichen Activity nach Android-Hintergrundkill sowie alle drei automatischen Kartenwechsel. Bei 640×360 dp sind alle Touch-Ziele mindestens 48 dp groß; Bewegung, Schuss, Sprung, beide Blickachsen und die gespeicherte, dichteunabhängige Empfindlichkeit sind geprüft. Die gemessene horizontale Drehung beträgt 0,750 gegenüber 0,375 Radiant bei 200 gegenüber 100 Prozent.

Zusammengehörige Lieferdateien liegen eingefroren unter `build/releases/0.51.0`:

- APK: `republic-commando-0.51.0-mission-presentation.apk`, SHA-256 `d79b993aad1c3c59454733fc4ceeb2aaa730dee2658d22a0f936ec60f19ca0db`.
- Szenenpaket: `mission1-scenes.rcpack.zip`, SHA-256 `aecc32559a4b332135c902d56e952b5d54a862b1bac9d927a65d5cf43ea5a9f2`.
- Der Arbeitsstand und die vollständigen Prüfsummen werden in `build/mission-collision/source-and-delivery.json` und `verification-summary.json` zugeordnet. Ein Emulatornachweis ersetzt keine Hör-/Touch-/ARM64-Abnahme auf dem physischen Handy.
