# Originaler Granatenauftrag: Ereignisablauf und Assetprüfung

Stand: 11. Oktober 2026. Originalziel 2 „Pick up grenades“ ist als begrenzter Rust-Ereignishost vorbereitet und geprüft. **Noch kein neuer spielbarer Abschnitt und keine neue APK.** Die spielbare Lieferung bleibt 0.20.1 unter `analysis/mission2-av`. Neue Analysebelege liegen unter `analysis/mission3-pickup`.

## Übernommene Originaldaten

- `geo_01a.ctm`, Script745 / Aktionen2953–2957: Kontakt mit Trigger746 (`DialogGunnerPosition`), Tracker auf `ThermalDetonatorBox01`, zwei blockierende Advisor-Zeilen, anschließend `MissionObjectives`.
- Script747 / Aktionen2958–2968: Trigger694 (`PickedUpGrenades`), Ziel2 abschließen, Missionsanzeige, Tracker `GunnerTarget`, Erinnerungen abschalten, Bestätigungszeile, Granaten-HUD zehn Sekunden blinken lassen, nach 0,5 Sekunden `PopUpGrenades`, weitere zwei Sekunden später `DialogBlowUpGunnerDialog`.
- Pickup610 `CTInventory.ThermalDetonatorBox`: Originalposition `[5261.803711, -4115.015137, -2385.343018]`, Kontaktzylinder Radius45 / Höhe44. Trigger694 sitzt praktisch am selben Ort, hat jedoch Höhe100. Der Auftragstrigger und die tatsächliche Inventaraufnahme sind unabhängig. Der Host erhält diese Besonderheit ausdrücklich; ein Kontakt mit dem höheren Trigger allein erzeugt keine Munition.
- `Properties.ThermalDetonatorDefaults.InitAmmoAmount = 2` und originale Aktion1233 `AddAmmo(-2)` ergeben Startbestand0. Kiste `AmmoAmount = 5`, `ThermalDetonatorAmmoDefaults.MaxAmmo = 5`. `Ammunition.HandlePickupQuery` begrenzt den Bestand, kündigt einen vollen Vorrat nicht erneut an und startet dann keinen Respawn.
- Die Karte setzt `bForceRespawn`, `bInstantRespawn` und `RespawnTime = 0.10000000149`. `Pickup.Sleeping` verwendet `GetRespawnTime` und prüft beim Wiedererscheinen bereits berührende Spieler. Der Host bildet den ausgewählten Inventar-/Wartepfad ab; Karma-Bewegung und Inventarobjektkette werden nicht ausgeführt.

## Originale Geräusche und Darstellung

| Rolle | Originalsample | PCM-Dauer |
|---|---|---|
| Geschützstellung erklären | `GEO_Voice.GEO_01.CA1G1751` | 2,896145 s |
| Granatenauftrag erklären | `GEO_Voice.GEO_01.CA1G1760` | 4,037098 s |
| Aufnahme bestätigen | `GEO_Voice.GEO_01.CA1G1759` | 1,819320 s |
| Inventaraufnahme | `GEN_Sound.Weapons.GenericWeaponPickup` | 0,734331 s |

Die vier unveränderten WAV-Dateien liegen in `sounds/0.wav` bis `sounds/3.wav`. Die Wartezeiten stammen aus PCM-Datenlänge und Byte-Rate, nicht aus geschätzten Sprachpausen. Wiedergabe und Lebenszyklus im Android-Host werden erst bei Szenenintegration abgenommen.

`PickupMeshes.ClonePickups.ThermalDetonatorBox` ist als `pickup.rcscene` vorbereitet: 402 Dreiecke, alle mit originaler Basistexur und UV0, keine untexturierten Flächen. `PickupTextures.ClonePickups.ThermalDetonatorBox_Shader` besitzt `Opacity=None` und `OutputBlending=OB_Normal`; deshalb ist die Grundfläche deckend. Die Alphawerte des Diffusebilds dürfen die Kiste nicht ausstanzen. Ohne diesen Adapter verschwanden große Teile der Kiste. `visual-validation.json` belegt die Materialdefaults und 65.167 ursprünglich nicht deckende Texel. `pickup-front.png` und `pickup-back.png` wurden als Diagnoseansichten erzeugt; Vorderansicht visuell kontrolliert. Vollständige Shader-, Beleuchtungs- und Karma-Parität bleiben offen.

## Prüfungen und Anschlussweg

Sechs neue Rust-Tests prüfen getrennte Inventar-/Auftragskontakte, wiederholte Berührung, unabhängige Skriptzweige, Originalaktionsreihenfolge, volle Inventare, Respawn, tote Spieler, fehlendes Inventar, unterschiedliche Zeitschritte und unveränderten Zustand nach ungültigen Eingaben. `rc-grenade-check` führt denselben Host zusätzlich mit den tatsächlich extrahierten Originalpositionen und Sprachdauern aus. Ergebnisse: `flow-validation.json` und `flow-trace.json`. Der Host erzeugt Effekte; er ruft keine allgemeine Skript-VM auf.

Die Anschlussroute wurde zusätzlich geprüft und **besteht noch nicht**. `rc-grenade-route-check` baut im nach dem Kampf freigegebenen Kollisionszustand einen Graphen mit 152 Originalankern. Er findet keinen vollständig gehbaren Pfad zum ursprünglichen Tracker764. 15 Anker sind vom bisherigen Ende erreichbar. Die nächstgelegenen Übergänge werden unter anderem von `KarmaProp4` und `KarmaProp7` blockiert (`blocked-route-links.json`). Script911 löst nach `CrashSiteSecured` und 0,5 Sekunden `KnockOverWall` aus; die vier zugehörigen Props909/910/912/913 sind bisher statische Collider. Original `KarmaProp.Trigger` aktiviert dagegen `PHYS_Karma` und weckt die Körper auf.

Ein ausdrücklich getrennter Diagnoseversuch unter `without-wall/` entfernt nur diese vier Collider. Er erweitert den erreichbaren Bereich auf 17 Anker, reicht aber ebenfalls nicht bis Tracker764. Weitere Übergänge treffen auf Welt-/Meshkollision. Damit ist das Entfernen der Mauer weder eine vollständige Lösung noch eine freigegebene Spieländerung. Ungültige Bewegungsprojektionen einzelner Kandidaten werden als abgelehnte Verbindungen protokolliert; sie werden nicht als passierbar angenommen. `route-validation.json` enthält bewusst `passed:false`.

Tracker764 liegt bei Z=-2344. Der Bodenprobe-Start bei +120 liegt hier in Geometrie; höhere Proben finden einen freien Standpunkt bei Z≈-2005. Diese Anpassung ist nur eine Diagnose für die Wegführung und verschiebt den Originalactor nicht. Die Eignung für den tatsächlichen Missionsweg bleibt zu klären.

## Nächste Integration

1. Originalen Übergang nach der Absturzstelle rekonstruieren: `KnockOverWall`, korrekte Collider-/Propzustände, Original-Navigationsverbindungen und tatsächlich gehbarer Weg zu Tracker764 und zur Kiste. Verbleibende Mesh-/BSP-Kontakte anhand der Quellobjekte prüfen.
2. Den geprüften Pickup-Host, die vorbereitete Kiste und vier Samples in das Missionspaket integrieren; Sieg nach Ziel1 durch Fortsetzung ersetzen. Inventar und tatsächliche Aufnahme sichtbar machen, Pause/Neustart/CPU/GPU absichern.
3. Einen durchgehenden Lauf Bacta → Absturzstelle → Granatenaufnahme mit normalen Eingaben prüfen und erst dann zusammenpassende APK und Szene ausliefern.

Ziel3 „Destroy gun position with grenades“ folgt separat. Granatenwurf, Projektil-/Explosionsregeln und Geschützzerstörung sind noch nicht implementiert. `PopUpGrenades` und die anschließenden Hinweis-/Dialogskripte werden aktuell als Ereignisse ausgegeben, nicht als funktionsfähige Wurfbedienung dargestellt.

## Reproduzieren

Im Projektverzeichnis, mit `GAME_DATA` auf den lokalen GameData-Ordner gesetzt:

```powershell
cargo run --release --locked --offline -p rc-inspect --bin rc-grenade-assets -- $GAME_DATA analysis/mission3-pickup
cargo run --release --locked --offline -p rc-inspect --bin rc-grenade-check -- analysis/mission3-pickup
cargo run --release --locked --offline -p rc-inspect --bin rc-grenade-visual-check -- $GAME_DATA analysis/mission3-pickup
cargo run --release --locked --offline -p rc-inspect --bin rc-grenade-route-check -- $GAME_DATA analysis/mission2-av/geo_01a-crash.rcplay analysis/mission3-pickup
```

Der letzte Befehl liefert beim aktuellen Stand erwartungsgemäß einen Fehlercode und schreibt die gescheiterte Wegprüfung aus. `scripts/Summarize-OriginalGrenades.py` prüft die erfolgreichen Grundlagenbelege, verlangt die offen ausgewiesene Weglücke und bindet Quellen, Originaldateien und die unveränderte vorherige Lieferung per SHA-256.
