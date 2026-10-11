# FSceneNode Project/Deproject und Vertex-Mesh-Anbindung

Ergaenzung 2026-10-10: `VERTMESH_PREPROCESS.md` schaltet jetzt die originale
Stream-/Sektionsvorbereitung und Actor-GetAbsMax vor diese Strecke. Die folgenden
Angaben zu diesen externen Antworten beziehen sich auf den isolierten historischen
Projektionsnachweis; dessen API und Berichte bleiben erhalten.

Stand: 2026-10-10. Fuenf Aufgaben: originale Vorwaertsprojektion, Rueckprojektion,
Float-/W-Grenzen und Roundtrips, transaktionale Mesh-Anbindung, Originalreplay.

## Original und Matrixlayout

Engine!104a3860..104a398d ist `FSceneNode::Project`,
Engine!104a3990..104a3a9a ist `FSceneNode::Deproject`. Ghidra-Exporte stehen unter
`analysis/decompiled/scene-project.{c,asm}` und `scene-deproject.{c,asm}`.
Die ASM-Dateien enthalten auch nachfolgende Instruktionen; der Replay stoppt
jeweils am RET und prueft Rueckgabezeiger, Stack und RET-8-Vertrag separat.

Project liest die zeilenweise Matrix Scene+114..+150 und Actorposition XYZ.
Fuer jede der vier Clipkomponenten ist die Reihenfolge genau
`((M[1][c]*Y + M[2][c]*Z) + M[0][c]*X) + M[3][c]`.
Danach berechnet das Original einmal `1/W` und multipliziert XYZ damit.
Das vierte Ergebniswort enthaelt **den unveraenderten Clip-W-Wert**.
Die Einskonstante 10653578 wird direkt aus dem Engine-PE geprueft.

Deproject liest eine separat gelieferte Matrix Scene+154..+18c. XYZ des Planes
werden zunaechst mit dessen gespeichertem W multipliziert. Fuer X folgt
`((M[2][0]*Z + M[3][0]*W) + M[1][0]*Y) + M[0][0]*X`.
Y und Z addieren nach den ersten beiden Produkten zuerst den X-, dann den
Y-Anteil. Eine vereinheitlichte Dotproductroutine wuerde diese Reihenfolge
veraendern. Keine FMA, Matrixinversion oder Genauigkeitserhoehung wird eingefuehrt.
Die Herkunft und Inversenbeziehung der Matrixsnapshots sind weiterhin Eingaben.

## Rustvertrag und sichere Grenzen

`scene_projection::{project,deproject}` verwenden rohe float32-Worte und
`shader_constants::Matrix`. Alle Matrix-/Vektoreingaben muessen finit sein,
auch die von Deproject nicht ausgewertete vierte Matrixspalte. Nichtfinite
Ergebnisse werden abgewiesen. Project weist beide Null-W-Vorzeichen und
einen nichtfiniten Kehrwert ab. Negatives W bleibt erlaubt. Deproject fuehrt
keine Division durch und akzeptiert W=0; W-Multiplikationsueberlauf ist ein Fehler.
Diese Fehler sind bewusste sichere Portgrenzen, keine behaupteten nativen
Fehlerrueckgaben. NaN-Payloads, abweichende MXCSR-Rundung und FTZ/DAZ sind nicht
nachgewiesen. Standard-float32-Rundung ohne FMA wird vorausgesetzt.

Zwölf Rusttests pruefen Identitaets- und Perspektivroundtrip, W-Erhaltung,
Kehrwertmultiplikation, Additionsreihenfolgen mit Ausloeschung, negatives W,
W-abhaengige Ruecktranslation, Null-W, nichtfinite Werte und Ueberlaeufe.

## Gemeinsame Meshtransaktion

`Runtime::submit_projected_mesh` nimmt `world_to_screen`, `actor_position` und
die vorhandene `vertmesh_lod::Request` an. Es berechnet die komplette Projektion
und ersetzt den Kamerawert `projected_w` im geklonten Request. Danach laeuft
`submit_lod` mit Gates, Vertexcallbackvertrag, Collapse-/Indexgenerierung,
Bufferupload und Materialpasses. Der Plan enthaelt Projektion und Meshplan.
Bei Erfolg wird der abgeleitete Kamerasnapshot uebernommen, auch bei einem
fruehen Gate-Ende. Bei Projektions- oder spaeteren Meshfehlern bleiben Runtime
und urspruenglicher Request erhalten. Es gibt keinen zusaetzlichen Runtimeklon:
die untergeordnete Meshoperation ist bereits transaktional.

## Vergleich und Ergebnis

`Generate-SceneProjection.py` verknuepft den SHA256-geprueften LOD-Input mit
synthetischen Matrix-/Actorpositionen. Der urspruengliche Kamerawert W ist
absichtlich NaN, damit ein versehentlich unveraendert weitergereichter Wert
auffaellt. Es sind keine Live-Kamera- oder Spielaufnahmen.

`Record-SceneProjection.py` ersetzt im vorhandenen Kamerareplay ausschliesslich
die bisher eingespeiste Project-Antwort durch den originalen Funktionskoerper.
Die originale Callstelle 105601f5 samt Argumenten, Stackvertrag, Ausgabepuffer
und anschliessendem W-Lesen bleibt sichtbar. Das bestehende Vergleichsloop
prueft vollstaendige Plaene, Requests und CPU-/Runtimezustaende nach jeder Anfrage.
Die alten Berichte und ihre historischen Testzahlen werden nicht umgeschrieben.

Ergebnis: **327 Mathematikproben**, davon **322 Project- und 326 Deproject-
Originalaufrufe**, **37.250 Mathematikinstruktionen**; **6 sichere Mathematikfehler**.
Darunter sind 64 perspektivische W-Matrizen und eine Ausloeschungsprobe.
Die gemeinsame Strecke prueft **50 Sequenzen / 146 Anfragen**, weitere **146
Original-Project-Aufrufe** und **4.647.392 Originalinstruktionen**. 36/30 fruehe
Sektions-/Actor-Gates, 80 Vertexcallbacks und je 80 Vertex-/Indexuploads,
67.080 erzeugte Indizes, 276 Draws und 1.197 Materialpass-Durchlaeufe stimmen
ueberein. **16 Fehlerfaelle** pruefen unveraenderte Requests und Runtime bei
fruehen Projektions- und spaeten Callback-/Geometrie-/Upload-/Materialfehlern.

**12 neue Rusttests, insgesamt 865 Workspace-Tests bestanden.** Debug/Release
bytegleich; Clippy alle Targets mit `-D warnings`, Format und Diff bestanden.
Berichte `analysis/reports/scene-projection*`, Evidenz `scene_projection_validation`.

```powershell
python scripts/Generate-SceneProjection.py
cargo run -p rc-inspect --bin rc-scene-projection-check -- analysis/reports/scene-projection.input.json analysis/reports/scene-projection.json
cargo run --release -p rc-inspect --bin rc-scene-projection-check -- analysis/reports/scene-projection.input.json analysis/reports/scene-projection-release.json
cargo test --workspace *> analysis/reports/scene-projection-tests.log
python scripts/Record-SceneProjection.py
```

Offen: Konstruktion der Szenenmatrizen, Meshtransform/RenderPreProcess,
x87-FPTAN/GetAbsMax, Vertexpositionen und Animation, echte Materialkompilierung,
COM-/GPU-Ausfuehrung. Das Spiel ist noch nicht spielbar. Androidpruefung zum Schluss.
