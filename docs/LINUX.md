# Entwicklung unter Linux

Die bestehende Rust-/PowerShell-Buildstrecke unterstützt jetzt Linux x86_64.
Werkzeuge, Python-Umgebung und Downloadcaches liegen im ignorierten Ordner
`tools/`. Es werden keine Systempakete oder Desktop-Einstellungen geändert.
`rustup` ergänzt die beiden Android-Targets zur vorhandenen Rust-Toolchain.

## Geprüfter lokaler Stand: 24.10.2026

Der zentrale Offline-Check besteht mit je 1.201 Debug-/Release-Bibliothekstests,
Clippy, Doc-Tests, Materialimport, Java und Buildvertrag. Der frische DLL-Vergleich
bestätigt alle 820 Kartenabfragen und die synthetischen Audio-Geometriefälle.
Ghidra hat `engine.dll` erfolgreich automatisch analysiert; die drei Audioexports
und der PowerShell-Adresswrapper sind geprüft.

Die APK für ARM64 und x86_64 ist einschließlich JVM-Frameparser offline gebaut.
Rust-/JNI-Bau und Gradle verwenden ausdrücklich NDK 27.2.12479018.
[Gesammelter Nachweis](../build/linux-setup/verification-summary.json).
Die vorbereiteten Missionseingaben und eine neue Android-Laufzeit-/Handyabnahme
stehen weiterhin aus; die spielbare Referenz bleibt 0.60.0.

## Einrichtung

Voraussetzungen: Rust/Cargo 1.92.0 einschließlich rustfmt/Clippy, Python 3 mit
venv/ensurepip, curl, ein C-Linker und Internet für die erste Einrichtung.

```bash
# Rust-Abhängigkeiten, Python-Analysepakete, JDK 21, PowerShell und Ghidra:
python3 scripts/Setup-Linux.py

# Zusätzlich SDK 35, Build-Tools 35.0.0, NDK 27.2.12479018 und Gradle 8.11.1:
python3 scripts/Setup-Linux.py --android

source scripts/env-linux.sh
python scripts/Check-LinuxEnvironment.py
```

Das Android-Setup fragt nach den Google-SDK-Lizenzen. Nur mit ausdrücklicher
Zustimmung des Nutzers kann `--accept-sdk-licenses` zur automatischen Annahme
ergänzt werden. Downloads der Hauptwerkzeuge sind mit SHA-256 gebunden und
fortsetzbar; nach einem Verbindungsabbruch denselben Setupbefehl erneut starten.
Python verwendet die Versionen aus
`config/analysis-requirements.txt`; Cargo bleibt an `Cargo.lock` gebunden.

Gebundene Hauptwerkzeuge: Ghidra 12.1.4, Temurin JDK 21.0.13+9 und PowerShell
7.4.13. Die Umgebung gilt für die aktuelle Shell und ihre Kindprozesse. Der
Build verwendet standardmäßig zwei Cargo-Jobs, damit Compiler und Analyse auf
diesem Rechner gleichzeitig laufen können; `CARGO_BUILD_JOBS` kann überschrieben
werden. PowerShell-Caches und Java-/Android-/Gradle-Werkzeuge bleiben lokal.

## Build und Originalvergleich

```bash
./build.sh -Target Check -OutputDirectory build/linux-check

# Beim ersten Android-Bau müssen auch die Gradle-Abhängigkeiten geladen werden:
./build.sh -Target Android -Online -OutputDirectory build/linux-android
# Danach ist der normale Offline-Bau möglich.

source scripts/env-linux.sh
# Bei einer anderen Installation diesen Pfad ausdrücklich überschreiben:
export RC_GAME_DATA="$HOME/.local/share/Steam/steamapps/common/Star Wars Republic Commando/GameData"
cargo run --release --locked --offline -p rc-inspect --bin rc-audio-geometry-check -- \
  "$RC_GAME_DATA" build/linux-setup/audio-geometry/maps
python scripts/Test-AudioGeometryOriginal.py \
  --game-data "$RC_GAME_DATA" \
  --output build/linux-setup/audio-geometry/original-replay.json \
  --verify-fixture fixtures/audio-geometry-native.json \
  --models build/linux-setup/audio-geometry/maps
```

`Check` führt dieselben Quellprüfungen wie unter Windows aus, einschließlich
Debug/Release, Doc-Tests, Clippy, Java und Buildvertrag. `Android` baut beide
ABIs und prüft den JVM-Frameparser; dafür ist noch kein Szenenpaket erforderlich.
Ein erfolgreicher APK-Bau ersetzt keine Ausführung auf einem Android-Gerät.
Der Umgebungsbericht unter `build/linux-setup/environment.json` unterscheidet
Werkzeuge, Android-Targets und vollständige Missionseingaben.

Für ein Update einer bereits installierten Debug-APK wird derselbe
Signierschlüssel benötigt. Die frühere Windows-Datei `debug.keystore` ist hier
nicht vorhanden; beim ersten lokalen APK-Bau entsteht ein neuer Schlüssel.
Bestehende Handyinstallationen und Spielstände deshalb beim späteren Geräte-
Update ausdrücklich berücksichtigen, bevor eine Neuinstallation erwogen wird.

## Ghidra ohne zusätzliche MCP-Verbindung

Die vorhandenen Java-Exportscripts laufen direkt im Headless-Modus:

```bash
source scripts/env-linux.sh
pwsh -NoProfile -File scripts/Analyze-Binary.ps1 \
  -GameData "$RC_GAME_DATA" -Binary System/engine.dll
pwsh -NoProfile -File scripts/Analyze-Addresses.ps1 \
  -Binary engine.dll -Addresses 10446180 -Output audio-radius.c
```

Die erste Analyse erzeugt `analysis/ghidra-projects/RepublicCommando` und die
Funktionsübersicht erneut. Die bisherige Importfunktion überschreibt dabei eine
gleichnamige Programmanalyse; vorhandene manuelle Annotationen zuvor sichern.
Eine Ghidra-MCP-Verbindung ist eine zusätzliche Bedienmöglichkeit und für diese
Projektbefehle nicht erforderlich.

## Fehlende historische Dateien

Der Linux-Checkout enthält derzeit keine alten `analysis/`, `build/` oder
`local-assets/mission1`-Bestände. Die versionierten Audio-Fixtures und Originaldaten
reichen für Rust-Entwicklung, neue Dekompilierungen und frische DLL-Vergleiche.

Der vollständige Missionsbau benötigt weiterhin die vorbereiteten, gehashten
Eingaben aus `config/scene-inputs.example.json`. Dazu gehören Szenen, Mesh- und
Animationsinventare sowie die drei aufgezeichneten Spielereingaben. Diese Daten
müssen aus dem bisherigen Entwicklungsbestand übernommen oder gesondert neu
erzeugt werden. Die Installation der Werkzeuge rekonstruiert sie nicht.

Die fehlenden Eingaben bleiben ein ausdrücklicher Fehler vor dem Szenenbau;
Hashprüfungen und Missionsabnahmen werden nicht abgeschwächt. Alte Windows-
Releaseberichte im Vault bleiben historische Nachweise. Neue lokale Ergebnisse
stehen unter `build/linux-*` und belegen nur den jeweils tatsächlich ausgeführten
Prüfumfang.

## Herstellerquellen

- [Ghidra 12.1.4](https://github.com/NationalSecurityAgency/ghidra/releases/tag/Ghidra_12.1.4_build)
- [Temurin JDK 21](https://adoptium.net/temurin/releases/?version=21)
- [PowerShell 7.4.13](https://github.com/PowerShell/PowerShell/releases/tag/v7.4.13)
- [Android SDK und Prüfsummen](https://developer.android.com/studio)
- [sdkmanager](https://developer.android.com/tools/sdkmanager)
- [Gradle 8.11.1](https://docs.gradle.org/8.11.1/release-notes.html)
