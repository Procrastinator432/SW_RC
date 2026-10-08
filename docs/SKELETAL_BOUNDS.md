# Lokale Posegrenzen, Kugel und Abschlussflags

Original `ApplyAnimation` 10509dc0: Knochenpunkte 1050ac31..1050acf8,
lokale Erweiterung/Sphäre 1050ad04..1050af10, Abschluss 1050b27c..1050b284.
Rust `skeletal_bounds.rs` verarbeitet bereits ausgewertete Matrizen.

Die erste berücksichtigte Position ist Knochenindex MoveBone+1. Bei MoveBone=0
bleibt der Move-Root draußen, bei -1 beginnt die Box am Root. Spätere Punkte
erweitern Minimum/Maximum mit geordneten Vergleichen; NaN erweitert nichts.
Fehlt der Initialisierungsknochen, bleiben vorherige Boxwerte Grundlage der
weiteren Rechnung. Es wird keine zusätzliche Validitätsmarke gesetzt.

Alle sechs Boxkoordinaten werden um den Ursprung mit **1,2** skaliert, dessen
f32-Bitmuster 0x3f99999a direkt in engine.dll 10664fac verifiziert ist. Minimum
wird anschließend um mesh+29c plus importiertes FVector::kOne vermindert,
Maximum um mesh+2a8 plus kOne erhöht. Diese Runtimevektoren sind bewusst
explizite Eingaben; der bisherige Mesh-Präfixleser erreicht sie noch nicht.
Auch der aufgelöste Runtimewert von kOne wird übergeben.

Kugelzentrum ist (max+min)/2, wobei die Y-Summe in umgekehrter Operandenreihenfolge
steht. Der belegte FVector-Divisionsoperator bildet zuerst 1/2 und multipliziert
damit. Radius ist die halbe Boxdiagonale mit nativer SSE-Reihenfolge x²+y²+z²,
RSQRTSS-Seed und einer Newtonkorrektur. Ein Distanzquadrat von 0 maskiert das
berechnete Ergebnis erst nach dem Seedaufruf auf +0.

Die Box wird vor dem Seedaufruf geschrieben, die Kugel danach. Eine explizite
Hostoperation veröffentlicht bei vorhandenem Actor-Primitiv die Weltgrenzen;
bei Actor+48=null ist sie ein No-op. Erst danach werden Instanzbytes +60/+61
auf 1 und +179 auf 0 gesetzt. Seed-/Publikationsfehler erhalten die bereits
geschriebenen Werte, ohne Abschlussflags vorzeitig zu setzen.

## Prüfung und Grenzen

`rc-pose-bounds-check` benutzt die 900 zuvor unabhängig geprüften Originaltrack-
Kanalstapelmatrizen und 64 zusätzliche synthetische Fälle. Die Polsterung ist
**explizit diagnostisch**: Minimum (.25,1,2), Maximum (2,.5,1), kOne (1,1,1).
Sie ist kein aus den Originalmeshes geladener Wert. Die ursprünglichen
Knochenmatrizen sind die geprüfte Grundlage; konkrete Original-Spielbounds
werden wegen der noch fehlenden Runtimepolsterung und Directors nicht behauptet.

Originalmatrixfälle nutzen die portable Quadratwurzel-Policy. Synthetische
Fälle liefern einen festen Seed .125 zum Prüfen der Rechenfolge, einschließlich
leerer Matrizenlisten und fehlendem Initialisierungsknochen. Actor+48 gilt in
allen Diagnosefällen als null; der Host zeichnet lediglich lokale Bounds vor
dem Flagschreiben auf. Pro Tick wird Cachebyte +61 vom Diagnoseaufrufer gelöscht,
was noch keine rekonstruierte natürliche Cacheinvalidierung ist.

`Record-PoseBounds.py` prüft alle 964 Box-/Kugelergebnisse und die
Publikations-/Flagsreihenfolge unabhängig bitgenau unter diesen Math-Policies.
Skalierungs-, Newton- und Divisorkonstanten werden aus den Original-DLL-Bytes
gelesen und gegen die ASM-Verwendung geprüft. Fünf Tests sichern zusätzlich
Move-Ausschluss, leere Punktmenge, NaN, Nullausdehnung und beide Hostfehlergrenzen.
Insgesamt 281 Tests, Clippy und Formatprüfung bestanden.

Die Actor-Welttransformation ist anschließend als Snapshot-Adapter ergänzt:
[SKELETAL_WORLD_BOUNDS.md](SKELETAL_WORLD_BOUNDS.md).

Offen bleiben Runtimepolsterungs-Loader, natürliche Actor-/Matrixbindung, Directors,
native RSQRTSS-Präzision, vollständige Pose-Vorbereitung und Skinning.
Androidprüfung folgt am Projektende.
