# Animationspuffer und Instanzvorbereitung

Stand: 2026-10-08. `skeletal_preparation` rekonstruiert die Instanzvorbereitung in ApplyAnimation bei `10509e7d..1050a4b6` und das spätere Scratch-Gate bei `1050a62e..1050a671`. Damit sind die Inhalte der Größenänderungen und die Verbindung zum inversen Referenzcache geprüft.

## Instanzpuffer

Das Original hält Quaternionen (+f8/+fc), Positionen (+100/+104) und Matrizen (+a0/+a4) in getrennten Arrays. `InstanceAnimationBuffers` bildet sie getrennt ab, damit auch unterschiedliche Ausgangsgrößen darstellbar bleiben.

Wenn Quaternion- oder Positionsanzahl von der Referenzknochenanzahl abweicht, setzt der Einstieg +61 auf 0 und passt beide lokalen Arrays an. Eine abweichende Matrixanzahl invalidiert +61 ebenfalls und passt das Matrixarray an. Stimmen alle Anzahlen, bleiben Inhalt und Cachebyte unverändert.

Die exportierten Helfer `103c3c70`, `103346b0` und `103c3b90` erhalten den bisherigen Präfix. Beim Vergrößern wird ausschließlich der Anhang mit Nullwörtern initialisiert. Beim Verkleinern bleibt der Präfix erhalten. Ein neu angehängtes Quaternion enthält somit vier Nullwörter; es wird hier nicht zur Identitätsrotation gemacht. Vorhandene NaN-Payloads, Vorzeichenbits und Matrixwörter bleiben unverändert.

Die tieferen Array-Helfer belegen Präfixkopie, Entfernen des hinteren Bereichs und die ursprünglichen Zähler-/Besitzflags. Das Rust-Modell übernimmt den gültigen Speicherinhalt und die logische Länge; native Heapkapazitäten, Besitzflags, Reallokationsadressen und Allokationsfehler werden nicht emuliert. Beschädigte negative native Arrayzähler sind keine gültige Eingabe dieser Vec-basierten Schnittstelle.

## Scratch und Reihenfolge

`prepare_channel_scratch` wird im Original erst nach dem Vollpose-Cachegate ausgeführt. Es prüft nur, ob die Quaternionanzahl kleiner als die Knochenanzahl ist. Nur dann werden beide Scratcharrays auf die Knochenanzahl gebracht. Das Positionsarray kann dabei verkleinert werden. Bei ausreichend vielen Quaternionen wird auch ein zu kurzes Positionsarray unverändert gelassen. Der Scratch wird weder pauschal geleert noch mit der Referenzpose gefüllt.

`prepare_animation_instance` verbindet diese Schritte in der belegten Reihenfolge:

1. Instanzpuffer anpassen und gegebenenfalls +61 invalidieren.
2. MeshToWorld über den Host der virtuellen Methode +e8 abfragen und speichern.
3. Den inversen Referenzpose-Cache bei Bedarf aufbauen.
4. Alle Mesh-Linkups in aufsteigender Reihenfolge über den Host aktualisieren.

Diese Vorbereitung liegt vor der Entscheidung zwischen Root-only- und Vollpose-Pfad. Auch ein gesetztes Pose-Cachebyte überspringt deshalb weder die Transformabfrage noch die Linkup-Aufrufe. Ein vorhandener inverser Referenzcache bleibt entsprechend seinem eigenen Gate erhalten.

Fehler an einer Hostgrenze erhalten die zuvor abgeschlossenen Schreibzugriffe: Bei Transformfehlern bleiben Größenänderungen erhalten, aber der alte Transform und der inverse Cache unverändert. Ein Fehler beim Referenzcache erhält den bereits gespeicherten Transform und dessen Teilcache. Ein Linkupfehler erhält die bisherigen Linkup-Aufrufe und den aufgebauten Referenzcache. Diese Grenzen bilden kontrollierte Rust-Fehler ab; native ungültige Speicherzugriffe werden nicht nachgestellt.

## Nachweis und verbleibende Arbeit

`rc-preparation-check` und `Record-AnimationPreparation.py` prüfen unabhängig 2.592 Instanzzustände und 216 Scratchzustände, einschließlich aller Größenkombinationen von 0 bis 5. Zusätzlich werden 130 Originalskelette mit 3.111 Referenz-Inversen vorbereitet und erneut aufgerufen. Alle Pufferwörter, Transformwerte, Cachebytes und 450 geordneten Linkup-Hostaufrufe stimmen. Die inversen Matrizen entsprechen dem bereits unabhängig anhand der Original-ASM geprüften Referenzcache-Bericht.

Neun zusätzliche Tests prüfen Größen-Gates, Präfix-/Null-Erhalt und Fehlerreihenfolge. **356 Workspace-Tests**, Clippy mit `-D warnings` und Formatprüfung bestanden.

Der damalige Diagnosehost liefert einen Transform und protokolliert die Linkup-Aufrufe. Die [konkreten Animationshosts](SKELETAL_RUNTIME_HOSTS.md) führen inzwischen die rekonstruierte Linkup-Aktualisierung tatsächlich aus und teilen deren Mappingzustand mit Root- und Kanal-Sampling. Der Transform bleibt geliefert. Der [Vollpose-Einstieg](SKELETAL_ANIMATION_ENTRY.md) und der [Root-only-Einstieg](SKELETAL_ROOT_ENTRY.md) verwenden dieselbe Instanzvorbereitung und getrennte Originalarrays. Dynamische Actor-Anbindung, echtes Ticking, Skinning und sichtbare Animation bleiben offen. Androidprüfung am Projektende.

Nachweise: `analysis/decompiled/skeletal-buffer-resize.c/.asm`, `skeletal-buffer-storage.c`, `skeletal-root-frame.asm`, `analysis/reports/animation-preparation.json`, `animation-preparation-validation.json`, `preparation-tests.log` und `analysis/evidence.json`.
