# Originalziel5: vorbereiteter Sprengablauf

Historischer Vorbereitungsstand. Die anschließende spielbare Lieferung 0.25.0 mit APK, Szene und Android-Abnahme ist in [DEMOLITION_MISSION.md](DEMOLITION_MISSION.md) dokumentiert; die eingefrorenen Belege dieses Zwischenstands bleiben erhalten.

Stand 2026-10-12: Der ausgewählte Ablauf für „Command demolition maneuver“ ist rekonstruiert und als separater Rust-Host geprüft. **Noch keine Integration in eine spielbare Szene und keine neue APK.** Die auslieferbare Version bleibt0.24.0 mit Originalzielen0–4. Dieser Zwischenstand bereitet den nächsten zusammenhängenden Integrationsmeilenstein vor.

## Originaldaten und Verhalten

Marker466 verweist auf Charge465; die Ladung zerstört Prop851/Prop12 und gibt NavigationPoint1094 frei. Die Markeraktion startet `SpawnDemoDroids`; das Setup löst `DemoUnderway` aus. Nur der eingetragene Teilnehmer darf das einmalige `MDplaceCharge`-Ereignis verwenden. Charge465 ist für Spieler nicht direkt setzbar, verwendet für NPCs den Setup-Timer, speichert Fortschritt und wird weder durch Schüsse noch durch einen automatischen Explosionstimer ausgelöst.

Die ursprünglichen Clips `DemolitionSetChargeStart` (29Frames/30Hz), `DemolitionSetChargeLoop` (60/30) und `DemolitionSetChargeStop` (39/30) sind geladen. Start-Notify321 löst `MDplaceCharge` bei normalisiert0,627241373 aus; danach läuft `ChargePlaced.Duration=10s`. Der Host wechselt nach dem Setup in den Zustand einer scharfen Ladung und lässt anschließend den Stop-Clip auslaufen. Die Verwendung des Stop-Clips nach dem Timer ist eine begrenzte Host-Ausführungsregel; die native Marker-GoalQueue ist noch nicht vollständig rekonstruiert. Originale Button-Sound-Notifies316–320 sind im Audit enthalten, aber noch nicht an eine Live-Audiolaufzeit angeschlossen.

Wichtig für die nächste Eingabeintegration: Der Spieler wählt den Zünder innerhalb des originalen `PlayerRange=192`. `HandSignal.SetupPlunger` behält die Ladung als `TargetActivate`; `FireWeapon` löst sie anschließend aus. Der Spieler kann daher vor dem Auslösen zurücktreten. Das Auswählen allein detoniert nicht. Der vorbereitete Host unterscheidet diese beiden Schritte. Die lokale Explosionskonfiguration ist15000Schaden mit Radius256; ein sofortiges Auslösen bei der Auswahl würde den Spieler gefährden und entspricht nicht diesem Bedienablauf.

Die Scharfschaltung meldet `DemoChargeArmed`. Die Sprengung meldet `BunkerDoorDestroyed`, fordert den Prop-Zustandswechsel, Originalschaden und Navigationsfreigabe an. Script195/1752–1760 aktualisiert Ziel5, zeigt das nächste Ziel, beendet drei Fabriken und startet nach4s `Conversation05`. Script536/2536–2557 liefert Setz-/Bereitschaftsdialoge, wartet auf Scharfschaltung und überspringt nach der Explosion die Erinnerungen. Die drei originalen IfCondition/GotoLabel/EndSection-Gruppen werden auf eine ausdrücklich begrenzte `IfExplodedEnd`-Operation abgebildet; kein allgemeiner Bytecode-Interpreter wird behauptet.

## Nachweise

- 65 Originalactor-/Action-Einträge mit aufgelösten Defaults, verschachtelten ActivationInfo/TransitionInfo, Action-Reihenfolgen und Notify-Objekten: `analysis/mission6-demolition/original-bindings.json`.
- 24 ausgewählte Aktionen und neun Original-PCM-Aufnahmen: `demolition-assets.json`, `sound-bindings.json` und `sounds/`.
- Zehn Verhaltenstests prüfen Teilnehmer-/Zielgates, wirkungsloses Warten vor Ankunft, einmalige Platzierung, Setup-Zeitpunkt, Scharfschaltung vor Stop-Ende, Zünderauswahl und Rückzug, explizites einmaliges Auslösen, Dialogabbruch, Zeitpartitionen und atomare Fehlerbehandlung.
- Sechs zusätzliche Originaldatenläufe bei20/60/144Hz prüfen jeweils Auslösen und unbegrenztes Warten mit scharfer Ladung: `flow-validation.json`. Die Ankunft wird dort ausdrücklich durch die Host-API vorgegeben; dies ist kein Gameplay-Eingabelauf.
- 15 Textur-/Poseproben der drei Originalclips samt gelber Scorch-Textur: `animation-validation.json` und PPM/PNG-Bilder. Die Sichtbarkeit im Diagnosebild wird geprüft.
- Der Weltprüfer wiederholt zunächst die10448 akzeptierten normalen Eingaben bis Ziel4. Anschließend erreicht ein eigenständiger physischer Begleitercontroller den Marker466 über Originalanker ohne Springen, Positionsversetzen oder zusätzliche Kollisionsfreigaben. Der akzeptierte Spielzustand bleibt unverändert. `world-validation.json` und `bunker-dynamic.json` bereiten den dynamischen Austausch Bunker2 → BunkerBlown2 vor; die zerstörte Form besitzt weiterhin23 Original-Kollisionshüllen. Das ist noch keine Abnahme eines spielbaren Durchgangs.

## Nächste Integration

1. Neue Szenenschicht anlegen, Goal5 nach dem Ziel4-Abschluss aktivieren und die geprüften Laufwege/Originalclips an den bestehenden Scorch-Controller anschließen.
2. Marker mit Squad-Befehl bedienbar machen, Zünderauswahl vom Auslösen trennen, dazu HUD und passende erste-Person-Darstellung/Attachment ergänzen. Originaltutorial3550–3555 und die Warrior-/Demo-Fabrikbindungen klären und anschließen.
3. Prop12 aus dem unveränderlichen statischen Paket in die vorbereitete dynamische Objektgruppe verschieben; BunkerBlown2-Mesh und seine23 Hüllen beim Explodieren einsetzen. Explosionsschaden, Effekte, Audio, Navigationsfreigabe und tatsächlichen Durchgang prüfen.
4. Zusammenhängende normale Eingaben0–5, wiederholte Befehle, Abbruch/Tod, sicheren Zünderrückzug und exakten Neustart abnehmen. Anschließend neue APK/Szene bauen und Android prüfen.

Die neue Host-API führt Effekte als Anforderungen aus; sie bewegt keine Figur, ersetzt keine Kollision, spielt keine Sounds und verursacht keinen Schaden. Allgemeine Squad-Befehlswarteschlange, Kampf/Folgen/Wiederbelebung, Schwierigkeitsskalierung des Setup-Timers, Abbruch/Fortsetzen einer NPC-Setzaktion, vollständige Fabriksimulation und Originalparität bleiben offen. Der Host verwendet für den ausgewählten Prüffall den unveränderten Default-Multiplikator1.

## Reproduktion

```powershell
$env:CARGO_INCREMENTAL='0'
$data='D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData'
$out='analysis/mission6-demolition'
cargo run --release --locked --offline -p rc-inspect --bin rc-demolition-originals -- $data $out
cargo run --release --locked --offline -p rc-inspect --bin rc-demolition-assets -- $data $out
cargo run --release --locked --offline -p rc-inspect --bin rc-demolition-flow-check -- $out
cargo run --release --locked --offline -p rc-inspect --bin rc-demolition-animation-check -- $data analysis/mission5-playable/scorch-mesh.json $out
cargo run --release --locked --offline -p rc-inspect --bin rc-demolition-world-check -- $data analysis/mission5-playable/geo_01a-scorch.rcplay analysis/mission5-playable/playthrough/input-replay.bin analysis/mission5-playable/route-crouched/route-graph.json $out
```
