# Namensbindung für SetBonePlace

`SnapshotBonePlaceNames` verbindet den bereits rekonstruierten SetBonePlace-Einstieg mit der gemeinsamen Referenzknochensuche. Die globale Namenstabelle liefert opake FName-Identitäten; Package-Indizes werden nicht direkt als globale Identitäten verwendet.

Die Originalfunktion `10500410` in engine.dll behandelt den Nullnamen sofort als Fehlschlag. Bei anderen Namen gewinnt der erste passende Alias. Seine Zielidentität wird genau einmal ersetzt, ohne rekursive Aliasauflösung; anschließend gewinnt der erste Referenzknochen mit dieser Identität. Bei angefordertem Matrixausgang kopiert der Alias seine 16 Matrixwörter bereits vor der Knochensuche, auch wenn diese anschließend keinen Treffer findet. Ohne Alias bleibt der Ausgang unverändert. Die zusätzliche Rust-Prüfung verlangt einen Matrixsnapshot nur, wenn tatsächlich kopiert wird.

Ungültige globale Indizes liefern einen Fehler. Die bereits belegte Reihenfolge in SetBonePlace bleibt erhalten: Ein Fehler bei der Zielauflösung lässt das Cachebyte unverändert; ein Fehler bei der späteren Anfangsknochenauflösung tritt nach der Invalidierung auf. Aktualisierungen vorhandener Directors erhalten deren History.

`rc-bone-name-check` und `Record-BoneNames.py` prüfen 130 Originalskelette mit 3.111 Knochen, 3.631 Abfragen, 260 Alias-Matrixkopien und 3.241 Anlage-/Aktualisierungspaaren. Die unabhängige Python-Prüfung rekonstruiert Namenstabelle, Aliasauflösung und vollständige Director-Wörter. Sechs zusätzliche Tests behandeln Duplikate, nicht rekursive Aliasfolgen, Nullnamen, optionale Matrixausgänge und Fehlerreihenfolge. Der Meilenstein bestand 336 Workspace-Tests, Clippy und Formatprüfung.

Die globale Tabelle verwendet die früher belegten festen Indizes und eine ausdrücklich diagnostische, sortierte Vergabe dynamischer Indizes. Aliasnamen und Aliasmatrizen sind Diagnosesnapshots. Die native dynamische Registrierung, das Lesen der Original-Aliasarchive und die Script-/Actor-Anbindung sind weiterhin offen. Das bekannte Legacy-Mesh ohne unterstütztes Präfix wird ausdrücklich ausgeschlossen.

Nachweise: `analysis/decompiled/skeletal-match-ref-bone.c/.asm`, `analysis/reports/bone-names.json`, `bone-names-validation.json`, `analysis/evidence.json`. Fortsetzung: [Inverser Referenzpose-Cache](SKELETAL_REFERENCE_CACHE.md). Androidprüfung am Projektende.
