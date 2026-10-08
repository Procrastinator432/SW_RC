# Inverser Referenzpose-Cache

`ensure_inverse_reference_cache` rekonstruiert den vorbereitenden Abschnitt von ApplyAnimation bei `10509f3c..1050a43b`. Grundlage sind die bereits gelesenen Original-Referenzknochen, keine Animationssamples.

Ein nicht leerer Cache wird unverändert wiederverwendet, auch bei inzwischen anderer Knochenanzahl oder ungültigen Referenzdaten. Dieser Pfad verändert das Instanzbyte +61 nicht. Bei leerem Cache wird +61 auf 0 gesetzt und der gesamte Ergebnisbereich mit Nullmatrizen angelegt. Anschließend entsteht für jeden Knochen die lokale Quaternion-/Translationsmatrix. Ab Knochen 1 wird sie mit der bereits berechneten Elternmatrix komponiert und mit der originalen skalaren Matrixinverse invertiert.

Die Referenzvorbereitung verknüpft auch Kinder des Move-Knochens; sie besitzt hier weder dessen Abtrennung noch einen Editor-Schalter. Ihre SSE-Additionsreihenfolge unterscheidet sich von der späteren Poseauswertung. `skeletal_reference_product.rs` wird deshalb aus genau diesem Originalabschnitt erzeugt. Die Inverse verwendet den zuvor rekonstruierten Identitätsrückfall bei exakt singulärer Matrix.

Rust lehnt Elternindizes ab, die negativ sind oder nicht vor dem aktuellen Knochen liegen. Bei einem solchen Fehler bleiben die vollständige Allokation, das invalidierte Cachebyte und bereits berechnete inverse Matrizen erhalten. Native ungültige Speicherzugriffe werden nicht emuliert. Ein nachfolgender Aufruf sieht den nicht leeren Teilcache und verwendet ihn entsprechend dem ursprünglichen Gate erneut. Bei einem leeren Skelett bleibt der Cache leer und wird beim nächsten Aufruf erneut vorbereitet.

`rc-reference-cache-check` erzeugt 130 Caches mit 3.111 inversen Referenzmatrizen. `Record-ReferenceCaches.py` berechnet sie unabhängig mit einem numerischen Interpreter der Original-ASM und prüft jede Matrix bitgenau für die verwendeten endlichen Eingaben. Zusätzlich werden 64 allgemeine Elternprodukte bitgenau gegen ASM und gegen f64-Matrixmultiplikation geprüft. Das bekannte Legacy-Mesh wird ausgeschlossen. Fünf Tests behandeln Move-Eltern, veraltete Caches, Teilzustände bei Fehlern, singuläre Matrizen und leere Skelette. **341 Workspace-Tests**, Clippy mit `-D warnings` und Formatprüfung bestanden.

Das Modul ist eine geprüfte Vorbereitung auf gelieferten Mesh-Snapshots. Die [Instanzvorbereitung](SKELETAL_PREPARATION.md) verbindet es inzwischen mit Pufferanpassung, Transform-Host und geordneten Linkup-Hosts. Die Verbindung mit dem vollständigen ApplyAnimation-Einstieg sowie die Nutzung beim Skinning und im Renderer stehen noch aus. Native CPU-Sondermodi werden nicht emuliert; eine sichtbare Spielanimation folgt daraus noch nicht.

Nachweise: `analysis/decompiled/skeletal-root-frame.asm`, `skeletal-apply-animation.c`, `analysis/reports/reference-caches.json`, `reference-caches-validation.json` und `analysis/evidence.json`. Androidprüfung am Projektende.


Fortsetzung 2026-10-08: [Verbundene vorbereitete Vollpose](SKELETAL_FULL_POSE.md) verbindet jetzt Kanal-Auswertung, Director-Hierarchie, Bounds und das gemeinsame Cachebyte. Die vorgelagerte Puffer-/Transform-/Inverse-/Linkup-Vorbereitung bleibt vorausgesetzt; 347 Workspace-Tests bestanden.
