# Vorbereiteter Root-only-Posezweig

`skeletal_root_pose.rs` ergänzt den Root-only-Zweig von USkeletalMeshInstance.ApplyAnimation und den Originalkonstruktor FMatrix(FQuat,FVector). Die Trackdaten und Schlüsselinterpolation bleiben offene Hostoperationen. Es wird keine bereits vollständige oder sichtbare Animation behauptet.

## GetFrame und Vorbereitung

Die Skelett-Vtable `1066a5c8`, Slot +dc, zeigt auf GetFrame (`1050b330`). Die Funktion leitet Actor und den Vergleich `flags == 3` direkt an ApplyAnimation (`10509dc0`) weiter. Genau dieses Flag verwendet der vorher untersuchte absolute GetMoveCoords-Aufruf. Die übrigen GetFrame-Argumente werden von diesem nativen Wrapper nicht ausgewertet.

ApplyAnimation führt vor dem Root-only-Zweig mehrere Schritte aus: Pose-/Matrixpuffer an die Knochenanzahl anpassen, ungültige Cachezustände markieren, MeshToWorld übernehmen, bei fehlendem inversen Referenzcache diesen aufbauen und Animations-Linkups aktualisieren. Diese Vorbereitung ist exportiert, aber noch nicht portiert. `apply_root_prepared` beginnt ausdrücklich danach. Der Caller muss vorbereitete Daten liefern; die Diagnose liefert diese Voraussetzung, statt eine komplette ApplyAnimation-Ausführung vorzutäuschen. Director-Anlage und -Aktualisierung erfolgen über den separat rekonstruierten [SetBonePlace-Einstieg](SKELETAL_SET_BONE_PLACE.md).

## Root-Auswahl und Samplinggrenze

Zuerst werden Quaternion und Position des ersten Referenzknochens in den Root-Pose-Snapshot kopiert. Danach prüft die Funktion alle Kanäle in Reihenfolge:

1. IsChannelActive(index, −1) muss true liefern.
2. Instanzwort +11c muss null sein.
3. Kanal-Boneindex +3c muss null und Intervallende +40 positiv sein.
4. GetSequence muss eine Sequenz liefern.
5. Deren Trackanzahlwort bei +60 muss nach Maske 0x1fffffff ungleich null sein.
6. Das erste Root-Linkup muss einen nicht negativen Trackindex liefern.

Der Root-only-Zweig prüft an dieser Stelle weder das Loop-Byte noch eine nicht null Rate. Dies unterscheidet ihn von der vorgeschalteten GetMoveCoords-Auswahl. Der Adapter erhält diese native Reihenfolge.

Der Kanalframe +1c wird auf [0,1] begrenzt. Ungeordnete NaN-Vergleiche führen anhand des Originalassemblers zu 0. −0 bleibt als Bitmuster erhalten. Anschließend berechnet das Original mit x87 `Frameanzahl * normalisierter Frame`, speichert das Ergebnis als f32 und ruft FSkelAnimSeq.GetRotPos (`10500fc0`) mit Trackindex und Root-Outputreferenzen auf.

Der Rust-Host erhält deshalb Frameanzahl und normalisierten Frame getrennt. Native x87-Präzision, Track-Decodierung, Schlüsselwahl und Quaternion-/Positionsinterpolation bleiben ausdrücklich seine Aufgabe. Eine mögliche Editoraktualisierung der Sequenzreferenz kann im Host bereits Kanalwörter ändern.

Mehrere geeignete Kanäle überschreiben den Root nacheinander; das letzte gelieferte Sample gewinnt. Nach erfolgreicher Schleife wird genau aus diesem Root eine Matrix erzeugt. Ein Fehler an der Samplinggrenze erhält bereits geschriebene Rootwerte, aktualisiert aber die abschließende Rootmatrix nicht. Die Root-only-Funktion verlässt ApplyAnimation vor den Vollpose-Schreibzugriffen auf Instanzbytes +60/+61/+179. Die gelieferten Snapshotbytes +60/+61 bleiben unverändert.

## Native Quaternion-/Translationsmatrix

FMatrix(FQuat,FVector) liegt in core.dll bei `10144650`. Der Konstruktor ist anhand seiner SSE-Instruktionen in Rust übernommen. Er normalisiert die Quaternion nicht. Insbesondere ergibt eine nicht normierte Quaternion auch die entsprechenden nicht orthonormalen Matrixwerte.

Die Operationen folgen der belegten Reihenfolge: Quaternionkomponenten verdoppeln, daraus einzelne Produkte bilden, anschließend die ursprünglichen Additionen und Subtraktionen durchführen. Eine algebraisch anders gruppierte Standardformel wird nicht verwendet. Die Konstanten 1 und 2 wurden direkt aus core.dll geprüft. Die drei Translationswörter werden unverändert kopiert, einschließlich −0, Infinity oder NaN-Payload. Die drei homogenen Nullfelder und der abschließende Wert 1 sind fest.

Die Matrix ist als 16 Wörter in ursprünglicher Speicherreihenfolge beschrieben. Sie ist noch keine Skinning- oder Renderer-Ausgabe. CPU-Sondermodi für Denormalbehandlung, FP-Ausnahmen und Fast-Math werden nicht emuliert.

## Prüfung und Grenzen

Die Probe übernimmt die 768 bisherigen NewSequence-Kanalsnapshots und erzeugt je neun ausdrücklich gelieferte Varianten. Referenzroot, Blockierwort, Boneintervalle, Trackanzahl, Linkup und Samplingresultate sind Diagnosedaten. Die Quellenzuordnung bleibt nachvollziehbar.

| Ergebnis | Fälle |
|---|---:|
| Referenzroot ohne Sample | 4.608 |
| Root mit geliefertem Sample | 1.536 |
| Offene Track-Decodierung/Interpolation | 768 |

Vier zusätzliche Matrixfälle prüfen Identität, nicht normierte Achsenquaternion und zwei allgemeine Quaternionen. Die unabhängige Prüfung berechnet alle Root-/Matrix-/Kanalwörter und Ereignisse nach, einschließlich der erhaltenen Matrix bei Samplingfehlern. Bei den allgemeinen Matrizen wird jede einzelne Operation auf f32 gerundet. Frühere Berichte bleiben unverändert.

```text
cargo run -p rc-inspect --bin rc-skeletal-root-probe -- analysis/reports/channel-playback.json analysis/reports/skeletal-root-pose.json
python scripts/Record-SkeletalRootPose.py
```

Nachweise: `analysis/reports/skeletal-root-pose.json`, `skeletal-root-pose-validation.json`, `analysis/decompiled/skeletal-get-frame.c`, `skeletal-apply-animation.c`, `skeletal-root-frame.asm`, `quat-translation-matrix.c/.asm` und `analysis/evidence.json`.

234 Workspace-Tests (224 rc-package, 4 rc-native, 6 rc-render), Clippy mit Warnungen als Fehler und Formatprüfung bestanden. Fünf neue Tests prüfen Matrixwerte und Translationbits, Root-Gates, getaggte Trackanzahl, Frameclamp/NaN/−0, Loop-/Rate-Sonderverhalten, letzte Samples, Teilschreibzugriffe und erhaltene Instanzflags.

Die obigen Ergebnisse dokumentieren den damaligen Root-only-Meilenstein. Inzwischen sind Originalskelette und Tracks, GetRotPos, vorbereitete Kanal-/Director-Poseauswertung, Matrixinverse/-komposition und Bounds in separaten Modulen rekonstruiert. Der [inverse Referenzpose-Cache](SKELETAL_REFERENCE_CACHE.md) ergänzt jetzt die Cachevorbereitung einschließlich Move-Elternverknüpfung. Die vollständige Verbindung dieser Module im ApplyAnimation-Ablauf, weitere Puffer-/Linkup-Vorbereitung, Skinning und sichtbare Animation bleiben offen. Androidprüfung weiterhin zum Schluss.

Fortsetzung: [GetRotPos-Schlüsselwahl und Positionen](SKELETAL_TRACK.md) implementiert jetzt Zeitabschnitte, direkte/Singleton-Schlüssel, Positionsdecodierung/-interpolation und geordnete Linkup-Suche für gelieferte Tracks. Quaternion-Decodierung/Slerp und x87-Framezeit bleiben Hostgrenzen; die obigen 234 Tests und Berichte beschreiben weiterhin diesen früheren Meilenstein.
