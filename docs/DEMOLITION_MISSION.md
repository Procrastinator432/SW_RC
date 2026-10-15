# Spielbarer Sprengabschnitt – 0.25.0

Stand: 12. Oktober 2026. Der ausgewählte Sprengauftrag „Command demolition maneuver“ ist in die zusammenhängende geo_01a-Szene integriert. Die Originalziele 0–5 sind vom bisherigen Bacta-Einstieg bis zur Bunkeröffnung spielbar. Die gesamte erste Mission bleibt offen; nächster Abschnitt ist Ziel 6 „Infiltrate Geonosian spire“.

## Lieferung und Bedienung

Gemeinsam verwenden:

- `analysis/mission6-playable/republic-commando-0.25.0.apk` (VersionCode 28, ARM64 und x86_64).
- `analysis/mission6-playable/geo_01a-demolition.rcplay` (RCPLAY08, 70.374.290 Bytes).

APK installieren und die neue Szene mit „Spielabschnitt geo_01a öffnen“ → „Szene öffnen“ auswählen. Der Einstieg und die bisherigen Aufträge bleiben wie in [0.24.0](SCORCH_RENDEZVOUS.md).

Nach dem Treffen mit Scorch führt die Zielanzeige zum Bunker. Auf die Ladungsstelle zielen und **BENUTZEN** drücken, um Scorch den Auftrag zu geben. Er läuft dorthin und setzt die Ladung. Nach der Bereitschaftsmeldung in die Nähe der Ladung gehen, darauf zielen und **BENUTZEN erneut drücken**, um den Zünder zu wählen. Anschließend mehrere Schritte zurückgehen und **FEUER** drücken. Die Explosion folgt dem Druck auf den Zünder in der Originalanimation. Der zerstörte Eingang ist begehbar; nach der Originalbestätigung endet dieser Abschnitt mit Sieg. „Neustart“ setzt auch die Ladung, Scorch, Sounds und die geschlossene Bunkerkollision zurück.

## Umsetzung und Herkunft

Marker 466, Charge 465, Teilnehmer Six-Two/62, Prop 851 und NavigationPoint 1094 stammen aus geo_01a. Der Begleiter verwendet die bereits physisch geprüfte Route, die Originalclips `DemolitionSetChargeStart/Loop/Stop` und die gelbe Scorch-Textur. Das Platzierungsereignis folgt dem Originalnotify; der Setup-Timer beträgt zehn Sekunden. Die gesetzte Ladung wechselt zwischen ursprünglicher inaktiver und aktiver Materialbindung. Während des Setzstarts hängt das ursprüngliche Ladungsmodell an `weaponAttach_L`; diese Auswahl ist eine begrenzte Attachment-Regel des Ports.

Zünder und Hände stammen aus `MarkerIcons.Demolition.Detonator` und `HudArms.HudArmsHands`. Die animierte linke Attachment-Matrix bewegt das Zündermodell. Als Bereitschaftspose dient Frame 0,4 des Originalclips; beim Feuern läuft `Detonate` mit ursprünglicher Rate. `AnimNotify_FireWeapon` bei 0,555555582 löst die Explosion aus. Vorher ist Rückzug möglich. Der Zünder verbraucht keine Blastermunition und wirft keine Granate.

1.165 ursprüngliche Bunkerflächen wurden aus der statischen Szene in das dynamische Objekt verschoben. Beim Explodieren wird Bunker2 durch das originale BunkerBlown2 ersetzt, einschließlich seiner 23 Kollisionshüllen. Schaden verwendet 15.000/Radius 256 und den vorhandenen begrenzten Explosionsadapter. Die ursprüngliche Reihenfolge prüft den Schaden vor dem Prop-Wechsel. Script 195 bestätigt Ziel 5 und meldet nach vier Sekunden `Conversation05`; Script 536 liefert Setz-, Bereitschafts- und Erinnerungssprache.

Die Szene enthält 49 Originalsamples: die bisherigen 33, zwei weitere Tutorialaufnahmen und 14 Spreng-/Animations-/Dialogbindungen. Für `SoundMultiple` wird wie im bisherigen Waffenpfad der erste gespeicherte PCM-Unterklang verwendet; Herkunft und Auswahl stehen in `audio-variants.json`. Die Notify-Intervalle berücksichtigen Schleifen und den im selben Tick begonnenen Stop-Clip, damit dessen frühes Button-Geräusch nicht ausfällt.

Der Android-Dateileser reserviert die bekannte Dateigröße einmal. Bei unbekannter Länge begrenzt und puffert er zunächst in einer temporären Cachedatei. Dadurch entfällt die frühere Verdopplung auf 128 MiB plus anschließende Payload-Kopie. Beide Pfade lesen die vollständige Szene mit nur 112 MiB Java-Testheap und identischer Prüfsumme; Größenwechsel, leere und abgeschnittene Dateien werden abgewiesen.

## Abnahme

- 13.397 aufeinanderfolgende normale Eingaben: Ziele 0–5, fünf Abschüsse, 20 HP, Sieg. Kein Positionsversetzen oder Mission-State-Injection.
- Falsche Blickrichtung löst keinen Squad-Befehl aus; eine scharfe Ladung bleibt mindestens weitere 20 Sekunden ohne FEUER erhalten. Rückzug, einmalige Zündung am Originalnotify und unveränderte Blasterschusszahl sind geprüft.
- Vorher blockierender Bunker, danach tatsächlich begangener Eingang mit den zerstörten Originalhüllen. Exakter Zustands-/Renderframe-Neustart und wieder geschlossener Collider; statisches GPU-Paket unverändert.
- Windows- und Android-x86_64-ABI wiederholen denselben Lauf, prüfen Originalaudio einschließlich aller Sprengnotifies und Neustart. Die 13 Scorch-Aufnahmen und alle einmaligen neuen Audio-Rollen laufen genau einmal; das Loop-Button-Sample läuft 20-mal.
- Android API 35: neue APK über reguläre Dateiauswahl geladen, GPU-Ausgabe mit 125 Texturen, Touch-Schießen/Originalwaffenaudio, Benutzen, tatsächliche Duckhöhe und Pause geprüft.
- 1.055 Workspace-Bibliothekstests, Clippy mit `-D warnings`, Format- und Diff-Prüfung. RCPLAY07-Regressionslauf endet weiterhin bei Tick 10.448 mit dem bisherigen Ziel-4-Sieg.

Die vollständige Eingabeabnahme auf Android erfolgt über die native ABI, nicht als kompletter Touch-Durchlauf. Die Prüfung dieser Version auf einem physischen ARM64-Gerät bleibt offen. Frühere Lieferungen und die vorbereiteten Sprengnachweise bleiben unverändert; Prüfsummen, Quellensnapshot und Originalbindungen enthält `analysis/mission6-playable/validation-manifest.json`.

## Grenzen und nächster Abschnitt

Dies ist der ausgewählte spielbare Sprengauftrag, keine vollständige Originalparität. Allgemeine Squad-GoalQueue, Folgen/Kämpfen/Wiederbeleben, Abbruch/Fortsetzung eines NPC-Setzauftrags und schwierigkeitsabhängige Setup-Zeiten bleiben offen. Die neue Bereitschafts-/Attachment-Pose ist eine dokumentierte Portregel. Das ursprüngliche Hologramm vor der Platzierung benötigt einen weiteren HardwareShader-Adapter; bis dahin zeigt die vorhandene HUD-Wegführung die Ladungsstelle an. Explosionssprites verwenden den bisherigen Bunkeradapter, nicht die vollständige große Originalpartikelanlage.

`SpawnDemoDroids` besitzt im untersuchten Map-Inventar keinen Empfänger. Die gesonderte `SpawnWarriorAttack`-Inszenierung und Tutorialaktionen 3552–3555 (zufällige Wartezeit, Timer, weitere Warnung/Fabrikauslösung) sind noch keine vollständige Live-Erweiterung; die beiden Sprachaktionen 3550/3551 sind integriert. StopFactory-/Tracker-Anforderungen werden im Ereignisprotokoll bewahrt, vorhandene zusätzliche Fabrikinszenierungen werden dadurch nicht nachträglich simuliert. Diese Fälle gehören vor der Gesamtmission-Abnahme in den verbleibenden Kampf-/Inszenierungsabgleich.

Als Nächstes: `Conversation05`, Originalziel 6 und den Anschlussweg durch den geöffneten Bunker bis zum Turm rekonstruieren, sichtbare Oberflächen und Audio früh prüfen, dann als nächsten zusammenhängenden Spielabschnitt integrieren. Abschließend folgen der vollständige Missionslauf und der Vergleich der noch begrenzten Originalsysteme.

## Reproduktion

```powershell
$env:CARGO_INCREMENTAL='0'
$data='D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData'
$out='analysis/mission6-playable'
cargo run -p rc-inspect --release --locked --offline --bin rc-demolition-originals -- $data $out
cargo run -p rc-inspect --release --locked --offline --bin rc-demolition-equipment -- $data $out
cargo run -p rc-inspect --release --locked --offline --bin rc-demolition-build -- $data analysis/mission5-playable/geo_01a-scorch.rcplay analysis/mission6-demolition $out
cargo run -p rc-inspect --release --locked --offline --bin rc-demolition-scene-check -- "$out/geo_01a-demolition.rcplay" analysis/mission5-playable/playthrough/input-replay.bin "$out/playthrough"
cargo build -p rc-native --release --locked --offline
python scripts/Validate-DemolitionAbi.py
scripts/Build-Android.ps1 -Abis arm64-v8a,x86_64
# Build scripts/android-demolition-replay.c with NDK x86_64 clang and -lrc_native,
# placing the helper at analysis/mission6-playable/android-demolition-replay.
python scripts/Test-DemolitionAndroid.py
python scripts/Summarize-DemolitionMission.py
```
