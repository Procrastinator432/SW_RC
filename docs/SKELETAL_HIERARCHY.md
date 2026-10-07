# Knochenhierarchie und Elternmatrizen

`crates/rc-package/src/skeletal_hierarchy.rs` ergänzt die Hierarchievorbereitung aus USkeletalMesh.PostLoad (`10512860`) und den lokalen Matrix-/Elternschritt aus ApplyAnimation (`1050a730` ff.). Die Eingabe ist eine bereits bestimmte lokale Quaternion-/Positionspose. Kanalblending, Director-Tracks, Bounds, Cacheflags und Skinning sind nicht Bestandteil dieses Schritts.

## PostLoad

Das Original vergleicht den Namen des ersten Referenzknochens mit dem globalen FName „Move“. Bei einem Treffer setzt es Mesh +1c0 auf 0, sonst auf -1. Unabhängig vom gespeicherten Wert setzt es den ersten Parentindex bei Bone +34 auf -1.

Für jeden Knochen berechnet PostLoad Tiefe (Bone +3c) und Nachkommenzahl (Bone +38) neu, indem es die Parentkette nach oben durchläuft. Damit ist nun belegt, dass +34 der Parentindex und +38 die Nachkommenzahl ist; die vorherigen Decoder erhielten diese Wörter bewusst ohne Interpretation. Die Kanalintervallberechnung kann dadurch später auf die neu berechneten Werte angeschlossen werden. Die Rohdaten früherer Berichte werden unverändert erhalten.

`prepare_hierarchy` gibt Parentindizes, Tiefen, Nachkommenzahlen und den Move-Index separat zurück. Parent-Vorwärtsverweise, ungültige Indizes und zyklische Snapshots werden ausdrücklich abgewiesen, statt native ungültige Speicherzugriffe oder Endlosschleifen nachzubilden. Alle 130 geprüften Originalskelette besitzen eine passende Parent-vor-Kind-Reihenfolge. Die Namensprüfung verwendet ASCII-Case-Insensitivität; globale FName-Zuweisung ist weiterhin ein separater Schritt.

## Matrixverknüpfung

Zuerst wird für jeden Knochen aus seiner lokalen Quaternion und Position der bereits rekonstruierte native Matrixkonstruktor verwendet. Rootindex 0 bleibt lokal. Für weitere Knochen multipliziert das Original die lokale Matrix mit der bereits ausgewerteten Parentmatrix.

Eine Sonderregel betrifft „Move“: Außerhalb des Editors bleiben direkte Kinder dieses Move-Knochens lokal; dessen Bewegung wird nicht in ihre Matrizen übernommen. Im Editor wird auch diese Verbindung ausgeführt. Bei gewöhnlichen Skeletten mit Move-Index -1 findet die normale Parentverknüpfung statt. Ein negativer Parent, der diese native Skip-Bedingung nicht erfüllt, wird im Rustadapter als Fehler ausgewiesen.

Der native Code berechnet die sechzehn Produkte mit unterschiedlichen SSE-Additionsreihenfolgen je Ausgabeelement. Eine Standardmatrixmultiplikation mit einheitlicher Summenfolge kann deshalb andere Floatbits liefern. `compose_bone_matrix` übernimmt die belegten Reihenfolgen, ohne FMA oder algebraische Umgruppierung. Getrennte f32-Operationen bilden den untersuchten normalen Floatpfad ab; NaN-Payload-Auswahl, Ausnahmeflags und Denormalmodi werden nicht emuliert.

`build_pose_matrices` schreibt zunächst die lokale Matrix des aktuellen Knochens und greift erst danach auf den Parent zu. Bei einem Fehler bleiben vorherige Matrizen und die bereits geschriebene aktuelle lokale Matrix erhalten. Eine unpassende Pose-/Hierarchielänge schlägt vor den Matrixschreibzugriffen fehl. Der Adapter beansprucht keine native Speicherallokator- oder Cachevorbereitungsemulation.

## Prüfung mit Originaldaten

Die Diagnose übernimmt die zuvor unabhängig geprüften Originalskelette und Linkups. Sie erzeugt für jedes der 130 Meshes eine Referenzpose im Spiel- und Editorzweig. Zusätzlich wird je Linkup die erste originale Sequenz bei Zeit 0, Frameanzahl/2 und Frameanzahl für alle zugeordneten Knochen decodiert. Fehlende Zuordnungen erhalten den Referenztransform. Dies ist eine ausdrücklich gewählte vollständige lokale Trackpose, kein bereits vollständiger ApplyAnimChannel-Lauf.

| Geprüfte Daten | Anzahl |
|---|---:|
| Diagnoseposen | 935 |
| Knochenmatrizen | 28.656 |
| Meshes mit Move-Root | 53 |
| Gesampelte lokale Knochen | 22.395 |
| Referenzfallbacks bei fehlendem Track | 39 |
| Zusätzliche allgemeine Matrixfälle | 32 |

Die unabhängige Pythonprüfung rekonstruiert Hierarchie und Move-Regel aus dem früheren Originalbericht. Sie liest die verwendeten Animationstracks erneut aus den Paketbytes, vergleicht lokale Positionen exakt und portable Quaternionen mit einer mathematischen Referenz. Der größte Komponentenfehler in diesen Fällen beträgt etwa `1,75e-7`, unter der Toleranz `2e-6`. Dies ist keine globale Fehlergarantie oder x86-Bitgleichheitsbestätigung.

Für die Parentmultiplikation interpretiert die Prüfung direkt die MOVSS/MOVAPS/MULSS/ADDSS-Ausdrucksbäume der Originalinstruktionen. Sie verwendet somit nicht die Rust-Reihenfolgentabelle als Orakel. Alle 28.656 Matrixausgaben sowie 32 allgemeine endliche 4×4-Matrixpaare werden bitweise geprüft. Frühere Reader-/Mathedefinitionen werden ohne Ausführung ihrer Prüf- oder Schreibabläufe wiederverwendet.

Fünf neue Tests prüfen Rootparent/Move/Tiefen/Nachkommen, ungültige und zyklische Parents, Editor-/Spiel-Unterschied, gewöhnliche Parentketten, Teilschreibzugriffe und einen Auslöschungsfall, der die native Summenreihenfolge von einer Standardmultiplikation unterscheidet. Insgesamt 262 Workspace-Tests (252 rc-package, 4 rc-native, 6 rc-render), Clippy mit Warnungen als Fehler und Formatprüfung bestanden.

```text
cargo run -p rc-inspect --bin rc-skeletal-pose-check -- analysis/reports/original-skeletal-linkups.json analysis/reports/original-skeletal-poses.json
python scripts/Record-SkeletalPoses.py
```

Nachweise: `analysis/decompiled/skeletal-postload-hierarchy.c/.asm`, `skeletal-root-frame.asm`, `skeletal-apply-channel.c`, `analysis/reports/original-skeletal-poses.json`, `original-skeletal-poses-validation.json` und `analysis/evidence.json`. Der untersuchte ApplyAnimChannel-Quelltext ist eine Grundlage für die nächste Blendingarbeit; sein vollständiger Ablauf wird hier noch nicht als portiert gezählt.

Offen bleiben übrige SkeletalMesh-/Skinningdaten, native Kanalblend-/Director-/Bounds-Auswertung, vollständiges ApplyAnimation, sichtbare Animation und die Legacy-Konvertierung. Androidprüfung bleibt am Schluss.
