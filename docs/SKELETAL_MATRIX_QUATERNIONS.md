# Native Matrix-zu-Quaternion-Umrechnung

Stand: 2026-10-07. `quaternion_matrix::quaternion_from_matrix` rekonstruiert
**core 10144cb0, FQuat::FQuat(FMatrix const&)**. `ApplyDirector` verwendet diese
Funktion für die erstmalige History aus der aktuellen Knochenmatrix sowie für
die Zielrotation aus der vorbereiteten Director-Matrix. Die Aufrufe liegen bei
**engine 105020c8** und **105020f6**. Der neue Helfer ist geprüft, aber noch nicht
in einen vollständigen Rotations-Director-Pfad eingebunden.

## Belegtes Verhalten

Die Eingabe besteht aus 16 rohen `f32`-Worten. Translation, W-Spalte und letzte
Zeile werden ignoriert. Es gibt keine Entfernung von Skalierung/Scherung und
keine abschließende Normalisierung.

Die Spur wird in der Reihenfolge `((m11 + m00) + m22) + 1` berechnet. Ist sie
positiv, folgt der Spurzweig. Andernfalls wählt das Original X nur bei
`m00 > m11 && m00 > m22`, danach Y bei `m11 > m22`, sonst Z. Gleichstände und
unordered Vergleiche fallen daher gezielt in spätere Zweige.

Jeder Zweig fordert einmal den RSQRTSS-Seed über die vorhandene
`QuaternionMath`-Schnittstelle an. Danach werden die originale Newton-Korrektur,
Multiplikation mit dem Radikanden und Nullmaske ausgeführt. Erst nach diesen
Operationen wird bei exakt null das Ergebnis auf +0 gesetzt. Ein Fehler der
Math-Schnittstelle liefert ausdrücklich einen Fehler ohne Quaternion-Ausgabe.
Die portable Policy verwendet `1 / f32::sqrt`; sie emuliert keinen bestimmten
x86-RSQRTSS-Seed und keine SSE-Kontrollregister.

Die Zweige bei nichtpositiver Spur unterscheiden sich von verbreiteten
Matrix-zu-Quaternion-Formeln: Die dominante Komponente ist
`(0.5 / reconstructed_root) * 0.5`; die W-Komponente verwendet eine Summe
symmetrischer Nebendiagonalelemente. Deshalb ergibt etwa die exakte
X-Halbdrehungsmatrix `diag(1,-1,-1)` unter der portablen Policy
**(0.125, 0, 0, 0)**. Dies ist durch Original-ASM und die DLL-Konstanten belegt
und wird bewusst nicht durch eine übliche Formel ersetzt. Der Spurzweig
verwendet für W dagegen `0.25 / factor`.

## Nachweis

`rc-matrix-quaternion-check` prüft die Knochenmatrizen der 900 zuvor geprüften
Weltraum-Director-Spielposen mit der ersten vorgegebenen MeshToWorld-Matrix.
Dies sind **29.912 Umrechnungen**; die Posen stammen aus Originaltracks,
Director- und Transformations-Snapshots bleiben Diagnoseeingaben.

`scripts/Record-MatrixQuaternions.py` interpretiert separat die Originalbefehle,
einschließlich Sprüngen, Vergleichen, RSQRT-Grenze und Bitmasken. Alle Ausgaben
stimmen unter derselben portablen Seed-Policy bitgenau überein. Die Verteilung
auf die Zweige beträgt Spur 13.040, X 9.395, Y 7.086, Z 391.

Zusätzliche **256 allgemeine und gezielte Zweigproben** verwenden einen
vorgegebenen Seed von 0.125. Auch sie stimmen bitgenau überein und erfassen
alle vier Zweige sowie Diagonalgleichstände. Die Konstanten 1, 0.5, 3 und 0.25
werden aus den Originalbytes von `core.dll` geprüft, nicht aus dem Decompiler
übernommen.

Sechs neue Tests prüfen Identität und ignorierte Translation/W-Werte,
Halbdrehungen, Diagonalgleichstände, die symmetrische W-Summe, NaN-Zweigwahl
und Fehler der Math-Schnittstelle. **304 Workspace-Tests**, Clippy mit
`-D warnings` sowie Formatprüfung bestanden. Bericht, Hashes und Prüfdetails:
`analysis/reports/matrix-quaternions-validation.json`, `analysis/evidence.json`.

## Offene Arbeit

History-Initialisierung und -Aktualisierung, zeitliche und absolute
Winkelbegrenzung sowie Quaternion-Potenz sind inzwischen als separate
[Rotationsvorbereitung](SKELETAL_DIRECTOR_ROTATION.md) verbunden und geprüft.
Rotationsanwendung, Actor-Skalierung und Vorfahrenkorrektur sind noch offen.
Der vorhandene Director-Einstieg
weist Rotationsanforderungen deshalb weiterhin ausdrücklich zurück. Ebenfalls
offen sind Runtimebindung und Skinning. Android wird am Schluss geprüft.

Die archivierten Helfer `RotationAngleFast` (1011fe70) und
`FQuat::operator^` (10146d40) sind in der Rotationsvorbereitung inzwischen unter
einer ausdrücklich portablen Math-Policy umgesetzt.
