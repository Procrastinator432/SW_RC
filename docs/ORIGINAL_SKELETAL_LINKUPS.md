# Originales Referenzskelett und Track-Linkups

`crates/rc-package/src/skeletal_mesh.rs` liest originale SkeletalMesh-Präfixe bis einschließlich Referenzknochen und gespeicherter Animationsreferenzen. Es implementiert außerdem den nativen Linkup-Aufbau bei `10509430`. Unterstützt sind SWRC-Paketversionen 151–159/Licensee 1 mit internem LOD-Archivformat 5–8; im geprüften Bestand verwenden alle unterstützten Meshes LOD-Version 8. Der übrige Mesh-Payload bleibt ausdrücklich ungelesen.

## Archivformat

Vor den Knochen stehen UPrimitive-Bounds, LOD-Felder und -Arrays sowie ein Positionsarray. Die Arrays werden anhand der rekonstruierten Archivelementgrößen begrenzt durchlaufen. Das bedeutet noch keine Geometrie-, Material- oder Skinningauswertung. Insbesondere beträgt die gespeicherte Größe eines LOD-Wedge zehn Bytes, obwohl sein Laufzeitelement zwölf Bytes belegt.

Persistente Paketarchive lassen die transiente UMesh-Referenz bei +54 und die gecachten Bone-to-Track-Arrays aus. Die entsprechenden Zweige prüfen Archivbyte +15. Diese Daten werden beim Lesen deshalb nicht aus benachbarten Paketbytes erfunden. Gespeicherte Linkups enthalten lediglich das Wort +0 (ab Paketversion 152; davor Standardwert 1) und die Animationsobjektreferenz. Nullreferenzen werden im Snapshot erhalten; das native nachgeschaltete Entfernen wird hier nicht ausgeführt.

Jeder Referenzknochen enthält Name, Flags, Quaternion, Position und weitere originale Wörter. Sein Laufzeitlayout umfasst 64 Bytes. Die Transformationsserialisierung schreibt dasselbe Feld +28 zweimal: Der zweite gelesene Wert ersetzt den ersten im nativen Objekt. Der Decoder erhält beide Archivwerte als `word_28_first` und `word_28`. Die abschließenden Wörter werden in Archivfolge +38 und +34 gelesen. Ihre weitergehende Interpretation bleibt offen; +38 wird bereits im belegten Kanalintervall verwendet.

Alle Transformationskomponenten bleiben rohe Floatbits. Die Quellen sind USkeletalMesh.Serialize (`10514080`), ULodMesh.Serialize (`10450d40`), UMesh.Serialize (`104576f0`), UPrimitive.Serialize (`104994d0`) und die zugehörigen Bone-/LOD-/Linkup-Hilfsfunktionen. Bone-Feldfolge, wiederholter Zugriff und Linkup-Auswahl sind zusätzlich anhand des Originalassemblers geprüft.

## Native Linkup-Semantik

`refresh_linkup` prüft zuerst nur die Länge des bestehenden Mappings gegen die Mesh-Knochenanzahl. Gleiche Länge beendet den Ablauf sofort, auch wenn sich Namen oder Animation geändert haben. Bei ungleicher Länge und fehlender Animation bleibt das Mapping erhalten. Ansonsten wird für jeden Mesh-Knochen der erste namensgleiche Animationseintrag gesucht; fehlende Treffer werden -1. Gibt es insgesamt keinen Treffer, wird die native Warnungsbedingung als Ergebnis ausgegeben. Aliase und eine automatische Root-Track-Annahme werden dabei nicht verwendet.

Der native Vergleich verwendet globale FName-Handles. Paketnamensindizes dürfen daher nicht direkt über Paketgrenzen verglichen werden. Die Bestandsprüfung löst Namen zuerst auf und verwendet die ASCII-Kleinschreibung als kanonische Identität; alle verwendeten Namen werden auf ASCII geprüft. Nicht-ASCII-Namen würden ausdrücklich abgewiesen, bis die originale Namensfaltung belegt ist. Dies erzeugt noch keine globalen Engine-FName-Handles.

## Originalbestand und Prüfung

130 unterstützte SkeletalMesh-Exporte enthalten 3.111 Referenzknochen und 225 Animationsreferenzen. Alle 225 Referenzen werden über die Paketobjektpfade auf originale MeshAnimation-Exporte aufgelöst. Die Bestandsprüfung baut die in persistenten Paketen leeren Mappingarrays neu auf. Fehlende Knochen bleiben fehlend; Root-Sampling findet ausschließlich bei einem tatsächlich zugeordneten ersten Mesh-Knochen statt.

8.172 Samples verwenden den daraus ermittelten Root-Track und Zeitpunkte 0, Frameanzahl/2 und Frameanzahl. Die Referenzrotation/-position stammen aus dem originalen ersten Mesh-Knochen. Die Samples durchlaufen komprimierte Track-Decodierung, portable Quaternionberechnung und den zuvor rekonstruierten Matrixkonstruktor. Sie sind noch kein vollständiger ApplyAnimation-Lauf mit Cachevorbereitung, Kanalblend oder sichtbarer Pose.

Eine unabhängige Pythonprüfung liest die Skelett-Präfixe erneut aus den Paketbytes, vergleicht sämtliche Knochenfelder und Linkup-Referenzen und berechnet die geordneten Namenszuordnungen nach. Für alle 8.172 Root-Samples liest sie die ausgewählten Originaltracks erneut, prüft Schlüsselwahl, Positionswörter und Matrixwörter exakt sowie Quaternionen gegen eine mathematische Referenz mit Toleranz `2e-6`. Der größte gemessene Komponentenfehler beträgt etwa `1,16e-7`. Dies bestätigt die geprüften Samples, keine allgemeine x86-Bitgleichheit. Als Parsergrundlage werden nur die Definitionen des früher unabhängig geprüften Paketreaders geladen; dessen Prüf-/Schreibablauf wird nicht ausgeführt.

Fünf neue Tests prüfen erste Namenstreffer, fehlende Namen/Warnung, unveränderte Cachewerte bei gleicher Länge, fehlende Animation, leere Knochenlisten, doppelte Transformationsfelder, abschließende Feldfolge, sämtliche Knochenfixture-Trunkierungen und ungültige Namen. Insgesamt 257 Workspace-Tests (247 rc-package, 4 rc-native, 6 rc-render), Clippy mit Warnungen als Fehler und Formatprüfung bestanden.

```text
cargo run -p rc-inspect --bin rc-skeletal-linkup-check -- "D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Animations" analysis/reports/original-skeletal-linkups.json
python scripts/Record-SkeletalLinkups.py
```

Nachweise: `analysis/decompiled/skeletal-linkup-build.c/.asm`, `skeletal-reference-bones.c`, `skeletal-prefix-serializers.c`, `lod-prefix-arrays.c`, `lod-prefix-elements.c`, `skeletal-mesh-prefix.asm`, `analysis/reports/original-skeletal-linkups.json`, `original-skeletal-linkups-validation.json` und `analysis/evidence.json`. Die Originalpakete werden ausschließlich gelesen.

## Grenzen

Nach den gelesenen Präfixen bleiben insgesamt 47.362.213 Meshbytes ausdrücklich offen: unter anderem weitere Mesh-/Render-/Skinningdaten. Der ältere SkeletalMesh-Export aus `beast.ukx` Version 148 wird als `UnsupportedLegacyPackage` ausgewiesen. Der Prefixdecoder ist kein vollständiger SkeletalMesh-Loader. Parent-Hierarchieauswertung, Vollpose, Blend-/Skinningpfad, sichtbare Animation und native x87/RSQRTSS-Bitgleichheit fehlen weiterhin. Androidprüfung bleibt am Schluss.

Fortsetzung: [Knochenhierarchie und Elternmatrizen](SKELETAL_HIERARCHY.md) implementiert jetzt den PostLoad-Hierarchieschritt und die Matrixverknüpfung einschließlich Move-/Editor-Sonderregel. 935 Diagnoseposen mit 28.656 Knochenmatrizen sind unabhängig geprüft. Die hier beschriebenen früheren Root-Samples bleiben unverändert; Kanalblend, Director, Bounds und Skinning bleiben offen.
