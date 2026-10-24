# Originale Audio-Geometrie und Radiusfortlauf

**Quellfortsetzung 24.10.2026:** [Zonentabellen und Laufzeitanbindung](AUDIO_GEOMETRY_RUNTIME.md) sind im ausgewählten Quellenhost umgesetzt; vollständige Missionsregressionen und Androidabnahme stehen noch aus. Der folgende Text dokumentiert den früheren Grundlagenabschluss.

Stand: 23.10.2026 · Quellbaustein nach 0.60.0. Die spielbare Lieferung bleibt [0.60.0](AMBIENT_LIFECYCLE.md). Dieser Baustein ist noch nicht mit der Szenenlaufzeit oder Android-Audio verbunden.

## Fünf umgesetzte Punkte

1. **Hörbarkeitspfad:** `IsAudibleAt` unterscheidet BSP (kein Actor oder Modus 0/2), ungeprüfte Hörbarkeit (andere Werte) und allgemeinen Trace (Modus 3). Ein Mover einschließlich Unterklassen erzwingt den Bypass. Der allgemeine Trace verwendet Maske `0x286`, Nullausdehnung und den Quellactor; seine eigentliche Kollision bleibt caller-owned.
2. **BSP-Hörbarkeit:** Der originale `FastLineCheck` mit seinen Blatt-/Outside-Regeln ist als eigener Rust-Kern umgesetzt. Die SSE-Summenreihenfolge unterscheidet sich für beide Endpunkte; der Linienzweig prüft das Vorzeichenbit. Die Abfrage verwendet die originalen BSP-Knoten, nicht sichtbare Dreiecke oder den Physik-Ersatzhost.
3. **Punktregionen und Import:** `PointRegion` liefert Blatt, Zone und ZoneInfo-Objektreferenz. Punkte auf einer Ebene wählen den Frontzweig; ohne Zonendaten wird Zone 0 verwendet, eine leere ZoneInfo-Referenz fällt auf den übergebenen LevelInfo zurück. Der neue Model-Leser erhält die bisher übersprungenen Seitenzonen und Blattindizes sowie die Zonen-Objektreferenzen. Vier echte Weltmodelle sind gelesen und gegen das Original geprüft.
4. **Radiusfaktor und Fortlauf:** `CalculateRadiusMultiplier` verwendet `0.85^(Distanzbyte²)` aus einer 64×64-Tabelle. Alle 256 Ergebnisse wurden mit der mitgelieferten Original-CRT ausgeführt; eine Ergebnistabelle bewahrt auch Subnormale plattformübergreifend. `update_radii` verbindet einen bereits aufgelösten Distanzwert mit dem bestehenden Original-Radiuskern, einschließlich Faktor 0,35 bei Verdeckung und unveränderter ursprünglicher Zeitrechnung.
5. **Exakte Speicherung und Prüfbefehl:** `SavedRadii` speichert die Float-/Zeitbits verlustfrei und weist nicht endliche Zustände ab. Fünf zusätzliche Bibliothekstests laufen im zentralen Debug-/Release-Prüfpfad. Originalausführung, echte Kartenabfragen und Abschlusshashes werden gesondert gesichert.

## Originalnachweis

Verwendete PC-Dateien:

- `System/engine.dll`, SHA-256 `f8b3fc9d62414828a1c31d8b5f3b2bcb951f7c0f142fdf2a9846c98d7b5d35a5`
- `System/msvcr71.dll`, SHA-256 `8094af5ee310714caebccaeee7769ffb08048503ba478b879edfef5f1a24fefe`

| Original | Adresse | Ausgeführter Umfang |
|---|---|---|
| `ULevel::IsAudibleAt` | `104468b0` | Ganze Funktion; kontrollierter allgemeiner Trace-Callback, echter BSP-Pfad. |
| `UModel::FastLineCheck` | `10552a60`, intern `105520a0` | Ganze Funktionen einschließlich segmentweiser BSP-Durchquerung. |
| `UModel::PointRegion` | `105538e0` | Ganze Funktion; zurückgegebene Zone, Blatt und Actor geprüft. |
| `ULevel::CalculateRadiusMultiplier` | `10446180` | Ganze Funktion und originale `_CIpow` aus der mitgelieferten CRT. |
| `FAbstractSource::UpdateRadii` | `103d267f` ff. | Die bereits belegten Rechenregionen nach der Geometrieabfrage, Profiling ausgelassen. |

512 synthetische BSP-Fälle prüfen jeweils Linie und Punktregion, darunter leere Modelle, beide Outside-Zustände, unterschiedliche Knotenflags und Ebenengrenzen. Dazu kommen 1.024 Modus-/Mover-/Tracefälle, alle 256 Distanzbytewerte sowie 512 aufeinander aufbauende Radiusupdates in 16 Zustandsfolgen. Die Radiusfolgen vergleichen nach jedem Speichern/Laden alle ursprünglichen Ergebnisbits. Eine ursprüngliche Callback-Neubelegung im Emulator wurde durch getrennte unveränderliche Callback-Adressen ersetzt, um dessen Codecache nicht mit einer neuen Callback-Ausgabe zu verwechseln.

Der Kartenprüfer liest GEO_01A/B/C und GEO_03A: 249/386/188/455 BSP-Knoten mit 5/14/3/12 Zonen. Je 205 Abfragen an Spielerstarts und originalen Kartenpunkten prüfen klare und blockierte Linien, Blatt-/Zonenwahl, exakten Serialisierungsfortlauf und abgewiesene Dateikürzung. Alle 820 Linien- und 820 Regionsresultate stimmen mit der ausgeführten Original-DLL überein. Private Kartendumps und Originaldekompilate bleiben im ausgeblendeten Buildbereich; die eingecheckbare Fixture enthält nur synthetische Eingaben, Ergebnisse und Quellhashes.

## Reproduzieren

```powershell
./build.ps1 -Target Check -OutputDirectory build/audio-geometry/final-check
cargo run --release -p rc-inspect --bin rc-audio-geometry-check -- `
  'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' `
  build/audio-geometry/maps
```

Optionaler frischer Originalvergleich mit `unicorn==2.1.4` und `pefile==2024.8.26`:

```powershell
python -X utf8 scripts/Test-AudioGeometryOriginal.py `
  --game-data 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' `
  --output build/audio-geometry/original-replay.json `
  --verify-fixture fixtures/audio-geometry-native.json `
  --models build/audio-geometry/maps
```

## Grenzen und nächster Abschnitt

Die Laufzeit-Erzeugung der 64×64-Zonendistanztabelle ist noch nicht rekonstruiert. Die serialisierten Zonen-Objektreferenzen sind **keine** solche Distanztabelle. Auch ursprüngliche Actor-Occlusion-Modi, gecachte Actor-/Listenerregionen und die Integration in Aufnahme/Freigabe und Wiedergaberadius müssen noch an den Szenenhost angeschlossen werden. Der allgemeine Tracepfad wird bisher ausdrücklich als externes Ergebnis geliefert; dynamische Mover-/Mesh-Abfragen werden dadurch nicht als vollständig portiert bezeichnet.

`SavedRadii` ist ein vorbereiteter Speichervertrag für diesen neuen Baustein; bisherige Spiel-Checkpoints werden nicht verändert. Geometrieprüfung und Radiusupdate sind lesende beziehungsweise reine Berechnungen ohne Audioereignisse oder Zufallskonsum. Validierung begrenzt endliche Ebenen/Positionen, Kinder-/Zonenindizes, Zyklen und Abfragebudget; diese Schutzgrenzen sind Regeln des Ports.

Dieser Abschnitt verändert weder APK noch Szenenpaket. Für einen neuen hörbaren Stand folgen Tabellenrekonstruktion, Szenenimport, persistente Laufzeitanbindung, normale Missions-/Legacyprüfungen und zuletzt Android. Freie physische Hör-/Handyabnahme und vollständige Mixer-/EAX-/HRTF-/Dopplerparität bleiben offen.

## Abschlussbelege

Der zentrale Check besteht mit je **1.201 Debug-/Release-Bibliothekstests**, Clippy aller Targets, Materialimport-, Dokumentations- und Java-Prüfungen. Die abschließende frische Originalausführung stimmt mit der eingefrorenen synthetischen Fixture und allen vier Kartendumps überein. [Prüfbericht](../build/audio-geometry/verification-summary.json) und [Quellzuordnung](../build/audio-geometry/source-and-verification.json) sichern den lokalen Arbeitsstand. Die APK-/Pakethashes früherer Lieferungen bleiben unverändert. Für diesen noch nicht angeschlossenen Baustein gibt es keine neue Android- oder Hörabnahme.
