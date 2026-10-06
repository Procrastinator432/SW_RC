# Kartenleser und BSP-Viewer (0.2)

## Verifiziert gegen die Installation

`rc-map-check` prüft alle 79 `.ctm`-Dateien: 205.548 UObject-Property-Listen, 9.123 BSP-Modelle und 185.835 Dreiecke über sämtliche Modelle. Keine Lesefehler. Das bedeutet nicht, dass sämtliche nativen Objektdaten vollständig dekodiert sind: unbekannte Property-Payloads werden als Raw markiert; Modellteile nach dem Vertexpool bleiben uninterpretierte Tails.

`geo_01a.ctm`, Weltmodell `Model1`: 362 Punkte, 249 BSP-Knoten, 131 Oberflächen, 633 Dreiecke vor Sichtbarkeitsfilterung. Separate Brush-/Mover-Modelle werden exportiert, aber im Viewer nicht ohne ihre Actor-Transformationen in die Welt eingefügt.

## Datenpfad

1. Paket-Imports/Exports und Namensreferenzen lesen.
2. Bei `RF_HasStack` (`0x02000000`) zwei kompakte Referenzen, 8 Byte ProbeMask, einen **16-Bit** LatentAction-Wert und bei Node != 0 einen kompakten Codeoffset lesen.
3. UE2-Tags lesen: Name, Info-Byte, optionaler Struct-Name, Payload-Größe, optionaler Arrayindex. Bool-Werte stecken im Info-Byte und besitzen keinen nachfolgenden Payload. Primitive Werte, Objekt-/Namensreferenzen, Vector und Rotator werden dekodiert; andere Strukturen/Arrays bleiben Raw.
4. `UPrimitive`: Min/Max-Vector, ein Gültigkeitsbyte, Sphere-Center und Radius.
5. `UModel`: Vectorarray, Pointarray, BSP-Nodes, Oberflächen, Vertexpool. Alle Arrays beginnen mit kompakten Counts. Knoten referenzieren einen Bereich des Vertexpools; Vertices referenzieren Punkte. Oberflächen tragen Materialreferenzen.
6. Konvexe BSP-Knotenpolygone als Triangle-Fan triangulieren. Referenzen und Dateigrenzen prüfen.

Belege liegen in `analysis/decompiled/serializers.c`, `model-layout.c`, `geometry-layout.c` und `bsp-layout.c`. Relevante Originalfunktionen: `UModel::Serialize` 0x10461c90, `UPrimitive::Serialize` 0x104994d0, Node-Serializer 0x1045e000, Surf-Serializer 0x1045ccc0 und Vertex-Serializer 0x1045cb60.

## Renderer und Grenzen

`rc-render` ist ein portabler Rust-Software-Rasterizer mit Perspektive, Tiefenpuffer und synthetischen Oberflächenfarben. Unreal-Z bleibt die Hochachse. Native Rendering-Pixel werden über die C-ABI/JNI als ARGB8888 an Android Bitmap übergeben. Die Kamera umkreist die Modellgrenzen; Touch-Ziehen verändert Yaw/Pitch.

Der Viewer unterstützt ausdrücklich das Weltmodell `Model1`. `PF_Invisible` und `PF_FakeBackdrop` (Maskierung 0x81) werden übersprungen. Er zeigt die vorhandene BSP-Geometrie, **keine vollständige Spielszene**: Im direkten CTM-Modus fehlen StaticMesh-Instanzen, Terrain, Texturen, Sky, Beleuchtung, Transparenz, Animation, Visibility/Zone-Culling und Kollisionslogik. Die vollständige Dekodierung von Level-Referenzen und Klassendefaults steht noch aus. Seit 0.5 sind Nah-Ebenen-Clipping, Zoom und freie Kamera ergänzt. Seit 0.7 sind Welt-BSP-Punkt-/Linienprüfungen als Diagnose vorhanden; die Bewegung bleibt ohne Spieler-Kollision.

Opaque Scene-Handles werden in einer Rust-Registry verwaltet. Freigegebene Handles schlagen sauber fehl; wiederholtes Freigeben ist erlaubt. Android lädt und rendert auf einem einzelnen Worker; Zerstören der Activity gibt das Modell wieder frei.

## Reproduzieren

```powershell
cargo run --offline -p rc-inspect --bin rc-map-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Maps' 'analysis\maps\validation.json'
cargo run --offline -p rc-inspect --bin rc-map -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Maps\geo_01a.ctm' 'analysis\maps\geo_01a'
& .\scripts\Build-Android.ps1
```

`rc-map` erzeugt JSON, OBJ und eine PPM-Vorschau aus Originaldaten. Die PNG-Vorschau ist eine Formatkonvertierung dieser PPM. Zum Testen auf Android die APK installieren und eine selbst kopierte Originalkarte über den System-Dateidialog wählen. Es werden keine Spielassets in die APK eingebettet. Android-Laufzeit und Touch-Verhalten sind inzwischen im API-35-Emulator geprüft; siehe [EMULATOR.md](EMULATOR.md). Viewer 0.3 lädt zusätzlich exportierte Szenen mit StaticMeshes; siehe [STATIC_MESHES.md](STATIC_MESHES.md).



## Stand 0.7

Originale Model-Tails bis RootOutside/Linked werden jetzt gelesen; Punkte und Linien ohne Ausdehnung können im Welt-BSP geprüft werden. 9.123 Modelle in 79 Karten ohne Fehler geprüft. Klassenvererbung für 1.302 Klassen katalogisiert; DefaultProperties und vollständige Spieler-/Mesh-Kollision bleiben offen. 27 Tests/Clippy, ARM64-/x86-64-Build, Signatur/zipalign und Emulator-Diagnose bestehen. Siehe [BSP_SOLID.md](BSP_SOLID.md).
