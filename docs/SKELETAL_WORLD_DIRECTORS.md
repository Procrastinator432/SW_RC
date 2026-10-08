# Weltraum-Directors und native Matrixinversion

Stand: 2026-10-07. Der Translations-/Plane-Skalierungspfad unterstützt jetzt
auch Director-Snapshots mit **+4c Bit0 aus**. Die MeshToWorld-Matrix wird vom
Aufrufer vorgegeben. Es besteht noch keine Bindung an laufende Original-Actors.

## Originalbefund und Umsetzung

`FMatrix::Inverse`, core **10143b40**, verarbeitet eine allgemeine 4×4-Matrix
mit skalaren SSE-Operationen. `skeletal_matrix_inverse.rs` bewahrt die
Reihenfolge jeder einzelnen Addition, Subtraktion, Multiplikation und Division
als getrennte `f32`-Operation. Die 318 Registerzuweisungen werden durch
`scripts/Generate-MatrixInverse.py` aus dem archivierten ASM erzeugt; Kommentare
ordnen sie den Originaladressen zu. Nach Regeneration ist `cargo fmt --all`
auszuführen. Dies ist keine Beschränkung auf affine Matrizen.

Bei **exakt null** als berechneter Determinante liefert die Engine die
Identitätsmatrix zurück. Es gibt weder einen Epsilon-Test noch eine Fehlermeldung.
Die Gleichheitsprüfung schließt unordered aus: NaN löst diesen Rückfall nicht
aus. Ein Test prüft NaN-Fortpflanzung; NaN-Payloads und abweichende
SSE-Kontrollregister werden nicht emuliert.

`ApplyDirector`, engine **10501c85–1050209d**, bildet anschließend
`Director * inverse(MeshToWorld)`. Jedes Matrixelement summiert die Produkte
in der Reihenfolge **3, 2, 1, 0**, jeweils linksassoziativ. Die Operandenreihenfolge
der Multiplikationen entspricht ebenfalls den Originalbefehlen.

`apply_director_with_transform` verwendet die umgerechnete Kopie für den
bereits belegten lokalen Translations-/Plane-Skalierungspfad. Der übergebene
Director-Snapshot bleibt unverändert. Directors mit gesetztem Lokal-Bit umgehen
die Inversion. Rotation liefert weiterhin einen ausdrücklichen Fehler, bevor
die aktuelle Knochenmatrix verändert wird.

`build_directed_pose_with_transform` verbindet dies mit der Director-Auswahl
innerhalb der Hierarchiestufe: Die Korrektur erfolgt nach der Elternverknüpfung
des aktuellen Knochens und vor der Berechnung seiner Kinder. Das erste aktive
passende Director-Element gewinnt weiterhin; ein Fehler führt nicht zum nächsten
Director. Bereits berechnete Matrizen bleiben bei Fehler erhalten.

## Unabhängige Prüfung

`rc-world-director-check` übernimmt die 900 Spiel-Posen aus dem vorher geprüften
Originaltrack-/Channel-Stack-Bericht. Jede wird mit drei vorgegebenen
MeshToWorld-Matrizen ausgewertet: nichtuniforme Skalierung plus Translation,
Achsenwechsel plus Scherung und Translation sowie eine singuläre Nullmatrix.
Die Director-Snapshots sind weiterhin kontrollierte Diagnoseeingaben.

`scripts/Record-WorldDirectorPoses.py` prüft:

- **2.700 Posen / 89.736 Knochenmatrizen**, einschließlich Vererbung korrigierter
  Eltern, bitgenau gegen unabhängig interpretierte Original-SSE-Ausdrücke.
- **128 allgemeine 4×4-Inversionen** und die zwei affinen Transformationen
  bitgenau gegen einen separaten skalaren ASM-Interpreter sowie numerisch gegen
  `f64`-Gauss-Jordan und das Produkt mit der Ausgangsmatrix.
- **32 allgemeine Matrixumrechnungen** bitgenau gegen aus dem Original-ASM
  gelesene Ausdrucksbäume und den Identitätsrückfall der singulären Transformation.

Der maximale Inversionsfehler gegenüber Gauss-Jordan beträgt
`7.95e-8`, die maximale Abweichung des Matrixprodukts von der Identität `8.50e-8`
für diese Eingaben. Vier Inversions-Tests und zwei weitere Director-Tests decken
unter anderem Singularität, nichtuniforme Skalierung, Translation, NaN und eine
auslöschungsbedingte Abweichung bei falscher Summenreihenfolge ab.
**298 Workspace-Tests**, Clippy mit `-D warnings` und Formatprüfung bestanden.
Hashes und Prüfdetails stehen in
`analysis/reports/world-director-poses-validation.json` und `analysis/evidence.json`.

## Offene Grenzen

Original-Director-/Actor-Runtimebindung, Rotations-Directors einschließlich
History, Winkelbegrenzung und Vorfahrenkorrektur, vollständige Pose-Vorbereitung,
inverse Bind-Posen und Skinning sind offen. Dieser Schritt erzeugt geprüfte
Knochenmatrizen; er ist noch keine animierte Spielfigur oder spielbare Portierung.
Die Android- und Emulatorprüfung erfolgt wie vereinbart am Schluss.

Die für Rotations-Directors erforderliche Matrix-zu-Quaternion-Umrechnung ist
inzwischen separat rekonstruiert und geprüft; siehe
[Matrix-Quaternion-Helfer](SKELETAL_MATRIX_QUATERNIONS.md). Die Anwendung der
Rotations-Directors ist inzwischen über den neuen
[veränderbaren Pose-Einstieg](SKELETAL_DIRECTOR_APPLICATION.md) verbunden und
geprüft. Die oben beschriebenen früheren Einstiege bleiben auf Translation und
Plane-Skalierung beschränkt; Runtimebindung und Skinning sind weiterhin offen.
