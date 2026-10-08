# Director-Auswahl und lokaler Translations-/Skalierungspfad

Original ApplyAnimation **1050abe9..1050ac2e**, ApplyDirector **10501bb0**.
Rust `skeletal_director.rs`; wiederverwendbarer Hierarchie-Hook in
`skeletal_hierarchy.rs`.

Pro fertig verknüpfter Knochenmatrix durchsucht die Engine die Director-Liste
ab dem Anfang. Der Knochenindex ist signed31 (`word0 << 1`, arithmetisch `>> 1`);
das höchste Bit ist kein Bestandteil des Index. Ein Treffer wird angewendet,
wenn mindestens eines der Bytes +48/+49/+4a ungleich 0 ist. Inaktive passende
Einträge werden übersprungen, beim ersten aktiven Treffer endet die Suche.

Die Korrektur erfolgt **nach** Elternmatrix-Verknüpfung und **vor** Berechnung
des nächsten Knochens. Damit verwenden Kinder bereits korrigierte Elternwerte.
Die Move-Root-/Editorregel bleibt erhalten. Der Hook erlaubt grundsätzlich den
Zugriff auf vorherige Matrizen, weil der noch offene Rotationspfad auch
Vorfahren verändern kann. Solche zukünftigen Backends müssen Bounds wie das
Original pro Knochen erfassen; eine nachträgliche Box aus allen endgültigen
Matrizen wäre bei rückwirkenden Änderungen nicht automatisch gleichwertig.

## Implementierter ApplyDirector-Teil

Ein Director-Snapshot enthält die originalen 28 Rohwörter (0x70 Bytes).
Der belegte Teil unterstützt **+4c Bit0 gesetzt** und **Rotation +49 aus**:

- Byte +48 aktiv kopiert alle vier Komponenten der gelieferten Matrixzeile 3
  in die aktuelle Knochenmatrixzeile 3, einschließlich W.
- Byte +4a aktiv multipliziert die ersten drei Matrixzeilen komponentenweise
  mit den entsprechenden gelieferten Zeilen, einschließlich ihrer W-Komponente.
  Das ist `FPlane::operator*=` **10114af0**, keine allgemeine Matrixmultiplikation.
- Translation wird vor der Plane-Skalierung angewendet.

Der lokale Einstieg lehnt Weltraum-Directors ausdrücklich ab. Der zusätzliche
Einstieg `apply_director_with_transform` unterstützt inzwischen deren Umrechnung
mit inverser MeshToWorld-Matrix; siehe [Weltraum-Directors](SKELETAL_WORLD_DIRECTORS.md).
Diese früheren unveränderbaren Einstiege unterstützen keine Rotation. Der neue
[veränderbare Pose-Einstieg](SKELETAL_DIRECTOR_APPLICATION.md) verbindet inzwischen
Rotation, Winkelbegrenzung, Quaternion-History und Korrektur früherer Knochenmatrizen.
In den früheren Einstiegen liefert ein ausgewählter Rotations-Eintrag ausdrücklich
einen Fehler, ohne ersatzweise einen späteren Director anzuwenden. Vorherige
Knochenmatrizen und die vor dem Aufruf fertig berechnete aktuelle Matrix bleiben
bei Fehler erhalten; spätere Knochen werden nicht weiterberechnet.

## Nachweis

`rc-director-pose-check` verwendet 900 vorher geprüfte Originaltrack-Posen und
wertet jede mit Spiel-/Editor-Hierarchie aus: **1.800 Posen, 59.824 Matrizen**.
Die Director-Snapshots sind kontrollierte Diagnoseeingaben: zuerst ein inaktiver
Treffer, danach ein markierter signed31-Treffer mit Translation/Plane-Skalierung,
zuletzt ein aktiver Rotationsdirector, der wegen des früheren Treffers nicht
aufgerufen werden darf. Sie sind keine geladenen Original-Directortracks.

Alle Matrizen und nachfolgenden lokalen Boxen/Kugeln wurden unabhängig geprüft;
die Bounds benutzen ausdrücklich Null-Meshpadding und kOne=(1,1,1). Der
implementierte lokale Pfad ändert keine Vorfahren, daher ist diese nachgelagerte
Bounds-Diagnose für ihn zulässig. Original-SSE-Ausdrücke prüfen Elternprodukte,
zusätzliche ASM-Nachweise prüfen Auswahl, Zeilenkopie und Plane-Multiplikation.
Fünf Tests prüfen Auswahl, Kindvererbung, Komponenten-/W-Skalierung, Fehlergrenzen
und Move-/Editorverhalten. **292 Tests**, Clippy und Formatprüfung bestanden.

Offen bleiben Director-Daten-/Runtimebindung, vollständige Pose-Vorbereitung
sowie Skinning. Android zum Schluss.
