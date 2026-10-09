# Originale Speicherstruktur → Szenensnapshot → Materialkonstanten

Stand 2026-10-09. Drei zusammenhängende Aufgaben abgeschlossen: sicherer Speicherabbild-Leser, Übernahme der originalen Kamera-/Licht-/Szenenfelder und Verbindung mit den serialisierten DynamicHologram-Konstanten sowie dem Original-Vertexprogramm. Grundlage sind die Befehle in `analysis/decompiled/shader-constants.asm` und die Original-PE-Importtabelle.

## 1. Sicherer Leser

`rc_package::shader_snapshot::Memory` liest ausschließlich gelieferte Speicherregionen mit originalen 32-Bit-Adressen. Die Rust-Adressen werden niemals als Prozesszeiger dereferenziert. Regionen werden sortiert; Nulladressen, leere Regionen, Überlappungen und Überlauf über den x86-Adressraum werden abgewiesen. Ein Feld darf über unmittelbar benachbarte Regionen reichen. Fehlende Bytes oder Lücken ergeben Fehler.

`Globals` enthält die Adressen der **aufgelösten globalen Variablen**, keine IAT-Slots oder DLL-RVAs. Die Originalimporte wurden direkt geprüft:

| Original-IAT | Variable | Gelesener Wert |
|---|---|---|
| 1006f304 | GEngineTime | rohes f32-Wort |
| 1006f0c8 | GIsEditor | 32-Bit-Integer, ungleich null |
| 1006f354 | GCubemapManager | Manager-Zeiger oder null |

Das Interface unterstützt versetzte Speicherabbilder. Es implementiert keinen Prozess-Dumper und keinen Live-Speicherzugriff.

## 2. Originalfelder übernehmen

Ausgangspunkt ist die gelieferte Adresse des D3D-Rendererobjekts. Seine Felder +9c0c und +8 liefern Renderzustand und Ansichtskontext. Der Ansichtskontext heißt im Diagnosecode `viewport`; daraus wird keine vollständige ursprüngliche C++-Typdefinition abgeleitet.

| Quelle | Inhalt |
|---|---|
| Renderzustand +2c/+6c/+ac | rohe ObjectToWorld-/WorldToCamera-/Projection-Matrizen |
| Ansichtskontext +184 → Kamera +54 | Laufzeit-CameraToWorld |
| Kamera +194/+198/+19c | separate Laufzeit-Augenposition |
| Ansichtskontext +30 → Objekt +138/+13c/+140 | Editor-Augenposition |
| Renderzustand +148 | Ambient-BGRA-Wort |
| Renderzustand +2f0/+2f4 | Nebelstart/-ende |
| GCubemapManager → +2c → Actor +1b4/+1b8/+1bc | roher DrawScale3D-Override |
| Derselbe Actor +64 | Alpha-Gate: Actor vorhanden und Flag 0x2000 nicht gesetzt |
| Renderzustand +328..334 | vier ursprüngliche Lichtquellenzeiger |

Eine vorhandene Lichtquelle liefert ihren Actor-Zeiger aus +0. Ein Null-Actor bleibt als vorhandene, aber ungültige Quelle im Snapshot erhalten; seine weiteren Felder werden nicht gelesen. Gültige Quellen liefern Actor-Art +2a und Cone +39 sowie Quellenfarbe +8, Position +18, Richtung +24, Reichweiten +30/+34, Flags +38/+3c und Helligkeit +48. Diese Werte gehen unverändert in die bestehende Lichttabelle mit ihren nativen Quellplatz-Lücken.

Im Editor wird der Laufzeitkamera-Pfad nicht gelesen. Eine fehlende Laufzeitkamera bleibt `None`, damit die bestehenden Konstantenfälle ihren unterschiedlichen Null-/Inverse-Regeln folgen können. Fehlender Cubemap-Manager oder fehlender Cubemap-Actor bedeutet keinen Skalierungs-Override und ein ausgeschaltetes Alpha-Gate. Rohe Matrix-/Farb-/Skalierungswörter werden nicht normalisiert.

`capture` erstellt einen vollständigen, **eifrigen** Host-Snapshot. Dafür müssen alle Felder der ausgewählten Zweige im Abbild vorhanden sein. Das ist strenger als die ursprünglichen bedarfsabhängigen Reads einzelner Konstanten. Ein Erfassungsfehler liefert keinen partiellen Host und verändert keinen Konstantenpuffer. Die RSQRT-Rechenpolitik wird ausdrücklich vom Aufrufer geliefert.

## 3. Materialintegration und Gegenprüfung

`Generate-ShaderSnapshots.py` erzeugt 256 synthetische, versetzte, dünn besetzte Speicherabbilder. Alle 256 Kombinationen aus vier Lichtquellenzuständen sind enthalten: null, Quelle mit Null-Actor und zwei gültige Varianten. Hinzu kommen Editor-/Laufzeitwege, fehlende Laufzeitkameras, Cubemap-Nullpfade und unterschiedliche Actor-Flags. Acht weitere Abbilder enthalten fehlende Bytes, Nullzeiger, Überlauf oder ungültige Objektverweise.

`rc-shader-snapshot-check` liest daraus Host-Snapshots und lädt alle zwanzig VS- und beide PS-Bindungen von `HardwareShaders.Hologram.DynamicHologram` direkt aus den Originalpaketen. Die 96-/8-Register-Bänke und der globale Flicker-Zustand bleiben über die 256 Updates erhalten. Eine zweite Pipeline mit direkt gelieferten erwarteten Hostfeldern muss exakt dieselben Ergebnisse liefern. Die vorhandene Core-Inversionsrekonstruktion beantwortet Matrixanfragen. c17 beginnt ausdrücklich mit .25; alle anderen anfänglichen Register sind ausdrücklich null. Die portable RSQRT-Politik ist ebenfalls ausdrücklich gewählt.

`Record-ShaderSnapshots.py` liest die Speicherregionen unabhängig als Adress-Wörterbuch, kontrolliert die Originalimporte, Hostfelder, aufgelösten Adressen und Materialbindungen und berechnet Puffer, Inversenanfragen, Flicker und Vertex-Ausgaben mit den bestehenden unabhängigen Originalbefehls-Interpretern nach.

- 256 gültige Snapshots, davon 128 Editorwege und zwölf Laufzeitwege mit Nullkamera.
- 256 Quellen mit Null-Actor ohne Zugriff auf weitere Quellen-/Actorfelder.
- 212992 Vorher-/Nachher-Registerwörter, 206 inverse Anfragen und 429 RNG-Werte geprüft.
- 128 erfolgreiche Vertex-Ausgaben; 128 erwartete Dispatcherfehler durch native Lichttabellen-Lücken. Vorherige Writes und Flicker-Änderungen bleiben dabei korrekt erhalten.
- Acht erwartete Erfassungsfehler; Debug-/Release-Berichte bytegleich.
- Zwei neue Leser-Grenzfalltests; **587 Workspace-Tests**, Clippy mit `-D warnings`, Format- und Diffprüfung bestanden.

Berichte: `analysis/reports/shader-snapshots{,-release,-validation}.json`, Eingaben `shader-snapshots.input.json`, Testlog `shader-snapshots-tests.log`.

```powershell
python scripts/Generate-ShaderSnapshots.py
cargo run -p rc-inspect --bin rc-shader-snapshot-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/shader-snapshots.input.json analysis/reports/shader-snapshots.json
cargo run --release -p rc-inspect --bin rc-shader-snapshot-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/shader-snapshots.input.json analysis/reports/shader-snapshots-release.json
cargo test --workspace *> analysis/reports/shader-snapshots-tests.log
python scripts/Record-ShaderSnapshots.py
```

Das sind synthetische Original-Layout-Abbilder, keine aus einem laufenden Spiel gewonnenen Aufnahmen. Live-Szenenerzeugung, echte Registerhistorie, vollständige Wrapper-/Textur-/Renderzustandsanbindung und spielbare Engine bleiben offen. Es entsteht keine neue gerenderte Mesh-Vorschau; ältere Vorschauen behalten ihre Fixtures. Allgemeine Diffuse-Abdeckung bleibt 271/289. Android-Prüfung weiterhin zum Schluss.

Korrektur 2026-10-09: Die hier aufgezeichnete VS-vor-PS-Reihenfolge mit getrennten persistenten Banken ist eine Diagnose-Fixture. Der Originalaufruf nutzt PS-vor-VS und einen gemeinsamen Scratchpuffer mit getrennten Geraeteuploads; siehe [HardwareShader-Uebergabe](HARDWARE_HANDOFF.md). Der Originaltreiber importiert Direct3D 8.
