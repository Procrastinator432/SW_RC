# Level-Weltbindung und gemeinsame Körperkontakte

Stand 6. Oktober 2026. Der Port liest die tatsächliche Weltmodell-Referenz aus dem Level-Objekt und prüft mathematische AABB-Körper gegen Welt-BSP sowie einfache Mesh-Hüllen am gespeicherten Actor-Pose. Android-Prüfung ist auf Nutzerwunsch bis zur abschließenden Integration verschoben; die APK bleibt vorerst bei 0.7.

## Weltmodell aus dem Level lesen

`rc-package::level::world_binding` benötigt genau einen Engine.Level-Export. Nach dessen UObject-Properties folgt ab Paketversion 153 eine zusätzliche Objekt-/Containerreferenz, dann die CompactIndex-Actorlistenlänge und die Actorreferenzen. FURL enthält vier Strings (Protocol/Host/Map/Portal), eine Liste von Optionsstrings und u32-Port/Valid. Direkt danach schreibt ULevel die Model-Referenz.

Der Reader unterstützt die vorhandenen Versionen 151–159 und Licensee 0–1. Er prüft Array-/String-/Payload-Grenzen und Objektverweise, weist mehrere Level-Exports ab und verlangt eine positive lokale Referenz auf Engine.Model. Null, importierte, außerhalb liegende und anders typisierte Weltverweise werden abgewiesen. Der Bericht hält Level-/Model-Exportindizes (nullbasiert), Objektpfad, Actor-Slots, Präfixverbrauch und verbleibende Bytes fest. Der übrige Level-Tail bleibt opaque.

Alle 79 Karten besitzen eine lesbare Bindung; in 45 heißt das referenzierte Weltmodell anders als Model1. Beispielsweise verweist ctf_hangar auf Model1281 und ras_02a auf Model410. `rc-render::Scene::from_map`, PC-Szenenimport, `rc-bsp-check`, `rc-hull-check` und `rc-mesh-probe` verwenden jetzt diese Referenz. Der Renderer vereinigt weiterhin keine untransformierten Brush-Models.

Native Belege:

- [ULevel / ULevelBase Serializer](../analysis/decompiled/level-serializer.c): engine.dll 0x1044a350 und 0x10449450.
- [FURL / Actorliste](../analysis/decompiled/level-prefix-helpers.c): 0x1055cf20 und 0x10448360.

## Vollständiger Welt-BSP-Audit

[level-bsp-validation.json](../analysis/collision/level-bsp-validation.json) prüft 9.123 Model-Tails ohne Fehler mit der tatsächlichen Weltbindung. Seine Punkt-/Linienproben verwenden weiterhin explizite Start-Transforms.

[level-hull-validation.json](../analysis/collision/level-hull-validation.json) löst alle 755 Start-Transforms auf und führt 4.530 Welt-Körperproben aus: 2.700 blockiert, 1.824 frei, sechs unvollständig. Sämtliche 9.123 Modelle und 26.840 festen Hüllen sind strukturell lesbar.

Die sechs unvollständigen Proben gehören zu ras_02a.PlayerStart0: Model410 enthält ein festes Blatt ohne Hülle. Die mathematische Diagnose lehnt eine vollständige Freigabe dann ausdrücklich ab. Der native Traversierungshelfer besitzt einen Skip-Zweig für iCollisionBound=-1; dessen konkrete Bedeutung für diese Karte und das Spielverhalten muss noch abgeglichen werden. Es wird keine Hülle geraten oder eine fehlerfreie Vollprüfung behauptet. Der Audit liefert deswegen Exitcode 1. Der vorherige Bericht hatte dieses Blatt bereits gespeichert; die damalige dokumentierte Anzahl null war ein Auswertungsfehler und wurde korrigiert.

## Körper gegen transformierte Meshes

Die Körperprüfung wählt bei nicht null Ausdehnung UseSimpleBoxCollision. Zylinder haben Vorrang, das einfache Model wird nur bei passendem Flag gewählt; fehlendes Model bei aktiviertem SimpleBox bedeutet gemäß dem nativen Dispatch frei. Ausgewählte komplexe und Zylinder-Formen bleiben ausdrücklich nicht unterstützt.

`HullSet::transformed` bringt lokale einfache Model-Hüllen in den Weltraum. Die Transformationsmatrix enthält dieselbe quantisierte Unreal-Rotation wie die Darstellung, DrawScale/DrawScale3D, PrePivot und Location. Ebenennormalen werden mit der invers-transponierten Matrix transformiert, einschließlich negativer und nicht gleichmäßiger Skalen. Originale Box-Bounds werden als sechs orientierte Ebenen erhalten; eine umschließende Welt-AABB dient der frühen Ablehnung. Die Spieler-Ausdehnung wird erst danach entlang der Weltachsen angewendet. Eine rotierte Hülle wird dadurch nicht auf ihre größere umschließende Box reduziert.

`StaticBodyWorld::sweep` prüft Welt und sämtliche gelieferten Mesh-Formen, meldet Fehler unabhängig von bekannten Kontakten und sortiert Kontakte nach Eintrittsanteil. Ergebniszustände Clear/Blocked/Indeterminate und complete folgen derselben Regel wie die Linienabfrage. complete gilt nur für diese Sammlung. Nicht klassifizierte Exports werden weiterhin separat berichtet.

## PC-Integrationsprüfungen

| Karte | Weltmodell | Klassifizierte Mesh-Actors | Körperproben | Blockiert / frei | Unvollständig |
|---|---|---:|---:|---:|---:|
| geo_01a | Model1 | 383 | 48 | 8 / 40 | 0 |
| entry | Model1 | 0 | 48 | 48 / 0 | 0 |
| ctf_hangar | Model1281 | 486 | 1.920 | 908 / 1.012 | 0 |

Die Proben benutzen Radius 40 und halbe Höhe 84 aus PlayerCommando-Defaults, also Extent [40,40,84]. In geo_01a blockiert der Körper zusätzlich Meshes entlang ±X, die die dünne Linie verfehlt. In entry liegt der Testkörper bereits am Kamera-Anchor in Überlappung; sämtliche Bewegungsproben melden einen Startkontakt. Das ist keine korrekt gespawnte Figur: Die PlayerStart-Position wird nur als Diagnosemittelpunkt verwendet. Spawn-Auswahl, Platzierung, Augenhöhe und Depenetration bleiben eigene Aufgaben.

Berichte: [geo_01a](../analysis/collision/geo_01a-body-probes.json), [entry](../analysis/collision/entry-body-probes.json), [ctf_hangar](../analysis/collision/ctf_hangar-body-probes.json).

Der PC-Szenenimport von ctf_hangar mit Model1281 wurde zusätzlich ausgeführt: 848 Welt-BSP-Dreiecke, 431 gerenderte Mesh-Instanzen, 358.304 Gesamtdreiecke und ein 24.694.163-Byte-Snapshot. 29 Mesh-Subklassen bleiben im Renderer ausdrücklich ausgelassen; die Kollisionsdiagnose hat einen eigenen erweiterten Actor-Importer. [Szenenbericht](../analysis/scenes/ctf_hangar-level-bound/scene.json).

53 Rust-Tests und Clippy über alle Targets bestehen. Neue Tests decken die Level-Versiongrenze 153, umbenannte Modelle, sämtliche Präfix-Trunkierungen, ungültige/mehrdeutige Referenzen einschließlich i32-Minimum, Box-/Line-Flag-Unterschiede, transformierte Bounds mit Rotation/Pivot/negativem Maßstab sowie sortierte Kontakte mit gleichzeitig fehlenden Formen ab. Der Hüllenpfad berücksichtigt jetzt ausdrücklich das native Laden mit Flags & 0x1f vor der CSG-Prüfung.

## Grenzen und nächster Schritt

Der Sweep bleibt f64-Mathematik mit konservativen Kontaktgrenzen. Native f32-Toleranzen, Backoff, FCheckResult-Attribute, komplexe/Karma-/Zylinder-Prüfungen und dynamische Actor-Zustände sind nicht vollständig portiert. Die Darstellung und Diagnose sind noch keine Spielerphysik. Nächste Arbeit: Startüberlappung und Spawn-Platzierung klären, Kontaktbegrenzung und Sliding entwickeln, Boden-/Stufenlogik ergänzen und anschließend das Gesamtverhalten in Android prüfen.

```powershell
cargo run -p rc-inspect --bin rc-hull-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/collision/level-hull-validation.json
# Exitcode 1 wegen sechs ausdrücklich unvollständiger ras_02a-Proben erwartet.
cargo run -p rc-inspect --bin rc-mesh-probe -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Maps\ctf_hangar.ctm' analysis/collision/ctf_hangar-body-probes.json
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```


2026-10-06: Platzierung und Kontaktgrenzen erweitert. body_placement unterscheidet Free/Touching/Penetrating, sweep_motion erlaubt tangentiale und auswärts gerichtete Bewegung bei Berührung; ursprüngliche geschlossene sweeps unverändert. 59 Tests/Clippy bestanden. 168 Positionsvorschläge und 42 exakte Bodenpositionen in geo_01a/entry/ctf_hangar geprüft; 41 horizontale Bewegungen Finished, eine Stopped, keine Query-Fehler. Korrektur: entry-Start berührt Geometrie, dringt aber nicht ein; früheres start_overlapping schloss Berührung ein. Keine automatische Spawn-Auswahl, Freistellung, Schwerkraft, Stufen oder Android-Integration. Android-Prüfung weiterhin zum Schluss. Details: D:\Rust Projects\RepublicCommandoAndroid\docs\MOVEMENT.md.
