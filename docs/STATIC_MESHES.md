# StaticMeshes und Szenenvorschau (0.3)

Der Rust-Leser dekodiert Render-Streams aus originalen `.usx`-Paketen:
Sections, Positionen, Normalen, UV-Streams und beide 16-Bit-Indexpuffer.
Unterstützter, gegen die Installation geprüfter Bereich: Paketversion 133–159,
Licensee 0–1. Kollision, Lazy-Editor-Triangles und Beleuchtungstails bleiben
uninterpretiert. Textur-/Materialdaten werden noch nicht gerendert.

## Belegte Rekonstruktion

Ausgangspunkt ist die lokale `engine.dll`, nicht ein generisches UE2-Layout:

- `UStaticMesh::Serialize` 0x10527330: Reihenfolge, Bounds und Versionsfelder.
- Section-Serializer 0x1032d8d0: 32-Bit-Flag und fünf 16-Bit-Felder.
- Vertex-Serializer 0x10525150: Position und Normale, jeweils drei float32.
- UV-Serializer 0x10334270: zwei float32 pro Vertex.
- Index-Serializer 0x10373d90: kompakter Count und uint16-Array.
- `AActor::LocalToWorld` 0x103196b0: PrePivot abziehen, DrawScale/DrawScale3D
  anwenden, mit Unreal-Rotator drehen, Location addieren. Rotatoren verwenden
  65.536 Einheiten pro Umdrehung und die originale 14-Bit-Winkelquantisierung.

Pseudocode: `analysis/decompiled/staticmesh-layout.c`, `staticmesh-streams.c`
und `actor-transform.c`. Die Feldbenennung wurde zusätzlich mit
[UEViewer UnMesh2.h](https://github.com/gildor2/UEViewer/blob/master/Unreal/UnrealMesh/UnMesh2.h)
abgeglichen; die Rust-Implementierung wurde aus den lokalen Serialisierern
und Daten erstellt.

## Validierung und verbleibender Datenfehler

`rc-mesh-check` prüft 2.447 StaticMesh-Exporte im Verzeichnis `StaticMeshes`.
2.446 werden gelesen, mit insgesamt 1.568.954 Dreiecken. Ein Export wird
ausdrücklich abgewiesen: `markericons.usx`, `SetTrap.TrapXSpotIcon`,
nicht endlicher float32 bei Payload-Byte 890. Bericht:
`analysis/meshes/validation.json`. Das Rohobjekt samt Herkunft liegt unter
`analysis/objects/markericons__SetTrap.TrapXSpotIcon.bin`.
Deshalb beendet sich die vollständige Mesh-Prüfung derzeit mit Exitcode 1;
dieser Fehler wird nicht als erfolgreicher Gesamtscan ausgegeben.

15 Rust-Tests und Clippy über alle Targets bestehen. Tests umfassen die
Versionsfelder, jede Verkürzung eines vollständigen synthetischen Meshes,
Indexgrenzen, Actor-Pivot/Rotation und ungültige Szenensnapshots.

## Szene aus Originaldaten erzeugen

```powershell
cargo run --offline -p rc-inspect --bin rc-mesh-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\StaticMeshes' 'analysis\meshes\validation.json'
cargo run --offline -p rc-inspect --bin rc-scene -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Maps\geo_01a.ctm' 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\StaticMeshes' 'analysis\scenes\geo_01a'
```

`rc-scene` liest explizite StaticMesh-Referenzen, löst externe Paketpfade
ohne Beachtung der Groß-/Kleinschreibung auf und fügt transformierte Meshes
zum Welt-BSP hinzu. Bisher werden nur Instanzen der exakten Klasse
`Engine.StaticMeshActor` eingebunden. Fehlende Transformationsproperties
verwenden Location/Rotation/PrePivot 0 und DrawScale/DrawScale3D 1.
Abgeleitete Klassen bleiben ausgespart, bis ihre Defaults zuverlässig
dekodiert sind. `bHidden` wird beachtet; Level-Referenzen und Sichtbarkeit
sind noch nicht vollständig rekonstruiert.

`geo_01a` ergibt 334 Mesh-Instanzen, 21 wegen ausstehender Subclass-Defaults
ausgesparte Actors und 134.790 Dreiecke einschließlich 557 sichtbarer
BSP-Dreiecke. `scene.json` nennt jede Instanz, Transformation und Auslassung.
`world.ppm`/`world.png` zeigen die Geometrie mit synthetischen Farben.

## Android-Datenpfad

Die Android-App 0.3 öffnet weiterhin `.ctm` direkt als BSP-Vorschau.
Zusätzlich liest sie `world.rcscene`: ein lokales Diagnoseformat mit
Header `RCSC`, little-endian Version 1, Triangle-Count und je Dreieck
neun float32-Koordinaten plus uint32-Farb-/Sectionkennung (40 Byte).
Counts, Dateilänge und endliche Koordinaten werden geprüft.
Die 5.391.612 Byte große Datei enthält aus den Originalassets abgeleitete
Geometrie. Die APK selbst enthält keine Original-Spielassets.

Die Szene wird derzeit auf dem PC vorbereitet. Die Android-App löst
originale Mesh-Pakete noch nicht selbstständig im Dateisystem auf.

```powershell
& .\scripts\Build-Android.ps1 -Abis 'arm64-v8a','x86_64'
# Nach Start des Testgeräts und APK-Installation, siehe EMULATOR.md:
$adb = '.\tools\android-sdk\platform-tools\adb.exe'
& $adb -s emulator-5554 push '.\analysis\scenes\geo_01a\world.rcscene' /sdcard/Download/geo_01a.rcscene
```

API-35-x86-64-Emulator: Installation, Szenenladen, native Darstellung,
Touch-Drehen und Wechsel Szene → Originalkarte → Szene erfolgreich.
67.384 Pixel im Renderbereich ändern sich nach dem Ziehen; Crash-Puffer leer.
Screenshots und UI-XML: `analysis/emulator/mesh-scene*`.
ARM64-Build erfolgreich; Ausführung auf physischem ARM64-Gerät steht aus.

Die Szene ist weiterhin untexturiert und ohne Gameplay. Nächste Schritte:
Klassendefaults und vollständige Level-Instanzlisten, Material-/Texturleser,
direkter Assetimport auf Android und ein Renderer für First-Person-Bewegung.


## Weiterentwicklung 0.4

Die aktuelle APK unterstützt originale Basistexturen und weiterhin alte V1-Szenen. Der heutige rc-scene-Aufruf ergänzt automatisch Basistexturen aus dem benachbarten Textures-Verzeichnis und schreibt dafür RCSC-Version 2. Prüfstand, Reproduktion und Einschränkungen: [TEXTURES.md](TEXTURES.md).


Kollisionspräfixe werden inzwischen separat gelesen und im Gesamtbestand geprüft. Die sichtbaren Dreiecke werden weiterhin nicht als fertige Spieler-Kollision behandelt. Einfache Model-Formen, Baumdaten, native Flags und Grenzen: [MESH_COLLISION.md](MESH_COLLISION.md).
