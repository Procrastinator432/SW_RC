# Verbundene Director-Pose und lokale/Weltgrenzen

Stand: 2026-10-07. `skeletal_directed_bounds::evaluate_directed_pose` verbindet
die vorbereitete lokale Pose mit Hierarchie, veränderbarer Director-Auswertung,
lokaler Box/Kugel, optionaler Weltpublikation und den Abschlussflags aus
**engine 10509dc0, ApplyAnimation**.

Der Einstieg beginnt ausdrücklich nach Channel-Auswertung und Vorbereitung.
Er entscheidet weder über Cachetreffer noch über den Root-only-Zweig. Der Aufrufer
liefert lokale Transformationsdaten, vorbereitete Hierarchie, Director-Snapshots,
MeshToWorld, Actor-Skalierung und Bounds-Padding. `DirectedPoseInput` bündelt
diese Eingaben; veränderbare Matrizen, Directors und Grenzen bleiben getrennt
sichtbare Ausgaben.

## Schreibreihenfolge

Für jeden Knochen wird zunächst seine Quaternion-/Translationsmatrix erzeugt
und bei Bedarf mit der Elternmatrix verknüpft. Dann folgt die Auswahl und
Anwendung des ersten aktiven passenden Directors, einschließlich History,
Winkelgrenzen, relativer Komposition und Korrektur früherer Knochenmatrizen.

Erst nach erfolgreichem Director-Aufruf wird die XYZ-Translation des aktuellen
Knochens in die lokalen Grenzen aufgenommen. Dies entspricht
**1050ac31–1050acf8**. `accumulate_pose_point` enthält dafür die bisherigen
Originalvergleiche als separaten Helfer. Move+1 initialisiert Minimum/Maximum;
spätere Punkte verwenden geordnete Vergleiche. Ein vorhandener Move-Knochen
selbst trägt nicht bei. Existiert kein Initialisierungspunkt, bleiben die
vorherigen Grenzen für die nachfolgende Erweiterung erhalten.

Nach der letzten erfolgreichen Knochenberechnung führt
`finish_accumulated_bounds` die bestehende Ursprungsskalierung um 1.2,
Padding-Erweiterung, lokale Kugel und Publikation aus. Der bisherige Helfer
`finish_prepared_bounds` delegiert auf dieselben Sammlungs-/Abschlussfunktionen;
seine vorher geprüfte Semantik bleibt erhalten.

Der Publikationshost kann keinen Actor oder einen vorgegebenen Actor-Primitive-
Transform mit Weltgrenzen-Ausgabe darstellen. **Erst nach erfolgreicher
Publikation** werden +60=1, +61=1 und +179=0 gesetzt.

## Fehlergrenzen

- Ein ungültiger Parent oder fehlgeschlagener Director erhält frühere Matrizen
  und rohe Punktgrenzen. Der aktuelle Punkt wird bei Director-Fehler noch nicht
  gesammelt; Padding, Kugel, Publikation und Abschlussflags werden übersprungen.
- Bei Fehler des lokalen RSQRT-Seeds sind die lokale Box bereits erweitert,
  lokale Kugel und Abschlussflags aber unverändert; Weltgrenzen bleiben unberührt.
- Bei Fehler des Welt-RSQRT-Seeds sind lokale Box/Kugel und Weltbox bereits
  geschrieben, während Weltkugel und Abschlussflags ihren vorherigen Zustand
  behalten. Dies übernimmt die separat geprüfte Weltpublikationsfunktion.

Die expliziten Math-Fehler sind Diagnosegrenzen; sie stellen keine behaupteten
C++-Ausnahmen der Original-SSE-Instruktionen dar.

## Validierung

`rc-directed-pose-bounds-check` wertet alle **7.200** Posen aus dem vorherigen
Rotationsdirector-Bericht erneut über den verbundenen Einstieg aus. Dabei
vergleicht das Programm sämtliche **239.296 Matrizen** und veränderten Director-
Snapshots direkt mit dem vorherigen Bericht. Es bricht bei jeder Abweichung ab.
Der Recorder überprüft zusätzlich die FNV-1a-64-Fingerprints aller Matrizen
gegen diesen zuvor unabhängig validierten Bericht.

Die lokalen Boxen, Kugeln und Publikations-Snapshots werden unabhängig aus den
Matrizen berechnet. Für die Weltgrenzen verwendet der Recorder den bestehenden
Interpreter der Original-SSE-Ausdrücke für Ecken und Kugelzentrum. Geprüft sind
**5.400 Weltpublikationen** mit nichtuniformer Skalierung/Translation, Scherung/
Achsenwechsel sowie Nullmatrix und **1.800 Fälle ohne Actor-Publikation**.
Die publizierten lokalen Snapshots tragen noch die alten Abschlussflags; nur
die erfolgreiche Endausgabe enthält die neuen Flags.

Das Padding ist ausdrücklich diagnostisch: Minimum (0.25,1,2), Maximum
(2,0.5,1), kOne (1,1,1). Es wird nicht als bereits aus Original-Meshdaten
rekonstruierter Runtimewert dargestellt. Die Quaternion-/RSQRT-Policy bleibt
portabel, keine bitgenaue x87-/CPU-Emulation.

Sechs neue Integrationstests prüfen Director-/Kindpositionen, Fehler bei Director,
Parent, lokalem Seed und Weltseed sowie eine Move-only-Pose ohne Actor.
Zum Abschluss dieses Meilensteins bestanden **324 Workspace-Tests**, Clippy
mit `-D warnings` und Formatprüfung; der nachfolgende SetBonePlace-Meilenstein
erhöht den Gesamtstand auf 330 Tests. Details und Hashes:
`analysis/reports/directed-pose-bounds-validation.json`, `analysis/evidence.json`.

Offen bleiben die vorgelagerten Cache-/Puffer-/Referenzvorbereitungen,
Director-/Actor-/MeshToWorld-Runtimebindung, Budget-Akkumulation, inverse
Bind-Posen und Skinning. Die Android-/Emulatorprüfung erfolgt am Schluss.


Fortsetzung 2026-10-08: [Verbundene vorbereitete Vollpose](SKELETAL_FULL_POSE.md) verbindet jetzt Kanal-Auswertung, Director-Hierarchie, Bounds und das gemeinsame Cachebyte. Die vorgelagerte Puffer-/Transform-/Inverse-/Linkup-Vorbereitung bleibt vorausgesetzt; 347 Workspace-Tests bestanden.
