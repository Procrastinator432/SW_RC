# Animierte Originalmodelle in geo_01a (Meilenstein 2)

Android **0.10.0-animated-scene**, versionCode **12**, 10. Oktober 2026.
Die neue Szene enthält drei Geonosian Warriors sowie die DC-17m mit Originalhänden.
Die vorbereitete Begegnung und ihre Kampfregeln bleiben Portierungsregeln; die Originalmission ist damit noch nicht rekonstruiert.

## Ausprobieren

1. `android/app/build/outputs/apk/debug/app-debug.apk` installieren bzw. aktualisieren.
2. **`analysis/milestone2/geo_01a-animated.rcplay`** auf das Gerät kopieren.
3. **Spielabschnitt geo_01a öffnen → Szene öffnen** und diese Datei auswählen.
4. Drei Geonosianer ausschalten und die grüne Exfil-Markierung erreichen. Feuer, Laden, Sprung, Pause und Neustart funktionieren wie bisher. **GPU / CPU** wechselt die Ausgabe.

Die bisherige Datei `analysis/playable/geo_01a.rcplay` bleibt die unveränderte Referenz mit Platzhaltermodellen und wird weiterhin geladen. Originalassets werden lokal aus der vorhandenen Installation vorbereitet und sind nicht in der APK enthalten.

## Originaldaten und Laufzeit

| Bestandteil | Übernommene Daten |
|---|---|
| Gegner | `geonosian.GeonosianWarrior`, 56 Knochen, erster gespeicherter LOD, 2.991 Dreiecke je Instanz, zwei Materialplätze |
| Gegnerclips | `GeonosianWarriorSet`: `ActionBreathe`, `RunForward`, `Fire`, `hit`, `Death` |
| Waffe | `hudarms.HudArmsDC17m`, 29 Knochen, erster LOD, 3.508 Dreiecke, sieben Materialplätze |
| Waffenclips | `HudArmsDC17mSet`: `ActionBreathe`, `Walk`, `Fire`, `Reload` |
| Texturen | Neun Belegungen mit Basis-Diffuse, jeweils maximal 256 × 256; ursprüngliche UVs und aufgelöste Ausgangstransformationen |

`rc-play-assets` erfasst Kartenactors und Klassen-/Instanzreferenzen. Die Karte referenziert unter anderem `GeonosianWarrior`; dessen Modell wurde deshalb gegenüber dem zuerst untersuchten Drone-Modell gewählt. Die vollständige Bestandsaufnahme steht in `analysis/milestone2/assets.json`.

Die Quaternion-Rechnung verwendet die vorhandene portable Näherung; native RSQRTSS-/x87-Bitgleichheit wird damit nicht behauptet.

Die Runtime wertet komprimierte Originaltracks über den vorhandenen Channel-Host aus, baut die Knochenhierarchie einschließlich Move-Bone-Regel auf und führt die rekonstruierte Palette, Skin-Command-Strecke und Abschnitts-/Indexauswahl aus. Inverse Referenzmatrizen sind unveränderliche Asset-Caches. Channel-Zeit ist Teil des festen Simulationsschritts und des Reset-Zustands. Pose-Scratch, Matrizen und Skin-Ausgaben sind pro Auswertung getrennt; Rendern verändert die Simulation nicht.

Die eigenen Adapter wählen Clips anhand der bestehenden Begegnung: Annäherung → Lauf, Schuss → Angriff, Schaden → Treffer, Tod → Todesclip mit gehaltenem Schlussbild. Die Waffe folgt der Kamera; ihr Tiefenpuffer wird nach der Welt-/Gegnerdarstellung geleert, bevor HUD und Touch-Bedienung folgen. CPU und GPU verwenden dieselbe Geometrie, Kamera und Materialbelegung. JNI behält ein abgefragtes Grafikpaket bis zur Kopie und verwirft es bei Simulation bzw. Neustart; dadurch wird die Skinningarbeit nicht für Größenabfrage und Kopie doppelt ausgeführt.

Das Format **RCPLAY02** enthält begrenzte visuelle Metadaten und ein unverändertes **RCPLAY01**-Basispaket. Rekursive Wrapper werden abgewiesen. Knochenzuordnung, Clipnamen, Speicherbudgets, Geometrie, Texturbelegung und Pose-Abhängigkeiten werden vor Veröffentlichung geprüft. **RCFRA002** überträgt dynamische Materialbatches und den Waffenpass; die bisherige Szene verwendet weiter **RCFRA001**.

Ein im Android-Durchlauf gefundener Eingabefehler ist behoben: `ACTION_UP` und `ACTION_POINTER_UP` übernehmen jetzt die letzte Blickposition, bevor die Pointerrolle freigegeben wird. Der separate Eingabeprüfer testet dies mit Gesten, die ihre gesamte Bewegung erst beim Loslassen liefern.

## Bewusste Grenzen

- Die drei Standorte, Gegnerbewegung, Treffer-AABB (Halbmaße 22/22/60), Schaden, Feuerrate und Nachladen stammen weiterhin aus dem Prototyp. Das Gegnermodell wird gleichmäßig auf 120 Einheiten Ruhehöhe skaliert und an den Standboden gesetzt; ausladende Flügel gehören nicht zum Treffer-AABB. Das ist kein originaler Actor-/Karma-Treffermodus und kein Originalspawn.
- Die Waffe verwendet ihr Originalmodell im Kameraraum, aber keine vollständig rekonstruierte originale First-Person-FOV-/Inventarstrecke. Die Nachladeanimation wird an die bisherige Nachladedauer von 1,2 Sekunden angepasst.
- Basis-Diffuse ersetzt den vollständigen Materialeffekt. Bei Shadern ohne Opacity wird der Texturalphakanal nicht als Transparenz verwendet; er enthält bei diesen Assets Effektmasken. Die Flügel verwenden denselben Diffuse-/Opacity-Eingang und einen Alpha-Test. Exakte ursprüngliche Blend-, Beleuchtungs- und Shaderparität bleibt offen.
- Die Texturverschiebungen der Waffenanzeige werden zum Ausgangszeitpunkt aufgelöst. Die Ziffern am Waffenmodell sind noch nicht mit der Munition synchronisiert; maßgeblich ist die Munitionsangabe im HUD. Animations-Notifies werden gespeichert, ihre Script-/Audioereignisse noch nicht ausgeführt. Diese Verbindungen gehören zu Meilenstein 3.
- Der Nutzer hat den flüssigeren GPU-Pfad der Vorgängerversion auf seinem Gerät bestätigt. Eine Prüfung dieser neuen Version und eine quantitative ARM64-Dauermessung sind getrennte, noch offene Nachweise.

## Erzeugen und prüfen

```powershell
cargo run --release --locked --offline -p rc-inspect --bin rc-play-animated -- "D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData" analysis/playable/geo_01a.rcplay analysis/milestone2
cargo run --release --locked --offline -p rc-inspect --bin rc-play-animation-check -- analysis/milestone2/geo_01a-animated.rcplay analysis/milestone2
cargo run --release --locked --offline -p rc-inspect --bin rc-play-check -- analysis/milestone2/geo_01a-animated.rcplay analysis/milestone2/desktop
```

Nachweise: `asset-receipt.json`, `mesh-bounds.json`, `animation-validation.json`, Desktop-Bilder, `graphics/validation.json`, `android/validation.json`, `cpu-android/validation.json`, `input/validation.json`, `abi-validation.json`, `legacy-replay-check.json` und `validation-manifest.json` unter `analysis/milestone2`.
Der Manifest-Generator bindet Quellen, Originalpakete, APK, Szene und Prüfergebnisse über SHA-256 zusammen. Emulator-Messwerte beschreiben CPU-Arbeit und Einreichungsintervalle, keine GPU-Timer oder garantierte Display-Bildrate.

Als Nächstes folgt Meilenstein 3: den belegten DC-17m-/Geonosian-Kampfpfad einschließlich Munitionsanzeige, Animationsereignissen und Originalgeräuschen integrieren.

## Prüfergebnis dieses Stands

937 Workspace-Tests, Clippy, Format- und Diff-Prüfung bestehen. Desktop-Durchlauf, neun Originalclips mit wechselnder und wiederholbarer Pose, feste Animationszeit bei 30/120 Eingabebildern, getrennte Trefferzustände, gehaltenes Todesbild, Kamera-Anbindung, Pause, Reset und beschädigte Zuordnung sind geprüft. Die alte Szene besteht weiterhin 1.002 Replay-Kommandos und alle fünf Referenzbilder exakt.

Im API-35-x86-64-Emulator bestehen GPU- und CPU-Durchlauf jeweils bis Sieg und Niederlage, Nachladen, Neustart sowie Dateifehler/Wiederherstellung. Vier gleichzeitige Pointer, finale Blickbewegung beim Loslassen, CANCEL, HOME und Dateiauswahl-Abbruch bestehen. Der tatsächlich neu erzeugte Grafik-Kontext liefert dasselbe Bild und dieselbe eingefrorene Pose. CPU/GPU-Startbilder: mittlerer Kanalfehler 0,560/255; 99,151 % der Kanäle weichen höchstens zwei Werte ab. Drei erneute Ladevorgänge bleiben innerhalb des begrenzten Speicherproben-Budgets; dies ist kein Langzeit-Leckfreiheitstest.

| Emulator-Messgröße | GPU Median / p95 | CPU Median / p95 |
|---|---|---|
| Host-Einreichungsintervall | 18,00 / 32,02 ms | 66,82 / 86,90 ms |
| JNI-Worker | 8,95 / 24,37 ms | 55,80 / 74,27 ms |
| Actor-Geometrie | 1,47 / 4,70 ms | 0,94 / 1,55 ms |

GPU: 1.131 Hostframes, 1.077 erfasste GL-Einreichungen; GL-CPU-Arbeit median 10,10 ms / p95 24,47 ms. CPU: 321 Hostframes; Szenenrasterung median 46,87 ms / p95 59,07 ms. Im GPU-Lauf wurde keine Simulationszeit verworfen, im CPU-Lauf maximal 0,0333 Sekunden durch die bestehende Aufholgrenze. Die Messreihen sind normale UI-Durchläufe mit unterschiedlichen Bildfolgen auf dem Emulator-Softwaretreiber; sie sind keine gepaarte Hardware-Benchmarkmessung und keine Display-FPS-Angabe.

Die APK enthält ARM64 und x86-64, beide JNI-/Rust-Bibliotheken besitzen 16-KiB-ELF-Ausrichtung. APK-Signatur und 16-KiB-Zip-Ausrichtung bestehen. Die neue Szene hat 19.022.005 Bytes. APK-SHA-256: `b8e49c7abd2de2b95cf4cf724c7fdc0c9e9f69f30f126cd044f53b673be5270c`. Szenen-SHA-256: `65a9befc485bcdc3058573b5a579586b6cb49ecb2ee02c72cbf82a4e6f860abb`.


Nachtrag 10. Oktober 2026: Der Nutzer bestätigt die Funktion dieser animierten Version auf seinem Gerät. Der neue M3-Kampfabschnitt ist separat in [COMBAT_SCENE.md](COMBAT_SCENE.md) beschrieben; diese M2-Szene bleibt unverändert.
