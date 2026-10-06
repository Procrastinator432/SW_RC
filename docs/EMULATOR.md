# Android-Emulatortest

Am 2026-10-06 wurde der vorhandene IntelliJ-Emulator unter
`C:\Users\brand\AppData\Local\Android\Sdk\emulator\emulator.exe` verwendet.
Ein virtuelles Gerät und Systemabbild waren dort noch nicht vorhanden.
Mit Zustimmung des Nutzers wurde im Projekt `RC_Test_API35` angelegt:
Android 15 / API 35, Google APIs x86_64, Pixel 2, 1080 × 1920,
Windows Hypervisor Platform, Software-Grafikausgabe.
Systemabbild: `tools/android-sdk/system-images/android-35/google_apis/x86_64`.
Gerätedaten: `tools/avd`. Diese großen lokalen Dateien gehören nicht ins Git.

## Start und Installation

PowerShell im Projektordner:

```powershell
rustup target add x86_64-linux-android
& .\scripts\Build-Android.ps1 -Abis 'arm64-v8a','x86_64'
& .\scripts\Start-TestEmulator.ps1 -Visible
$adb = '.\tools\android-sdk\platform-tools\adb.exe'
& $adb -s emulator-5554 wait-for-device
& $adb -s emulator-5554 shell getprop sys.boot_completed
# Erst fortfahren, wenn sys.boot_completed = 1.
& $adb -s emulator-5554 install -r '.\android\app\build\outputs\apk\debug\app-debug.apk'
& $adb -s emulator-5554 push 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Maps\entry.ctm' /sdcard/Download/entry.ctm
& $adb -s emulator-5554 push 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Maps\geo_01a.ctm' /sdcard/Download/geo_01a.ctm
& $adb -s emulator-5554 shell am start -W -n org.rcport.diagnostic/.MainActivity
```

Ohne `-Visible` startet das Testgerät im Hintergrund. Zum Beenden:
`& $adb -s emulator-5554 emu kill`.
Ohne `-Abis` baut `Build-Android.ps1` weiterhin ausschließlich ARM64.

## Prüfergebnisse

- APK-Installation und Kaltstart erfolgreich; Android meldet `primaryCpuAbi=x86_64`.
- Originalkarte `entry.ctm`: SWRC 157/1, 115 Namen, 16 Imports, 29 Exports;
  BSP-Raum wird nativ dargestellt.
- Touch-Ziehen dreht die Kamera. Vorher/Nachher-Screenshots unterscheiden
  sich um 95.791 Pixel im Renderbereich.
- Originalkarte `geo_01a.ctm`: SWRC 159/1, 4.891 Namen, 421 Imports,
  3.704 Exports; BSP wird dargestellt.
- Ungültige Datei (README als `invalid.ctm`): verständliche Fehleranzeige
  `not a little-endian Unreal package`; danach funktioniert Kartenladen wieder.
- Android-Crash-Puffer nach dem Test leer.

Belege: `analysis/emulator/`: Screenshots, UI-XML und `crash-log.txt`.
Die BSP-Bodenfläche von `geo_01a` ist groß, deshalb erscheint die zentrale
Geometrie klein. Statische Meshes, Texturen und Gameplay fehlen weiterhin.
Dieser Test bestätigt den nativen x86-64-Viewer. Ausführung der ARM64-Version
auf einem physischen Android-Gerät und eine spielbare Engine sind noch offen.

## Szenenviewer 0.3

Die aktuelle APK trägt versionCode 3 / 0.3.0-scene-viewer. Zusätzlich wurde geo_01a.rcscene mit 334 Mesh-Instanzen und 134.790 Dreiecken geprüft: Laden, Rendering, Touch-Drehung und Kartenwechsel erfolgreich; Crash-Puffer leer. Anleitung und Belege in [STATIC_MESHES.md](STATIC_MESHES.md). Das Testgerät wurde danach beendet.

## Texturviewer 0.4

Aktuelle APK: versionCode 4 / 0.4.0-texture-viewer. geo_01a-textured.rcscene mit 41 Textureinträgen (26 verschiedene Originalbilder) geprüft. Darstellung, Touch-Drehung, ältere V1-Szene und CTM, verkürzte Datei und Wiederherstellung funktionieren; Crash-Puffer leer. Details: [TEXTURES.md](TEXTURES.md).

## Stand 0.5

Zoom, freie Diagnosekamera, Nah-Ebenen-Clipping und Kamera-Reset sind ergänzt und im API-35-Emulator auf der texturierten Originalszene geprüft. 20 Rust-Tests/Clippy; ARM64-/x86-64-Build, Signatur und zipalign bestehen. Bewegung bleibt ohne Kollision und Gameplay. Siehe [CAMERA.md](CAMERA.md).

## Stand 0.6

Original-Startpunkte aus CTM und Snapshot-Version 3 lassen sich anwählen; keine Pawn-Augenhöhe oder Spawn-Logik. 23 Tests/Clippy, ARM64-/x86-64-Build, Signatur/zipalign und Emulatorprüfung einschließlich exakter Rückkehr/Reset und Fehler-Recovery bestehen. Details und Testskripte: [LEVELS_AND_COLLISION.md](LEVELS_AND_COLLISION.md).

## Stand 0.7

Originale Model-Tails bis RootOutside/Linked werden jetzt gelesen; Punkte und Linien ohne Ausdehnung können im Welt-BSP geprüft werden. 9.123 Modelle in 79 Karten ohne Fehler geprüft. Klassenvererbung für 1.302 Klassen katalogisiert; DefaultProperties und vollständige Spieler-/Mesh-Kollision bleiben offen. 27 Tests/Clippy, ARM64-/x86-64-Build, Signatur/zipalign und Emulator-Diagnose bestehen. Siehe [BSP_SOLID.md](BSP_SOLID.md).


## Default-Anchors im bestehenden Viewer 0.7

PC-Default-Auflösung erzeugt entry-defaults.rcscene mit einem Startpunkt, den der direkte CTM-Import wegen fehlender Rotation noch auslässt. `scripts/Test-DefaultsEmulator.ps1` prüft beide Importwege, Anwählen, Bewegung, Rückkehr, Reset und erneutes Laden. `scripts/Compare-DefaultsFrames.py` bestätigt sichtbare Bewegung und exakte Rückkehr/Reset/Recovery; Crash-Puffer leer. Belege: analysis/emulator/defaults-*. Die APK bleibt Version 0.7, Snapshot v3; System-/Properties-Pakete werden noch nicht auf Android direkt gelesen.
