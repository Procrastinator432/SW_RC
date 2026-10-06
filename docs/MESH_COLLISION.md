# Originale StaticMesh-Kollisionsdaten

Stand 6. Oktober 2026. `rc-package::mesh_collision` liest den originalen Kollisionspräfix hinter den Render-Streams. `mesh_query` ergänzt die Formauswahl und boolesche Linienproben gegen einfache BSP-Formen am PC. Vollständige UStaticMesh::LineCheck-Hitdaten, Spieler-Ausdehnung und Bewegung sind noch nicht umgesetzt. Die Android-App bleibt bei der zuletzt geprüften 0.7.

## Gelesenes Format

Nach dem Wire-Index-Buffer folgen eine CompactIndex-Referenz auf ein optionales Engine.Model, ab Paketversion 151 ein gespeichertes `UseCollisionForBakedShadowsOnly`-Wort, die CollisionTriangle-Tabelle, die CollisionNode-Tabelle und ein FVector zur Skalierung komprimierter Bounds. Ältere Triangulationsformate unter Version 133 werden abgewiesen. Lazy-Editor-Dreiecke, Bump-Daten und weitere nachfolgende Tails bleiben unverarbeitet; der Bericht hält den tatsächlichen Präfixverbrauch und die Restgröße fest.

Ein CollisionTriangle enthält drei u16-Verweise in den Render-Vertex-Stream sowie einen CompactIndex auf einen Materialslot. Ein CollisionNode enthält einen u16-Dreieckverweis, drei u16-Kinder (coplanar/front/back, Sentinel 65535) und sechs signed i16-Bounds. Die Bounds werden komponentenweise mit dem gespeicherten FVector multipliziert. Die Knoten können dasselbe Dreieck mehrfach verwenden; Dreieck- und Knotenanzahl müssen daher nicht übereinstimmen.

Der Reader prüft Paket-/Payload-Grenzen, Vertex-/Material-/Model-Referenzen, Kinder, Dreieckverweise, invertierte Bounds, nicht endliche/negative Skalierungen und Zyklen. Eine lokale einfache Form muss tatsächlich Engine.Model sein. Ihr BSP-Präfix und Solid-Zustand werden zusätzlich mit den vorhandenen Model-Readern geprüft. Fehler dieser Form werden separat von Fehlern des Mesh-Präfixes ausgewiesen.

## Gesamter lokaler Bestand

| Befund | Anzahl |
|---|---:|
| StaticMesh-Exports | 2.447 |
| Gelesene Kollisionspräfixe | 2.446 |
| Kollisionsdreiecke | 948.512 |
| Kollisionsbaumknoten | 2.526.483 |
| Referenzierte lokale einfache Models | 1.719 |
| Als BSP-Kollisionsstruktur validierte einfache Models | 1.696 |
| Ausgenommene einfache Models | 23 |

Der eine Mesh-Fehler ist weiterhin `markericons.SetTrap.TrapXSpotIcon`: nicht endlicher Renderwert bei Payload-Byte 890. Sein Kollisionspräfix wird nicht anhand geratener Offsets gelesen.

Bei 18 einfachen Models ist die Paketversion älter als der bisher unterstützte BSP-Reader (151+). Bei fünf weiteren lehnt der SolidBsp-Validator eine ungültige Ebene ab: assaultship_gm ventShafts.lightBrace/doorBrace sowie upperkashyyyk_gm Bridge.Brdg_Window_Vista, Buildsext.temp_Bterrace und Bridge.Brdg_area4ceil. Der Bericht enthält die genaue Herkunft. Diese Formen dürfen nicht stillschweigend als geprüfte Spieler-Kollision verwendet werden.

[mesh-validation.json](../analysis/collision/mesh-validation.json) enthält alle Ergebnisse. Der Audit endet wegen des bekannten defekten Meshes weiterhin mit Exitcode 1. Es wird kein erfolgreicher Vollbestandstest behauptet.

## Welche Form wählt die Engine?

`UStaticMesh::StaticConstructor` registriert UseSimpleLineCollision, UseSimpleBoxCollision und UseSimpleKarmaCollision und setzt alle drei auf True. Der Reader übernimmt diese belegten nativen Defaults und überschreibt sie mit vorhandenen Bool-Tags im Mesh. 2.444 gültige Meshes haben True/True/True; zwei haben True/False/False.

Für Linien ohne Ausdehnung gilt bei Trace-Flag 0: Actor-Zylinder hat Vorrang; sonst wird bei vorhandenem Model und UseSimpleLine=True das einfache Model gewählt. Fehlt es bei UseSimpleLine=True, liefert die Engine frei. Es gibt hier keinen automatischen Rückfall auf gespeicherte Dreiecke. Ein geladener Baum wird bei UseSimpleLine=False oder Trace-Flag 0x100 gewählt. `line_route` bildet die Auswahl mit explizitem Runtime-Ladezustand ab.

PostLoad (0x105260a0) ruft Build nur bei interner Mesh-Version unter 9 auf. Build (0x10529500) und BuildBSPCollision (0x1052b060) erzeugen Dreiecke, BSP-Knoten und komprimierte Bounds aus RawTriangles; Material EnableCollision beeinflusst die Dreiecksübernahme. Der Serializer setzt UseCollisionForBakedShadowsOnly selbst auf 1 und kann bei Version 148+ im Nicht-Editor-Laden die gespeicherten Tabellen verwerfen. Ein gespeicherter Baum beweist daher weiterhin keine verwendbare komplexe Laufzeitkollision. Die neuen Pseudocode-Dateien dokumentieren diese Pfade; der Rust-Port führt Build noch nicht aus.

## Boolesche Actor-Proben am PC

`mesh_query::player_line_clear` nutzt bei einfacher Form den vorhandenen SolidBsp-Linientest. World-Punkte werden mit inverser Rotation, komponentenweiser inverser DrawScale/DrawScale3D und PrePivot in den lokalen Raum gebracht. Die Rotation verwendet dieselbe Unreal-Quantisierung wie die Render-Transformation. Negative Skalen sind erlaubt; singuläre und nicht endliche Transformationen werden abgewiesen. Die Diagnose filtert auf bCollideActors und bBlockPlayers; bBlockActors wird separat berichtet. Zylinder, ausgewählte komplexe Tabellen und nicht verfügbare einfache Models werden ausdrücklich abgewiesen.

Der bisherige Audit [geo_01a-mesh-probes.json](../analysis/collision/geo_01a-mesh-probes.json) gilt für genaue StaticMeshActor-Exports. Der inzwischen erweiterte Importer und die gemeinsame Weltabfrage sind in [WORLD_COLLISION.md](WORLD_COLLISION.md) dokumentiert.

`rc-mesh-probe` löst diese Eigenschaften mit dem System/Properties-Katalog auf. Für jeden exakten Engine.StaticMeshActor werden vom aufgelösten Startpunkt sechs 10.000-Units-Achsenlinien geprüft. In geo_01a: 334 Actors, davon 68 nicht blockierend übersprungen, 236 mit einfacher Form und 30 mit freier Dispatch-Auswahl. 1.596 unabhängige Actor-Proben ergeben 7 blockiert und 1.589 frei; keine nicht unterstützten Actors. [geo_01a-mesh-probes.json](../analysis/collision/geo_01a-mesh-probes.json) enthält jede Probe und Herkunft. Diese Zahlen sind einzelne Actor-Prüfungen, keine zusammengeführte Weltabfrage.

45 Rust-Tests und Clippy über alle Targets bestehen. Neue Tests prüfen die Dispatch-Matrix einschließlich fehlendem Model/Trace-Flag/Zylinder, inverse Transformation mit negativem Maßstab und Pivot, transformierte feste BSP-Halbräume, Spieler-Flags sowie fehlende/komplexe/Zylinder-Formen. Bestehende Reader-Tests prüfen weiterhin Trunkierungen und ungültige Daten.

Keine neue APK und kein neuer Emulatorlauf: Die Android-App bleibt bei 0.7 und nutzt diese PC-Proben noch nicht. Die Diagnose berechnet weder FCheckResult (Zeitpunkt, Normalen, Material, Backoff) noch extents oder Bewegung. Subklassen, Legacy-Models und ungültige Ebenen bleiben offen. Die aggregierte BSP-/Mesh-Abfrage mit expliziten Ausnahmen ist jetzt am PC verfügbar; robuste Extent-Sweeps und Integration in die Android-Bewegung bleiben offen.

## Reproduzieren

```powershell
cargo run -p rc-inspect --bin rc-mesh-collision-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\StaticMeshes' analysis/collision/mesh-validation.json
cargo run -p rc-inspect --bin rc-mesh-probe -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Maps\geo_01a.ctm' analysis/collision/geo_01a-mesh-probes.json
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Lokale Belege

- [UStaticMesh::Serialize](../analysis/decompiled/staticmesh-serializer.c): engine.dll 0x10527330.
- [Tabellen und komplexe Linienprüfung](../analysis/decompiled/staticmesh-collision-arrays.c): 0x10522960, 0x10522b20, 0x1052bdd0, 0x1052db00.
- [CBox-Serializer/Expansion](../analysis/decompiled/compressed-box.c): core.dll 0x10116410 und 0x10116390.
- [Native Property-Registrierung und Defaults](../analysis/decompiled/staticmesh-static-constructor.c): 0x105253d0.
- [UStaticMesh-Konstruktor](../analysis/decompiled/staticmesh-constructor.c): 0x10527230.
- [LineCheck und PointCheck-Dispatch](../analysis/decompiled/collision-checks.c): 0x10530500 und 0x10530a60.

- [PostLoad, BuildBSPCollision und Zylinder-Auswahl](../analysis/decompiled/staticmesh-postload.c): 0x105260a0, 0x1052b060, 0x1052c030.
- [Build](../analysis/decompiled/staticmesh-build.c): 0x10529500.
