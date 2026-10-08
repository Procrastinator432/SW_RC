# SinTime, TanTime und XYCircle

Stand 2026-10-08. Drei weitere Konstantenfälle des Original-D3DDrv-Dispatchers rekonstruiert und in die persistenten 8-/96-Register-Bänke eingebunden. Grundlage: `analysis/decompiled/shader-constants.asm`; Sprungtabelle und Konstanten wurden direkt im Original-PE geprüft.

1. **SinTime, Fall 10 / 1001dff9.** Die Engine-Zeit wird wie zuvor mit signiertem Rest modulo 120 auf f32 gespeichert. Frequenz ist das erste Wort der Materialkonstante. Sowohl +0 als auch -0 werden durch 1 ersetzt. Die Originalfolge lädt die Frequenz mit FLD, multipliziert ohne vorherigen f32-Store mit der gewickelten Zeit und führt FSIN aus. Alle vier Registerkomponenten erhalten denselben auf f32 gespeicherten Wert. Die übrigen drei Materialwörter werden ignoriert.
2. **TanTime, Fall 11 / 1001e04a.** Gleiche Zeit-/Frequenzregeln, anschließend FPTAN. Dessen zusätzliche Stapel-Eins wird verworfen; der Tangenswert wird viermal ausgegeben. Keine Begrenzung auf 0..1 und kein Ausweichen an Polstellen.
3. **XYCircle, Fall 13 / 1001e3b1.** Verwendet ausschließlich die gewickelte Zeit. Ausgabe `[500*cos(t),500*sin(t),0,1]`. Originalradius 500 an Adresse 10072688, Bits `43fa0000`. Die Skalierung erfolgt im x87-Register vor dem f32-Store. Alle Materialwörter bleiben unbenutzt, auch eine dort gelieferte Frequenz.

Die portable Rust-Auswertung nutzt f64-Transzendentale und speichert anschließend auf f32. Das Produkt zweier f32-Eingaben passt exakt in f64; die Transzendentalfunktionen sind damit jedoch keine vollständige Emulation der x87-Implementierung. SinTime/TanTime bleiben auf endliche Eingaben und Argumentbetrag <=8192 begrenzt, wie die bestehende CosTime-Diagnose. Nichtendliches Tangens-Ergebnis ist ein sicherer Hostfehler. XYCircle verlangt endliche Zeit und ist durch den 120-Sekunden-Rest begrenzt. Diese Schutzgrenzen sind keine ursprünglichen Engine-Prüfungen.

Ein fehlerhafter Registerwert wird nicht teilweise geschrieben. Bereits erfolgte Writes und der ermittelte Bankzähler bleiben erhalten. Die drei neuen Fälle fordern keine Inversion und keinen Zufallswert an. Vorherige Registerinhalte in unbenutzten Slots bleiben unverändert; Zeit-/Szenenbeschaffung bleibt Hostaufgabe.

`rc-wave-constant-check` prüft 2048 synthetische Zeit-/Frequenzkombinationen durch den Dispatcher, abwechselnd in Pixel- und Vertex-Bänken. Enthalten sind signierte Null, negative Zeiten/Frequenzen, große endliche Zeiten, Wrap-Grenzen und Werte nahe Tangens-Polstellen. Die separate `Probe-WaveConstants.rs` führt FPREM, FSIN, FPTAN und FCOS mit der ursprünglichen Multiplikations-/Store-Reihenfolge aus, ohne Spiel-DLLs zu laden. Sie setzt den x87-Control-Word auf 037f und stellt den vorherigen Wert wieder her; die tatsächliche Control-Word-Konfiguration des laufenden Originalspiels wird nicht behauptet.

`Record-WaveConstants.py` kontrolliert unabhängig die Fixtures, Original-Sprungtabelle, Konstanten, mathematischen Ergebnisse und vollständigen Bänke. Alle 10240 x87-Ausgabewörter stimmen für diese Proben mit der portablen Auswertung überein; 6144 Registerausgaben und 851968 Vorher-/Nachher-Bankwörter geprüft. Debug/Release-Berichte und Eingabedateien sind bytegleich. Diese beobachtete Gleichheit ist keine universelle x87-/Android-Präzisionszusage, insbesondere nahe Polstellen.

Vier neue Grenzfall-/Dispatchtests; **579 Workspace-Tests**, Clippy mit `-D warnings`, Format- und Diffprüfung bestanden. Berichte: `analysis/reports/wave-constants{,-release,-validation}.json`, Eingaben `wave-constants{,-release}.input.bin`, Gegenprobe `wave-constants.x87.bin`, Testlog `wave-constants-tests.log`.

Reproduktion im Projektordner auf x86_64:

```powershell
cargo run -p rc-inspect --bin rc-wave-constant-check -- analysis/reports/wave-constants.json analysis/reports/wave-constants.input.bin
cargo run --release -p rc-inspect --bin rc-wave-constant-check -- analysis/reports/wave-constants-release.json analysis/reports/wave-constants-release.input.bin
rustc scripts/Probe-WaveConstants.rs -O -o target/wave-constant-probe.exe
& target/wave-constant-probe.exe analysis/reports/wave-constants.input.bin analysis/reports/wave-constants.x87.bin
cargo test --workspace *> analysis/reports/wave-constants-tests.log
python scripts/Record-WaveConstants.py
```

Offene Konstantenfälle: DrawScale3D 30 und Rotator 34. Live-Szenenbeschaffung, vollständige Renderzustandsanbindung und spielbare Engine bleiben offen. Frühere Mesh-Vorschauen behalten ihre Fixtures; allgemeine Diffuse-Abdeckung bleibt 271/289. Android-Prüfung zum Schluss.
