# DrawScale3D, Rotator und gemeinsame Dispatcher-Prüfung

Stand 2026-10-08. Drei Aufgaben abgeschlossen: die letzten beiden regulären Konstantenfälle 30/34 und eine unabhängige gemeinsame Prüfung aller Typen 0..34. Damit sind die regulären Typen im begrenzten Hostmodell implementiert; die Beschaffung echter Szenenwerte und die native Spielengine bleiben eigenständige offene Arbeit.

**DrawScale3D, Fall 30 / 1001dcb0.** Der Original-Prolog liest die importierte globale Variable `GCubemapManager` (IAT 1006f354). Falls der Manager existiert, liefert sein Feld +2c den Actor-Override. Bei vorhandenem Actor werden die drei rohen Wörter +1b4/+1b8/+1bc kopiert; W ist rohe Null. Negative Null, NaN-Payloads und andere rohe Werte bleiben dabei erhalten; keine Rechenpolitik oder Matrix wird benötigt. Das ist kein allgemeiner Viewport-Actor und kein automatisch aus der Szene ermittelter Objektmaßstab.

Ohne diesen Actor werden die Längen der ersten drei ObjectToWorld-Zeilen ermittelt, jeweils nur XYZ. Bei Zeile 0 lautet die Quadratsumme `f32(f32(y*y+z*z)+x*x)`, bei Zeile 1/2 `f32(f32(x*x+y*y)+z*z)`. Anschließend originale RSQRTSS-Saat plus Newton-Verfeinerung, mit separater f32-Rundung jedes MULSS/SUBSS. Bei Quadratsumme null wird die Verfeinerung trotzdem ausgeführt und das Ergebnis anschließend auf rohe Null maskiert. W ist in diesem Pfad 1. Translation und vierte Spalte werden ignoriert.

Die Saat bleibt eine ausdrückliche Host-Rechenpolitik, wie bei Lichtreichweiten. Diagnose nutzt die tatsächliche x86_64-RSQRTSS-Instruktion. Der portable Helfer nutzt `1/sqrt(q)` ohne Bitgleichheitsbehauptung; seine Ausgaben werden separat geprüft. Nichtendliche relevante Matrixwerte/Summen/Ergebnisse und fehlende Saatpolitik sind sichere Hostfehler. Ein Register wird bei solchen Fehlern nicht teilweise geschrieben, frühere Writes bleiben erhalten; das unterscheidet sich ausdrücklich von möglichen partiellen Originalwrites.

**Rotator, Fall 34 / 1001e09c.** Zwei Register: `[cos(a),-sin(a),0,0]` und `[sin(a),cos(a),0,0]`. `a=f32(frequency*f32(engine_time%120))`. Das Original verwendet MULSS und einen f32-Store vor FCOS/FSIN. Anders als SinTime/CosTime/TanTime bleibt eine Nullfrequenz null; signierte Null beeinflusst die Sinusvorzeichen. Der zweite Bindingplatz wird übersprungen. Der Bankzähler berücksichtigt die Zwei-Register-Spanne. Sichere Upload-Grenzen prüfen beide Register, auch wenn ein bereits gecachter Zähler nur bis zum ersten reicht.

Rust verwendet portable f64-Transzendentale auf dem f32-Argument, anschließend f32-Stores. Endliche Eingaben und Argumentbetrag <=8192 sind der begrenzte Diagnosevertrag, keine nativen Engine-Prüfungen. `Probe-RotatorConstants.rs` prüft separat die ursprüngliche SSE-/x87-Reihenfolge mit MXCSR 1f80 und x87-Control-Word 037f und stellt beide Einstellungen wieder her. Spiel-DLLs werden dabei nicht ausgeführt. Die tatsächlichen Einstellungen eines laufenden Originalspiels und universelle Android-/x87-Präzision werden nicht behauptet.

**Gemeinsame Prüfung.** `rc-complete-constant-check` erzeugt 128 synthetische Szenen mit je zwei Updates derselben 96-Register-Bank: alle Typen vorwärts und rückwärts gepackt. Absichtlich ungültige Fortsetzungs-Bindings prüfen das Überspringen von Matrix-/Rotator-Folgeplätzen. Die Diagnose setzt den Zähler vor dem zweiten Layout ausdrücklich zurück, erhält jedoch Registerwörter und Flicker-Zustand. Ohne solche Invalidierung bliebe der ursprüngliche Zähler gecacht.

Der Host liefert Matrix, Kamera, Licht-, Zeit-, Cubemap-Actor- und RNG-Snapshots ausdrücklich. Derselbe Cubemap-Actor steuert über seine Flags +64 auch das Licht-Alpha-Gate; dessen bisherige Bezeichnung als Viewport-Actor wurde anhand des Originalimports korrigiert. Inverse Anfragen werden mit der bestehenden Rekonstruktion des originalen Core-FMatrix-Inverse beantwortet. Editor-/Laufzeitkameras, gemeinsame Objektinverse, Lichtpositions-/Reichweiten-/Farbkonstanten, Spotlights, Nebel, Zeitfunktionen, Rohwörter und Flicker werden gemeinsam geprüft. Dies ist keine Behauptung einer vollständigen Material-/Szenenanbindung.

`Record-CompleteConstants.py` prüft unabhängig Original-Import und Sprungtabelle, Fixtures, DrawScale3D-SSE-Befehle, Rotator-x87-Ausgaben, Core-Inversion, Matrixkomposition, Lichtpositionen und die kompletten Bänke. Ergebnis:

- 256 Dispatches, alle 35 regulären Typen abgedeckt.
- 196608 Vorher-/Nachher-Registerwörter, 384 inverse Host-Anfragen und 384 RNG-Werte geprüft.
- 256 Rotator-Ausgaben / 2048 x87-Registerwörter stimmen für die geprüften Proben überein; keine universelle Präzisionszusage.
- Debug-/Release-Berichte und Eingabedateien bytegleich.
- Sechs neue Grenzfall-/Cachetests; **585 Workspace-Tests**, Clippy mit `-D warnings`, Format- und Diffprüfung bestanden.

Berichte: `analysis/reports/complete-constants{,-release,-validation}.json`, Eingaben `complete-constants{,-release}.input.bin`, Gegenprobe `complete-constants.x87.bin`, Testlog `complete-constants-tests.log`.

Reproduktion im Projektordner auf x86_64:

```powershell
cargo run -p rc-inspect --bin rc-complete-constant-check -- analysis/reports/complete-constants.json analysis/reports/complete-constants.input.bin
cargo run --release -p rc-inspect --bin rc-complete-constant-check -- analysis/reports/complete-constants-release.json analysis/reports/complete-constants-release.input.bin
rustc scripts/Probe-RotatorConstants.rs -O -o target/rotator-constant-probe.exe
& target/rotator-constant-probe.exe analysis/reports/complete-constants.input.bin analysis/reports/complete-constants.x87.bin
cargo test --workspace *> analysis/reports/complete-constants-tests.log
python scripts/Record-CompleteConstants.py
```

Offen bleiben Live-Szenenbeschaffung einschließlich GCubemapManager-Actor, vollständige Renderzustands-/Materialanbindung, echte Registerhistorie und spielbare Engine. Die früheren Mesh-Vorschauen behalten ihre Fixtures; allgemeine Diffuse-Abdeckung bleibt 271/289. Android-Prüfung weiterhin zum Schluss.
