# Lichtreichweite, Objekt-Augenposition und Nebel

Stand 2026-10-08. Drei weitere Aufgaben des nativen D3DDrv-Konstantendispatchers umgesetzt. Grundlage: `analysis/decompiled/shader-constants.asm`, Original-DLL SHA256 im Validierungsbericht. Die Szenenwerte stammen weiterhin aus ausdrücklich gelieferten Host-Snapshots.

1. **LightInvRadius1..4, Fälle 16/19/22/25.** Nutzen dieselbe Vier-Quellen-Tabelle wie Position und Farbe, einschließlich ihrer nativen Quellplatz-Lücken. Fehlende gezählte Quelle liefert `[10000000,10000000,10000000,0]`. Bei Actor-Art 0x13: `[k,k,outer-inner,1]`, k hat Originalbits `322bcc77`. Sonst: Länge der ersten ObjectToWorld-Zeile geteilt durch `outer-inner`, zweimal ausgegeben; Z ist das rohe inner-Wort, W=1. Die Länge entsteht aus separaten f32-Quadraten, Summen und einer RSQRTSS-Newton-Verfeinerung. Die Konstanten 3 und 0.5 wurden direkt aus dem PE geprüft. Keine Inversion oder Richtungsnormalisierung.
2. **EyePositionObjectSpace, Fall 33.** Teilt den Objektinversen-Cache mit WorldToObject und den Lichtpositionen. Editorquelle: Viewport-Actor +138/+13c/+140; Laufzeitquelle: Kamera +194/+198/+19c. Beide sind von der CameraToWorld-Matrix getrennte Felder. Editor addiert Produkte in Reihenfolge X,Z,Y, Laufzeit Z,Y,X; danach Translation. W=1; vierte Inversen-Spalte wird ignoriert. Auch eine fehlende Laufzeitkamera wird erst nach der Inversen-Anfrage erkannt.
3. **Fog, Fall 31.** Rohe Start-/Endwörter bleiben X/Y. Mit `r=f32(1/f32(end-start))` gilt Z=`f32(end*r)`, W=r. Kein Clamp und keine Bereichssortierung. Nullbereiche behalten IEEE-Infinity/NaN; NaN-Payload-Gleichheit zwischen Prozessoren wird nicht zugesichert.

Die RSQRTSS-Saat ist eine ausdrückliche Host-Rechenpolitik. Ohne Politik wird der gewöhnliche Lichtfall abgewiesen. Der mitgelieferte portable Helfer verwendet `1/sqrt(q)` und behauptet keine Bitgleichheit mit x86-RSQRTSS. Die Diagnose nutzt die tatsächliche x86_64-Instruktion des Testrechners. Bei q=0 wird die Verfeinerung ausgeführt und das Ergebnis anschließend wie im Original auf rohe Null maskiert.

Sicherer Hostvertrag für Reichweite und Augenposition: nichtendliche relevante Eingaben/Ergebnisse werden abgewiesen; gewöhnliche Lichtreichweite mit Nullnenner ebenfalls. Das sind Schutzgrenzen, keine ursprünglichen Engine-Fehlerprüfungen. Ein fehlerhafter Registerwert wird nicht teilweise geschrieben; frühere Registerwrites, Zähler und erfolgte Host-Anfragen bleiben erhalten.

`rc-scene-constant-check` erzeugt 1024 synthetische Szenen, 4096 Reichweiten-, 2048 Augenpositions- und 1024 Nebelregister. `Record-SceneConstants.py` prüft die Fixtures, interpretiert die Original-SSE-Befehle unabhängig, kontrolliert 32768 Bankwörter und 1024 gemeinsame Inversen-Anfragen. Debug und Release sind bytegleich. Vier neue Grenzfall-/Cachetests; insgesamt 575 Workspace-Tests, Clippy mit `-D warnings` und Formatprüfung bestanden.

Berichte: `analysis/reports/scene-constants{,-release,-validation}.json`, Testlog `scene-constants-tests.log`. Reproduktion aus dem Projektordner:

```powershell
cargo run -p rc-inspect --bin rc-scene-constant-check -- analysis/reports/scene-constants.json
cargo run --release -p rc-inspect --bin rc-scene-constant-check -- analysis/reports/scene-constants-release.json
cargo test --workspace *> analysis/reports/scene-constants-tests.log
python scripts/Record-SceneConstants.py
```

Offene Konstantenfälle: SinTime 10, TanTime 11, XYCircle 13, DrawScale3D 30 und Rotator 34. Live-Szenenbeschaffung, vollständige Renderzustandsanbindung und spielbare Engine bleiben offen. Frühere Mesh-Vorschauen behalten ihre Fixtures; allgemeine Diffuse-Abdeckung bleibt 271/289. Android-Prüfung weiterhin zum Schluss.
