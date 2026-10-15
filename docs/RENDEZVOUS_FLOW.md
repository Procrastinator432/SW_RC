# Originalziel4: Rendezvous with Six-Two

Stand 2026-10-12: Ereignis-/Dialoggrundlage implementiert und geprüft, noch kein spielbarer Scorch-Abschnitt. Die aktuelle spielbare Lieferung bleibt APK0.23.0 plus `analysis/mission4-complete/geo_01a-gunner.rcplay` für Originalziele0–3. Neue Belege liegen ausschließlich unter `analysis/mission5-transition`.

## Originalbefund

Der vollständige geo_01a-Audit erfasst1920 Actor-/Action-Exports. Das originale MissionObjectives-Objekt benennt Ziel4 als „Rendezvous with Six-Two“, Ziel5 als „Command demolition maneuver“ und Ziel6 als „Infiltrate Geonosian spire“. `SpawnSmallBunker` ist ein Geonosianer für die brennende Bunkerinszenierung; daraus folgt kein zusätzlicher Kampfauftrag.

Aus fünf ScriptedTriggern wurden sechs begrenzte Fragmente mit40 Aktionen übernommen:

| Originalscript | Auslöser | Aktionen | Wirkung |
| --- | --- | --- | --- |
| 195 | levelstart | 1746 | Sprengladung Disabled |
| 325 | SpawnSmallBunker | 2092–2103 | Wartezeiten, Feuerschaden/Immolate, Fabrikwechsel und Ereignisse |
| 847 | Conversation04 | 3178–3181 | Vier aufeinanderfolgende Funkaufnahmen |
| 531 | Add62ToSquad | 2518–2526 | Scorch-Begrüßung, Intro, Squad-Aufnahme, HUD-Verknüpfung, DialogDemolition |
| 1128 | DialogDemolition | 3539–3549 | Zwei Fabriken stoppen, Advisor-Dialog, Ziel4 abschließen, Sprengladung aktivieren |
| 195 | ActivateDemoCharge | 1748–1750 | Zieltracker, Auto-Zustand und Sprengmarker |

Spielertrigger458 liegt bei `[7243.9501953125,-10566.3466796875,-2047.6087646484375]`, Radius500/Höhe40. Original: PlayerProximity, einmalig. Scorchs originale Fabrik457 erzeugt Commando62Delta mit AICommando62. DemolitionCharge465 ist ausdrücklich nicht direkt vom Spieler benutzbar; der spätere Sprengauftrag benötigt Squad-Befehle.

## Implementierung und Prüfung

`rc-game::rendezvous` führt getrennte Ereignis-/Zeitspuren aus. Ereigniskaskaden behalten ungenutzte Zeit; unabhängig laufender Funk blockiert die Rekrutierung nicht. Player-Zylinderkontakt, Zeitfortschritt und Effektanwendung sind getrennt. Ungültige Eingaben verändern den Zustand nicht. Wiederholte Ereignisse und Wiedereintritt spielen die ausgewählten einmaligen Spuren nicht erneut ab.

`rc-rendezvous-assets` prüft originale Klassen, Aktionsreihenfolge, Aktivierungsflags und ausgewählte Defaults; acht Original-PCM-Aufnahmen werden mit tatsächlicher Dauer extrahiert. `rc-rendezvous-flow-check` prüft diese konkreten Assets bei20/60/144Hz: ohne Kontakt kein Squad-Beitritt/Zielabschluss; danach Beitritt vor Ziel4 und Sprengmarker. Die Dialogkette bis Zielabschluss dauert12,659819 Sekunden nach Kontakt. Alle Bildraten liefern dieselbe Effektreihenfolge.

Sieben neue Verhaltenstests bestehen; insgesamt1043 Workspace-Bibliothekstests sowie Clippy ohne Warnungen bestanden. Das belegt den ausgewählten Host, keine allgemeine UnrealScript-VM oder native Scriptparität. Effekte sind noch nicht an Game, Fabriken, Companion-KI, HUD und Android-Audio angeschlossen.

## Wegprüfung und konkreter Integrationsblocker

`rc-rendezvous-route-check` untersucht99 bodengestützte Originalanker in einem begrenzten Kartenbereich. Nur die zehn bereits in Zielen0–3 freigegebenen dynamischen Actors werden deaktiviert; die35 BunkerBlown-Hüllen bleiben erhalten. Es werden keine weiteren Kollisionsobjekte gelöscht. Ausgewählte Wegsegmente werden mit der bestehenden Pawn-Bewegung überprüft; die Zielpositionen müssen innerhalb ihrer Originaltrigger liegen.

Der Lauf erreicht den ersten Wegauslöser802, findet jedoch in diesem Gehgraphen keinen Anschluss zum zweiten Auslöser957.15 Anker sind von802 aus erreichbar. Die nächste Graphgrenze führt von PathNode294/Export1161 zu PathNode295/Export1162. Dort melden Sweeps unter anderem StaticMeshActor118/Export1190 (`Arena_HR.ArenaWalls.BSPTrim`) und StaticMeshActor109/Export1189 (`Slab`) als Blockierer. Die Bodenwahl für1162 liegt bei z−2057,87684 statt Original-z−2251,30298, also193,42614 Einheiten höher. Bei1163 liegt sie164,92864 Einheiten höher.

Das ist ein reproduzierbarer Befund der aktuellen Kollisionsauswahl und des begrenzten Gehgraphen. Er beweist keine unpassierbare Stelle im Originalspiel. Original-Kollisionsdispatch, Mesh-Hüllen/Transform und die Bodenwahl an1190/1189/1188 müssen zuerst gegen die Originaldaten geprüft werden; anschließend Geh-/gegebenenfalls belegte Sprungverbindungen und Triggerkontakte erneut testen. Belege: `route/supported-anchors.json`, `route/blocked-route-links.json`, `route/route-validation.json`.

## Nächste zusammenhängende Integration

1. Anschlussweg einschließlich originaler Boden-/Mesh-Kollisionsauswahl klären und mit Pawn-Eingaben bis Trigger458 nachweisen.
2. Scorchs Fabrik457, AI-Skript, Originalmesh/-animation und Begegnungsposition rekonstruieren; ursprüngliche Schlachtfeldfabriken und Bunkerinszenierung binden.
3. Ereignis-Host, acht Audioaufnahmen, Squad-HUD und Zieltracker an Game anbinden. Abschnittssieg von Ziel3 zum Ziel4-Abschluss verschieben, ausschließlich in einer neuen versionierten Szene; alte Szenen weiter unterstützen.
4. Durchgängigen Lauf von der Landung bis Scorch, Neustart und native ABI prüfen; anschließend passende APK/Szene erzeugen und Android prüfen. Der folgende Sprengbefehl bleibt ein eigener Integrationsabschnitt.

## Reproduktion

Aus dem Projektverzeichnis, jeweils mit `CARGO_INCREMENTAL=0`:

```powershell
cargo run --release --locked --offline -p rc-inspect --bin rc-next-mission-audit -- "D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData" analysis/mission5-transition
cargo run --release --locked --offline -p rc-inspect --bin rc-rendezvous-assets -- "D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData" analysis/mission5-transition
cargo run --release --locked --offline -p rc-inspect --bin rc-rendezvous-flow-check -- analysis/mission5-transition/rendezvous-assets.json analysis/mission5-transition/flow-validation.json
cargo run --release --locked --offline -p rc-inspect --bin rc-rendezvous-route-check -- "D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData" analysis/mission4-complete/geo_01a-gunner.rcplay analysis/mission5-transition/route
cargo test --release --locked --offline --workspace --lib
```

Die Wegprüfung endet aktuell mit Exit1 und einem negativen Prüfbericht. Das ist der dokumentierte offene Integrationsbefund. Es gibt für diesen Vorbereitungsstand keine neue APK oder rcplay-Datei.
