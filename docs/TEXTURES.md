# Texturen und Diffuse-Vorschau (0.4)

Viewer 0.4 rendert jetzt originale Basistexturen auf statischen Meshes.
`geo_01a` verwendet 26 verschiedene Originalbilder über 41 Materialbelegungen
auf 124.793 von 134.790 Dreiecken. Der Renderer zeigt keine vollständige
Nachbildung der ursprünglichen Shader oder Beleuchtung.

## Originalformate und Leser

Der Datenpfad wurde aus der lokalen Engine rekonstruiert:
`UTexture::Serialize` 0x10550560, `UMaterial::Serialize` 0x10451790,
Mipmap-Array 0x10550400 und Mipmap-Element 0x1054fda0.
Pseudocode liegt in `analysis/decompiled/texture-*.c`.
Nach den getaggten Properties folgen ein kompakter Mipmap-Count und je Mip:
absolutes Ende des Lazy-Bytearrays (uint32), kompakte Datenlänge, Daten,
Breite/Höhe (uint32) sowie UBits/VBits (byte). Endoffsets und Payload-Grenzen
werden geprüft. Die Formatnummer stammt aus `BitmapMaterial.Format`.

Der unabhängige Rust-Decoder enthält P8 mit Palette, RGB8/BGRA8, L8 sowie
DXT1/DXT3/DXT5. Die aktuelle Szene verwendet ausschließlich DXT:
19 verschiedene DXT1-, sechs DXT3- und eine DXT5-Textur.
BC1/BC2/BC3 entsprechen DXT1/DXT3/DXT5; die Blockinterpretation wurde mit
der [Microsoft-Dokumentation](https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-block-compression)
abgeglichen. Tests prüfen Endfarben, Farb-/Alpha-Interpolation,
DXT1-Transparenz, DXT3-Alpha und DXT5-Alpha sowie kurze Payloads.

`rc-texture-check` prüft die Mipmap-Strukturen aller direkten Engine.Texture-
Exporte in `Textures`: 2.312 Exporte, 2.265 vollständig eingelesen,
47 ausdrücklich nicht unterstützt. Die 47 liegen in `warfarefonts.utx`
mit Version 121 / Licensee 28; der derzeitige Leser unterstützt Licensee 0–1.
Der Scan ist eine Strukturprüfung, keine erfolgreiche Pixeldekodierung jedes
vorhandenen Texturformats. Die Formate 10, 12 und 14 bleiben beispielsweise
außerhalb des Pixeldecoders. Bericht: `analysis/textures/validation.json`.
Wegen der 47 nicht unterstützten Exporte liefert dieser Gesamtscan Exitcode 1.

## Materialbelegungen und Grenzen

Die Materials-Property der StaticMeshes wird jetzt als getaggtes Struct-Array
gelesen. Jede Section verwendet ihren Materialeintrag und den ersten UV-Stream.
Materialreferenzen werden über Paketgrenzen verfolgt; Zyklen und zu lange
Ketten schlagen explizit fehl.

- Texture wird direkt gelesen; Shader verwendet Diffuse.
- HsBumpDiff-/Spec-/Blend-Varianten verwenden DiffuseTexture und gespeicherte
  DiffUVScale. Für nicht gespeicherte DiffUVScale gilt vorläufig 1; vollständige
  Klassen-Defaults sind noch offen.
- Combiner liefert eine Basislagen-Vorschau: Operation 1 wählt Material2,
  Operation 7 Mask, übrige Operationen Material1. Addieren, Multiplizieren,
  Maskenüberlagerung und weitere Combine-Effekte sind noch nicht umgesetzt.
- FinalBlend wird auf Material reduziert; TexCoordSource wird nur für Kanal 0
  unterstützt. Materialketten stehen vollständig im Szenenbericht.

Fünf Referenzen von `geo_01a` bleiben ohne Textur: drei TexCoordSource-Materialien
mit anderem UV-Kanal und zwei HsFalloff-Materialien. Betroffene Flächen behalten
synthetische Farben. BSP-Oberflächen sind ebenfalls noch untexturiert.
Normalmaps, Specular, Lichtmaps, Falloff, echte Alpha-Blends, Clamp-Modi,
animierte UVs und vollständige Material-Defaults fehlen.

Die Texturen sind für diesen Software-Viewer auf maximal 64 × 64 begrenzt:
passende Originalmips werden bevorzugt; andernfalls erfolgt Nearest-Sampling.
Sampling verwendet Wrap und perspektivisch interpolierte UVs. Alpha unter 128
wird verworfen. Beleuchtung bleibt die bisherige synthetische Flächenbeleuchtung.
Die 41 Textureinträge enthalten Wiederverwendungen der 26 Originalbilder.

## Szene und Android

```powershell
cargo run --offline -p rc-inspect --bin rc-texture-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Textures' 'analysis\textures\validation.json'
cargo run --offline -p rc-inspect --bin rc-scene -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Maps\geo_01a.ctm' 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\StaticMeshes' 'analysis\scenes\geo_01a-textured'
& .\scripts\Build-Android.ps1 -Abis 'arm64-v8a','x86_64'
# Nach Start und Installation, siehe EMULATOR.md:
$adb = '.\tools\android-sdk\platform-tools\adb.exe'
& $adb -s emulator-5554 push '.\analysis\scenes\geo_01a-textured\world.rcscene' /sdcard/Download/geo_01a-textured.rcscene
```

`rc-scene` sucht Texturpakete im `Textures`-Verzeichnis neben `StaticMeshes`.
`scene.json` dokumentiert jede Materialkette, UV-Skalierung und Fehlstelle.
Der 9.825.520 Byte große Snapshot enthält Originalgeometrie und abgeleitete
Texturpixel; die APK enthält selbst keine Spielassets. Der Import wird weiter
am PC vorbereitet. Android lädt die Szene und rendert ausschließlich in Rust.

Der neue Leser unterstützt sowohl RCSC-Version 1 als auch Version 2. V2 besitzt einen
16-Byte-Header (Magic, Version, Dreieckzahl, Textureinträge), je Dreieck 68 Byte
(v1-Geometrie plus sechs UV-float32 und int32-Texturindex, -1 ohne Textur), danach
je Textur Breite/Höhe (uint32) und ARGB8888-Pixel. Grenzen, Referenzen und
endliche UVs werden vor Verwendung geprüft.

## Prüfung am 6. Oktober 2026

19 Rust-Tests und Clippy über alle Targets bestehen. ARM64-/x86-64-APK-Build,
Signatur und 16-KiB-zipalign-Prüfung erfolgreich. Im API-35-x86-64-Emulator:
Start, texturierte Darstellung, Touch-Drehung, alte V1-Szene, Original-CTM,
Fehleranzeige für einen verkürzten Snapshot und erneutes Laden erfolgreich.
67.355 Pixel ändern sich im Renderbereich nach dem Ziehen; Crash-Puffer leer.
Belege: `analysis/emulator/texture-scene*` und `texture-crash-log.txt`.
Physischer ARM64-Test und Gameplay stehen weiterhin aus.

Nächste Schritte: Klassendefaults und vollständige Level-Instanzlisten,
weitere UV-Kanäle/Materialeffekte, bessere Kamera und Kollisions-/Bewegungskern.

## Stand 0.5

Zoom, freie Diagnosekamera, Nah-Ebenen-Clipping und Kamera-Reset sind ergänzt und im API-35-Emulator auf der texturierten Originalszene geprüft. 20 Rust-Tests/Clippy; ARM64-/x86-64-Build, Signatur und zipalign bestehen. Bewegung bleibt ohne Kollision und Gameplay. Siehe [CAMERA.md](CAMERA.md).
