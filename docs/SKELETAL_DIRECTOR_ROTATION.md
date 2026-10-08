# Director-Rotationsvorbereitung, History und Winkelbegrenzung

Stand: 2026-10-07. `skeletal_director_rotation::prepare_director_rotation`
rekonstruiert **ApplyDirector, engine 105020b3–105022f2**. Der Aufrufer liefert
die aktuelle Knochenmatrix, die bereits in den lokalen Raum umgerechnete
Director-Matrix und den vorbereiteten Actor-Skalierungsvektor.

Diese Funktion bildet die Vorbereitungsstufe ab. Sie schreibt noch keine
Rotationszeilen in die Knochenmatrix und führt keine relative Matrixverknüpfung
oder Vorfahrenkorrektur aus. Der neue veränderbare Pose-Einstieg verbindet sie
inzwischen mit der [Rotationsanwendung](SKELETAL_DIRECTOR_APPLICATION.md).
Die früheren unveränderbaren Translations-/Skalierungseinstiege weisen
Rotations-Directors weiterhin ausdrücklich zurück.

## Originalreihenfolge

1. Ist das History-Byte **+6c** null, wird die aktuelle Knochenmatrix in ein
   Quaternion umgerechnet, nach **+58..+64** kopiert und +6c auf 1 gesetzt.
   Die drei folgenden Padding-Bytes bleiben unverändert. Jedes bereits
   nichtnull gesetzte Byte überspringt diese Initialisierung.
2. Die lokale Director-Matrix wird in das Zielquaternion umgerechnet.
3. Ist **+54 >= 0**, wird `AngleDiffFast(history, target)` aufgerufen und das
   Ergebnis zu `f32` gespeichert. Übersteigt es das Budget **+68**, folgt
   Slerp mit `budget / distance`. Anschließend wird +68 immer auf +0 gesetzt,
   auch wenn keine zeitliche Begrenzung nötig war.
4. Ist **+50 >= 0**, wird `RotationAngleFast(target)` berechnet und zu `f32`
   gespeichert. Übersteigt der Winkel +50, folgt `target ^ (maximum / angle)`.
5. Das endgültige Zielquaternion wird nach +58..+64 als neue History kopiert.

Die Aktivierungsvergleiche sind geordnet: NaN deaktiviert die jeweilige Grenze,
-0 aktiviert sie. +54 ist hier nur ein Gate; es wird in dieser Funktion nicht
mit einer Zeitdifferenz multipliziert. Das Budget +68 wird als Snapshot
übergeben, seine vorgelagerte Akkumulation ist nicht Bestandteil dieses Schritts.

Jede tatsächlich erfolgte Begrenzung baut die Director-Matrix aus dem neuen
Quaternion und der bisherigen XYZ-Translation neu auf. Dabei wird der
Skalierungsvektor auf (1,1,1) zurückgesetzt; W der neuen letzten Zeile ist 1.
Ohne Begrenzung bleiben die gelieferte Matrix und Actor-Skalierung erhalten.

History-Initialisierung und Budget-Reset sind unmittelbar sichtbare Schreibzugriffe.
Ein späterer Fehler der Math-/Aufrufschnittstelle rollt sie nicht zurück. Die
abschließende History-Kopie erfolgt erst nach erfolgreichem Ende beider Grenzen.
Tests prüfen die Snapshots an sechs aufeinanderfolgenden Fehlerstellen.

## Mathematische Helfer und Genauigkeitsgrenze

`DirectorRotationHost` stellt explizite Schnittstellen für Matrixkonversion,
Winkeldifferenz, Slerp, schnellen absoluten Winkel und Quaternion-Potenz bereit.
`PortableDirectorRotationHost` verbindet den bereits geprüften Matrix-Quaternion-
und Slerp-Code mit folgenden rekonstruierten Helfern:

- **core 1011feb0, AngleDiffFast:** Skalarprodukt in x87-Reihenfolge W,Z,Y,X,
  Betrag, obere Grenze 1, danach `2 * acos`. Unordered wird ebenfalls auf 1
  begrenzt. Die portable Umsetzung approximiert x87 mit `f64`.
- **core 1011fe70, RotationAngleFast:** Betrag der W-Komponente, obere Grenze
  1 einschließlich unordered, danach `2 * acos`. Kein Normieren.
- **core 10146d40, operator^**, mit **10146b80, RotationAngle:** Norm der
  XYZ-Komponenten in skalarem SSE mit RSQRT-Seed, einer Newton-Korrektur und
  nachträglicher Nullmaske. Die Norm wird oben auf 1 begrenzt und der Winkel
  über `2 * asin` berechnet. W wird für diesen Winkel nicht gelesen. Ist der
  Winkel nicht geordnet positiv, wird das ursprüngliche Quaternion kopiert.
  Andernfalls werden XYZ mit dem Sinusverhältnis skaliert und W durch den
  Kosinus des skalierten halben Winkels ersetzt.

Innerhalb von `operator^` bleibt der von `RotationAngle` zurückgegebene Winkel
in x87 ST0; hier gibt es **keinen f32-Zwischenspeicher**. Die portable Umsetzung
bewahrt ihn als `f64`. Das ist eine Näherung der x87-/Transzendentalarithmetik,
keine bitgenaue Emulation des Originalprozessors. Die beiden Fast-Winkel werden
dagegen im Director-Aufrufer tatsächlich zu f32 gespeichert. Die Gate-Konstanten
in engine 1065efcc und core 10186c1c sind aus DLL-Bytes als +0 belegt.

## Validierung

`rc-director-rotation-check` erzeugt vier Konfigurationen für jede der 900 zuvor
geprüften Originaltrack-abgeleiteten Spielposen. Verwendet werden deren erste
zwei Knochenmatrizen als aktuelle und gelieferte Zielmatrix, vorgegebene
Director-Snapshots und Actor-Skalierung (2,0.5,3). Die Konfigurationen kombinieren
initialisierte/uninitialisierte History und deaktivierte/aktive Winkelgrenzen.

`scripts/Record-DirectorRotations.py` berechnet unabhängig alle Quaternion-
Ergebnisse, Matrixneuaufbauten, Skalen, Begrenzungsflags und mutierten Director-
Snapshots. Matrixkonversionen werden mit dem separaten Original-ASM-Interpreter
berechnet; die Winkel-/Slerp-/Potenzprüfung verwendet dieselbe ausdrücklich
portable Genauigkeitspolitik, ohne Rust-Code aufzurufen.

Alle **3.600 Ergebnisse** stimmen unter dieser Policy bitgenau überein:
1.800 History-Initialisierungen, 2.700 Budget-Resets, 2.282 zeitliche und 385
absolute Begrenzungen. Acht neue Tests prüfen zusätzlich NaN-Gates, Byte-Padding,
gleich große Grenzen, Aufrufreihenfolge, Fehler-Snapshots und die unterschiedlichen
Winkeldefinitionen. **312 Workspace-Tests**, Clippy mit `-D warnings` sowie
Formatprüfung bestanden. Nachweise und Hashes liegen in
`analysis/reports/director-rotations-validation.json` und `analysis/evidence.json`.

## Offene Verbindung

Die Anwendung der vorbereiteten Rotation, relative Komposition über Director
+4c Bit1, Actor-Skalierung auf die Rotationszeilen und Korrektur früherer
Knochenmatrizen sind inzwischen separat verbunden und geprüft. Offen bleiben
vorgelagerte Budget-Akkumulation, Director-/Actor-Runtimebindung, die vollständige
Pose-Pipeline und Skinning. Android und Emulator am Schluss.
