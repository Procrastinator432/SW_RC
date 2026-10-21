# Build und geprüfter Quellstand

Zentraler Einstiegspunkt unter PowerShell 7 ist im Projektverzeichnis:

```powershell
.\build.ps1
```

Standard ist `-Target Check`: Format, sämtliche Debug- und Release-Bibliothekstests, Doc-Tests, Clippy für alle Targets mit Warnungen als Fehler, Native-DLL, Inspector und Java-Prüfungen für Kampagnenwege, Checkpoints und Szenenleser. Die Java-Prüfung nutzt einen synthetischen 64-MiB-Inhalt bei 96 MiB Heap; sie benötigt keine Spieldaten. Ein Quellvertrag verhindert neue implizite Archiv-/Maschinenpfade in Szenen-Buildern. Fehler brechen den Lauf ab und ergeben einen fehlgeschlagenen Bericht.

Ergebnisse: `build/validation.json`, einzelne Protokolle unter `build/logs`. `-OutputDirectory` erlaubt getrennte Prüfläufe. Der Bericht nennt Commit, Dirty-Status, Toolversionen und jeden Prüfschritt. Abhängigkeiten werden mit `--locked` und standardmäßig offline aufgelöst; `-Online` erlaubt die normale Cargo-/Gradle-Auflösung. Das installiert noch keine Android-Toolchain.

`-Target Check` enthält auch die Regeln für transparente Framepasses. `-Target Android` prüft nach dem APK-Build zusätzlich den tatsächlichen Android-Frameparser auf der JVM mit dem lokalen Android35-JAR (`gpu-frame-contract`); dabei werden neue Effektpakete, Legacyframes und ungültige Daten geprüft. Einzelaufruf: `scripts/Test-GpuFrame.ps1`. Dies erzeugt keinen GLES-Kontext und ersetzt keinen Geräte-Bildvergleich.

## Voraussetzungen

- Rust/Cargo, hier geprüft mit Rust 1.92.0 (`stable-x86_64-pc-windows-msvc`), sowie die MSVC-Linkerwerkzeuge. `Cargo.lock` ist versioniert. Andere Toolversionen werden im Bericht erfasst; eine rustup-Toolchain wird nicht heimlich nachinstalliert.
- JDK 21 über `JAVA_HOME` oder `tools/jdk21/<jdk>/`. Alternativer gemeinsamer Werkzeugordner: `-ToolsDirectory D:/Pfad/tools`.
- Für Android zusätzlich das vorhandene SDK unter `<tools>/android-sdk`, Plattform/Build-Tools 35, NDK 27.2.12479018, beide Rust-Android-Targets und Gradle 8.11.1. Die vorhandenen Einrichtungsskripte bleiben getrennt vom Build.
- Tools, SDKs, erzeugte Dateien, private Asset-Eingaben und lokale IDE-Einstellungen sind nicht Teil des Git-Quellstands. `.idea` bleibt lokal erhalten.

## Android

Der Androidbuilder legt temporäre Compilerdateien unter `build/android-compiler-temp` an und stellt `TEMP`/`TMP` anschließend wieder her. Das vermeidet NDK-Umbenennungsfehler in eingeschränkten Windows-Tempverzeichnissen. Gradle benötigt weiterhin eine Verbindung zu seinem lokalen Buildprozess; ein Sandbox-Verbindungsfehler ist kein Spielfehler.

```powershell
.\build.ps1 -Target Android
# Einzelne Architektur:
.\build.ps1 -Target Android -Abis arm64-v8a
```

Standardmäßig entstehen ARM64 und x86_64. Die APK liegt unter `build/artifacts/republic-commando-<versionName>.apk`, ihr SHA-256 im Bericht. `Android` allein führt die Quellprüfungen nicht erneut aus. Die erzeugte APK ist weiterhin eine Debug-APK; Store-Signierung und Veröffentlichung gehören nicht zu diesem Schritt.

Seit 0.49.0 erzeugt `-Target Package -SceneInputs FILE` das einmalig importierbare `mission1-scenes.rcpack.zip`. `-Target All` erzeugt es nach der Szenenpipeline automatisch aus deren aufgelösten Eingaben. [Kampagnenbedienung und Paketformat](CAMPAIGN_MENU.md).

## Explizite Szeneneingaben

Die 14 zuvor an historische Mission-Verzeichnisse gekoppelten Builder verwenden jetzt benannte Eingaben aus `RC_BUILD_INPUTS`. Kein Szenen-Builder greift implizit auf `analysis/mission…` oder einen fest eingebauten Steam-Pfad zurück. Die zentrale Szenenpipeline setzt diese Umgebung und stellt sie anschließend wieder her.

`config/scene-inputs.example.json` enthält den SHA-256-gebundenen Eingabestand und portable relative Dateinamen. Dazu gehören vorbereitete Welt-/Mesh-/Animationsdaten, die bestehende Karte A sowie die drei aufgezeichneten Eingabeläufe. Die originalen oder daraus erzeugten großen Inhalte werden lokal bereitgestellt, beispielsweise unter `local-assets/mission1/assets`; sie werden nicht in Git eingecheckt. Relative Pfade gelten immer relativ zur Manifestdatei. Ohne diese Dateien schlägt der Szenenpfad vor der Kompilierung mit dem fehlenden Schlüssel fehl; der normale Quellbuild bleibt unabhängig davon.

Beim Konsolidieren wurden die vorhandenen Eingaben einmal ausdrücklich aus dem bisherigen Arbeitsbestand in diesen eigenständigen lokalen Eingabesatz kopiert und gehasht. Danach ist das Archiv für den neuen Build unnötig. Das ist noch kein vollständiger Extraktionslauf allein aus einer frischen Spielinstallation: Vorbereitete Originaldaten und Karte A sind deklarierte Inputs. Die alten Analysen, Prüfskripte und eingefrorenen Lieferungen bleiben als historische Nachweise erhalten; sie gehören nicht zum zentralen Buildpfad.

```powershell
.\build.ps1 -Target Scenes `
  -SceneInputs local-assets/mission1/scene-inputs.json `
  -GameData 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData'

# Ein Schritt mit bereits vorbereiteten Eingaben:
.\build.ps1 -Target Scenes -SceneStage map3-elevator `
  -SceneInputs local-assets/mission1/scene-inputs.json -GameData 'D:/Pfad/GameData'

# Alle Quellprüfungen, Android und die Missionspipeline:
.\build.ps1 -Target All `
  -SceneInputs local-assets/mission1/scene-inputs.json -GameData 'D:/Pfad/GameData'
```

Die unterstützten Stufen stehen in `config/scene-builds.json`. `mission` baut die A-Korrekturen einschließlich Bacta/Präsentation sowie B, C und den Fabrikeinstieg in deklarierter Reihenfolge; erzeugte Zwischenstände ersetzen die jeweiligen Eingabeschlüssel. Jeder externe benötigte Input wird vorab per SHA-256 geprüft. Die Pipeline validiert anschließend den vollständigen A/B/C→Fabrik-Lauf mit 25 Checkpoints, exaktem Zustand/Grafik, Fortlauf, atomarer Fehlerabweisung und Neustart. Zusätzlich prüft `rc-presentation-check` den regulären A-Eingabelauf, speichert Spieleransichten und führt den tatsächlichen A→B-Wechsel aus. Der zusätzliche `rc-weapon-animation-check` prüft originale Waffenclips, getrennte Kanäle, Ruhefrist und beschädigte Checkpoints in ausdrücklich kontrollierten Darstellungsfixtures; diese ersetzen die regulären Missionsläufe nicht. Einzelstufen führen diese Gesamtläufe nicht automatisch aus. Ergebnisse und aufgelöste Eingaben stehen unter `build/scenes`.

Für einzelne ältere Tools bleiben ihre positionalen Argumente bestehen. Zusätzliche Eingaben kommen aus dem Manifest; `RC_BUILD_OUTPUT` bestimmt den Ausgabeordner und `RC_GAME_DATA` die Originalinstallation. Der eigenständige `rc-throw-scene-build` verwendet ebenfalls benannte Eingaben, gehört aber nicht zur aktuellen B/C/Fabrik-Rezeptfolge.

## Ursache und Regression des Stackfehlers

Der normale Debug-Test `bundle_roundtrip_retains_collision_spawn_encounter_and_rejects_corruption` brach unter Windows mit `0xc00000fd / STATUS_STACK_OVERFLOW` ab. `bundle::decode` enthielt alle 13 Wrapper-/Formatzweige in derselben rekursiven Funktion. In unoptimiertem Code reservierte jeder Aufruf die großen temporären Decoder-/Game-Werte aller Zweige; der standardmäßige Test-Thread-Stack reichte nicht.

Der Dispatcher ruft jetzt getrennte Formatfunktionen auf. Nicht ausgewählte Formate reservieren dadurch keine temporären Werte im aktuellen Frame. Größenlimits, erlaubte Wrapper-Reihenfolge, Fehler und Snapshot-ABI bleiben bestehen. Der Zentralbuild setzt keine höhere Stackgröße, sondern entfernt ein möglicherweise geerbtes `RUST_MIN_STACK` nur für seinen eigenen Lauf und stellt es anschließend wieder her. Ein eigener Regressionstest verlangt den Roundtrip zusätzlich auf explizit 2 MiB Stack. Debug und Release werden beide geprüft.

## Versionsverwaltung

Der bisherige Missions-/Checkpoint-Stand bis 0.47.0 ist im separaten Basis-Commit `ea9d212` gesichert. Der folgende Konsolidierungscommit enthält Decoderfix, Buildpfad, Manifeste, Zeilenenden und Dokumentation. Der geprüfte Abschluss wird lokal als `v0.48.0` annotiert. Ein Tag bedeutet keine Veröffentlichung; es wird nichts gepusht.

Die zentralen Quellprüfungen werden zusätzlich aus einem frischen Checkout ohne `analysis`, `local-assets` und lokale IDE-Dateien ausgeführt. Die große Asset-Extraktion und physische ARM64-Geräteabnahme bleiben getrennte Aufgaben.
