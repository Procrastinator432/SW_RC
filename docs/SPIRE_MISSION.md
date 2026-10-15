# Spielbarer Turmzugang – 0.26.0

Stand: 12. Oktober 2026. Die ausgewählten Originalziele 0–6 von geo_01a sind vom bisherigen Bacta-Einstieg bis zum ursprünglichen Ausgangsauslöser zusammenhängend spielbar. Der neue Abschnitt setzt nach der Bunkersprengung fort. Dies ist noch keine vollständige Rekonstruktion aller Kämpfe und Inszenierungen der ersten Mission.

## Lieferung und Bedienung

Gemeinsam verwenden:

- `analysis/mission7-spire/republic-commando-0.26.0.apk` (VersionCode 29, ARM64 und x86_64).
- `analysis/mission7-spire/geo_01a-spire.rcplay` (RCPLAY09, 80.217.777 Bytes).

APK installieren, „Spielabschnitt geo_01a öffnen“ → „Szene öffnen“ wählen und die neue Szene öffnen. Die vorherigen Aufträge einschließlich Scorch/Sprengladung sind wie in [0.25.0](DEMOLITION_MISSION.md) bedienbar. Nach der Sprengung durch den Bunker gehen und der grünen Weganzeige folgen. Beim Artilleriebereich kurz auf Funkmeldung und Barrikadenfreigabe warten, dann dem Weg zum Turm folgen. Die Tür öffnet automatisch in der Nähe. Am originalen Ausgang endet der Abschnitt mit Sieg. Neustart setzt auch Barrikaden, Tür, Funk und Ausgang zurück.

Der Ausgang bewahrt das Originalziel `GEO_01B.ctm#playerstart` als protokollierte Reiseanforderung. Die nächste Karte wird in dieser Version noch nicht geladen.

## Originalbindungen und begrenzte Umsetzung

Script 848 liefert `Conversation05` mit vier ursprünglichen Radioaufnahmen. Trigger 39 startet den ausgewählten Script-32-Anschluss: zwei Sprachmeldungen, `SpawnExitArea`, `GunshipAttack`, `SpiderDroidAttack`, `ArialBombardmentEndLevel`, vier Sekunden Warten, 50.000 Schaden gegen die drei Barrikaden mit ursprünglich jeweils 49.000 Lebenspunkten und `UnblockSpiderDroidBarricade`. Die drei originalen MetalWall2-Meshes 860–862 sind sichtbar und blockieren vor der Freigabe. Nachher verschwinden ihre Geometrie und Kollisionshüllen.

Trigger 24 aktiviert die beiden Türhälften 34/35. Geschlossene und offene Posen verwenden Originalmesh, Rotation, Skalierung und `KeyPos[1]`. Die ursprünglichen Defaults `DelayTime` (0,2 s) plus `MoveTimes[0]` (0,5 s) bestimmen die Freigabe; der begrenzte Host wechselt am Ende auf die offene Pose, ohne kontinuierliche Mover-Interpolation. Öffnungs- und Endgeräusch sind original. `PopUpEndLevel` und der Hinweisschritt aus Script 1065 bleiben protokolliert; die Port-Weganzeige führt zum Ausgang, das originale animierte Pfeilmodell ist noch offen.

Teleporter 9 verwendet ursprünglichen Ort, Radius 1.000 und Höhe 100. Dadurch beginnt sein Berührungsbereich bereits vor der Türmitte; der Test respektiert diese Originalgrenze. Die freie Türpassage wird zusätzlich per Kollisionsabfrage geprüft. Der Laufweg nutzt 14 physisch geprüfte Wegpunkte aus ursprünglichen Pfad-/Triggerpositionen einschließlich BlockedPath 780; vor Trigger 24 liegt ein begrenzter Wartepunkt innerhalb dessen Originalradius.

Die Szene enthält 57 Originalsamples (acht neue Funk-/Türbindungen). Die Texturketten für die neu sichtbaren Barrikaden und Türhälften sind in `build-validation.json` belegt. Die vorherige statische Szene bleibt erhalten; neue Actor-Geometrie gelangt über den bestehenden dynamischen CPU-/GPU-Frame in beide Renderer.

## Abnahme

- 14.746 zusammenhängende normale Spieleingaben, Ziele 0–6, fünf Abschüsse, 20 HP, Sieg und einmalige ursprüngliche Reiseanforderung. Keine Positions- oder Missionszustandsinjektion im Abnahmelauf.
- Artilleriebarrikade und Tür blockieren zunächst. Die Live-Ereignisse geben sie frei; die Abschlussposition berührt den Originalteleporter. Alle fünf Actor-Freigaben und acht neuen Audiorollen treten genau einmal auf.
- Neustart stellt Zustand, Renderframe und geschlossene Kollision exakt wieder her. Das statische GPU-Paket bleibt unverändert.
- Windows-native ABI und Android-x86_64-native ABI wiederholen dieselben Eingaben einschließlich Audio und Neustart. Die APK wird zusätzlich über den regulären Android-Dateidialog geöffnet; GPU, Touch-Schießen, Benutzen, Duckhöhe, Waffensound und Pause werden geprüft.
- Beide Dateilesepfade lesen die neue Szene mit 112 MiB Java-Heap und gleicher SHA-256-Prüfsumme. Fehlerhafte Längen werden abgewiesen, temporäre Dateien entfernt.
- 1.060 Workspace-Bibliothekstests, Clippy ohne Warnungen, Format- und Diff-Prüfung. Die ältere RCPLAY08-Szene endet weiterhin bei Tick 13.397 mit Ziel-5-Sieg.

Die Android-Gesamtabnahme nutzt native ABI-Eingaben; ein vollständiger manueller Touch-Durchlauf und die Prüfung dieser Version auf einem physischen ARM64-Gerät bleiben offen. Frühere Lieferungen bleiben unverändert. Prüfsummen, Originalbindungen und Quellensnapshot stehen in `analysis/mission7-spire/validation-manifest.json`.

## Verbleibende Grenzen und nächster Meilenstein

Die Artillerieereignisse sind live protokolliert und steuern den ausgewählten Barrikadenablauf. Vollständige Gunship-/Spider-Droid-Kampfsequenzen, Projektilfabriken, Bombardierungsanimationen und deren Schaden sind noch nicht simuliert. `SpawnExitArea` hat im untersuchten Karteninventar keinen Empfänger. Der Abschnitt ist deshalb keine vollständige Original-Artilleriebegegnung. Die bisherigen Grenzen zu Squad-Kampf/Folgen/Wiederbeleben, Warrior-Fabriken, Tutorial 3552–3555, Hologrammshader und großen Partikelsystemen gelten weiter.

Als Nächstes folgt der Kampf- und Inszenierungsabgleich von geo_01a: die fehlenden Artillerie-/Fabrikabläufe mit Gegnern, Projektileffekten und Schaden integrieren und den gesamten Lauf erneut abnehmen. Erst danach die vollständige erste Mission als abgeschlossen bezeichnen. Der Kartenwechsel nach GEO_01B wird anschließend als eigener Anschluss umgesetzt.

## Reproduktion

```powershell
$env:CARGO_INCREMENTAL='0'
$data='D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData'
$out='analysis/mission7-spire'
cargo run -p rc-inspect --release --locked --offline --bin rc-spire-originals -- $data $out
cargo run -p rc-inspect --release --locked --offline --bin rc-spire-build -- $data analysis/mission6-playable/geo_01a-demolition.rcplay $out
cargo run -p rc-inspect --release --locked --offline --bin rc-spire-route-check -- $data "$out/geo_01a-spire.rcplay" analysis/mission6-playable/playthrough/input-replay.bin $out
# Rebuild after the offline route proof to embed route.json.
cargo run -p rc-inspect --release --locked --offline --bin rc-spire-build -- $data analysis/mission6-playable/geo_01a-demolition.rcplay $out
cargo run -p rc-inspect --release --locked --offline --bin rc-spire-scene-check -- "$out/geo_01a-spire.rcplay" analysis/mission6-playable/playthrough/input-replay.bin "$out/playthrough"
cargo build -p rc-native --release --locked --offline
python scripts/Validate-SpireAbi.py
scripts/Build-Android.ps1 -Abis arm64-v8a,x86_64
# Build scripts/android-spire-replay.c with NDK x86_64 clang and -lrc_native,
# placing the helper at analysis/mission7-spire/android-spire-replay.
python scripts/Test-SpireAndroid.py
python scripts/Summarize-SpireMission.py
```
