# Mesh-Animation: Eintritt und Kanalauswahl

Folgemeilenstein: [LOD-Kanalwiedergabe](CHANNEL_PLAYBACK.md) ergänzt die Schreibzugriffe nach der Kanalauswahl auf gelieferten Sequenz-/Kanalsnapshots. Die hier dokumentierte frühere Probe bleibt unverändert.

`mesh_animation.rs` ergänzt die geprüften Eintrittsbedingungen von ULodMeshInstance.PlayAnim, die Skelett-Kanalauswahl und MatchRefBone mit einem null Output-Matrixzeiger. Die Eingaben sind ausdrücklich gelieferte Laufzeitsnapshots. Ein Decoder für originale Skelett-/AnimSeq-Pakete und die eigentliche Animationswiedergabe sind weiterhin offen.

## Virtuelle Ziele im Original

Die exportierten Vtables wurden direkt aus engine.dll gelesen:

| Klasse / Vtable | GetActor +9c | PlayAnim +b0 | GetChannel +f8 | PostInitAnim +104 |
|---|---|---|---|---|
| ULodMeshInstance / 1066a2b0 | 103b0730 | 10451100 | 103b05f0 | 103b0780 |
| USkeletalMeshInstance / 1066a5c8 | 103b0730 | 10451100 | 10509540 | 104ffde0 |
| UVertMeshInstance / 1067bd88 | 103b0730 | 10451100 | 1055f890 | 103b0780 |

UMeshInstance.PlayAnim (`103b0450`) liefert false, dessen GetChannel (`103b05f0`) null. Die drei oben genannten Klassen verwenden dieselbe LOD-PlayAnim-Funktion, aber unterschiedliche Kanalziele. GetActor liest die Actorreferenz bei Instanzoffset +58. Das beweist die Ziele dieser konkreten Originalklassen, nicht die dynamische Klasse einer bislang nicht geladenen Spielinstanz oder weitere Overrides.

UVertMeshInstance.GetChannel ruft einen Arrayhelfer mit (1,1) auf und liefert den Anfang des Kanalarrays. Dieser Helfer und Vertex-Kanäle wurden in diesem Meilenstein nicht portiert. UMesh.MeshGetInstance (`104577a0`) wurde ebenfalls exportiert: Wiederverwendung, Freigabe und Konstruktion von Instanzen bleiben separate offene Arbeit.

## Eintritt in die LOD-Wiedergabe

`lod_play_anim` führt die belegten Aufrufe in dieser Reihenfolge aus:

1. Mesh-Sequenzsuche über virtuell +ac, mit Sequence-Handle und `!GIsEditor`.
2. Wenn die Sequenz fehlt und Sequence nicht None ist: Warncache-/Loggrenze, dann natives false (`MissingSequence`).
3. Actor über virtuell +9c abfragen; null ergibt false (`NoActor`).
4. Kanal über virtuell +f8 abfragen; null ergibt false (`NoChannel`).
5. Fortsetzung an der offenen Kanalwiedergabegrenze.

Eine nicht gefundene None-Sequenz passiert die erste Abbruchprüfung. Die Sequenzsuche wird auch für None ausgeführt. Der Rust-Host liefert das Ergebnis der Suche, die Actorreferenz und die ausgewählte Kanalimplementierung; hier werden keine originalen Clips oder Instanzen daraus abgeleitet. Die globale Fehlsequenz-Warnunterdrückung bleibt eine explizite Hostoperation. Hostfehler halten vor dem nächsten Aufruf an.

Die native Fortsetzung verändert unter anderem Frame, Rate, Blendwerte und Loopflag. Bei Neuinitialisierung folgen PostInitAnim und AActor.ReplicateAnim, bevor ein Instanzflag gelöscht und true zurückgegeben wird. Diese Fortsetzung ist exportiert, aber hier noch nicht ausgeführt. Eine bereits erfolgte Kanaleinfügung bleibt im Snapshot sichtbar, wenn die Fortsetzung offen ist; es gibt keinen erfundenen Rollback.

## Skelett-Kanalauswahl

USkeletalMeshInstance.GetChannel (`10509540`) verwendet den gelieferten Bone-Handle. Ist er None oder existiert noch kein Kanal, liest die Funktion stattdessen den ersten Referenzknochen des Meshes. Deshalb wird beim ersten Kanal selbst eine explizite Boneanforderung durch den Root ersetzt. Ein fehlender/leer gelieferter Skeletonsnapshot an diesem nativen Dereferenzpunkt ergibt einen ausdrücklichen Fehler.

Danach wird der erste Kanal mit identischem Bone-Handle und identischer vorzeichenbehafteter Kanalnummer wiederverwendet. Dabei werden sämtliche 18 Wörter erhalten, und MatchRefBone wird nicht aufgerufen. Ein solcher Treffer kann deshalb bei explizitem Bone auch ohne Skeletonsnapshot bestimmt werden. Ein vorhandenes Aliasziel wird nicht als derselbe Bone-Handle behandelt.

Fehlt der Kanal, sucht MatchRefBone (`10500410`) zunächst einmal in der geordneten Aliastabelle nach dem angeforderten Handle. Der erste Treffer ersetzt den Suchhandle. Es gibt keine rekursive Aliasauflösung. Anschließend wird der erste passende Referenzknochen gesucht. Ein anfänglicher None-Handle liefert −1. Der optionale Matrixoutput des Originals wird von GetChannel als null übergeben und ist hier nicht modelliert.

Ein neuer Kanal wird vor dem ersten Kanal mit größerer Kanalnummer eingefügt; bei gleicher Kanalnummer entscheidet der größere Referenzknochenindex. Es handelt sich um vorzeichenbehaftete Vergleiche. Der Originalcode verschiebt bestehende 0x48-Byte-Einträge und nullt genau 18 Wörter. Nur diese Felder werden anschließend beschrieben:

| Offset | Inhalt |
|---|---|
| +08 | Ursprünglicher ausgewählter Bone-Handle, vor Aliasauflösung |
| +0c | Gelieferte Kanalnummer |
| +3c | Gefundener Referenzknochenindex |
| +40 | Rohwort des Referenzknochens bei +38, plus 1, plus Knochenindex |

Die Addition bei +40 erhält das native 32-Bit-Wrapping. Die Bedeutung des Bone-Rohworts +38 wird nicht weiter behauptet. Alle übrigen neuen Kanalwörter bleiben null. Bestehende Payloads bleiben bei Verschiebung unverändert. Rust-Vecs ersetzen nur die nachvollziehbare Snapshotoperation; native Arraytags, Allokator-ABI und Speicherfehler werden nicht emuliert.

## Prüfung und Grenzen

Die Probe verarbeitet alle 256 früher aufbereiteten AnimProp-Anforderungen mit je sieben ausdrücklich synthetischen Szenarien. Sequenz-/Actor-Ergebnisse, Root/Bone-Handles und Kanalinhalte sind Diagnoseeingaben. Originale Klassennamen und Requestzuordnung bleiben aus dem früheren Bericht erhalten; synthetische Bone-Handles sind keine Originalpaket-Namensbindungen. Bei den Kanaltests werden Bone/Channel bei Bedarf ausdrücklich überschrieben.

| Szenario | Fälle | Resultat |
|---|---:|---|
| MissingSequence | 256 | Abbruch vor Actor-/Kanalauswahl |
| NoActor | 256 | Abbruch vor Kanalauswahl |
| NoChannel | 256 | None-Root liefert keinen Match |
| CreateRoot | 256 | Root neu eingefügt, Fortsetzung offen |
| ReuseRoot | 256 | Kanal vollständig erhalten, Fortsetzung offen |
| InsertAlias | 256 | Aliaskanal vor bestehendem Kanal eingefügt, Fortsetzung offen |
| UnknownBone | 256 | Kein Match, Kanäle unverändert |

Damit ergeben sich 512 eingefügte und 256 wiederverwendete Kanäle. Die unabhängige Pythonprüfung berechnet jeden Fall einschließlich aller 18 Kanalwörter, Ereignisreihenfolge, Eingangsparameter und Originalrequestzuordnung nach. DLL-Bytes prüfen die Basisklassenstubs und GetActor; drei exportierte Vtables und die maßgeblichen Assemblerstellen werden ebenfalls verglichen. Frühere Berichte werden nur gelesen.

```text
cargo run -p rc-inspect --bin rc-mesh-animation-probe -- analysis/reports/animation-calls.json analysis/reports/mesh-animation-channels.json
python scripts/Record-MeshAnimationChannels.py
```

Nachweise: `analysis/reports/mesh-animation-channels.json`, `mesh-animation-channels-validation.json`, `analysis/decompiled/mesh-animation-entry.c/.asm`, `skeletal-animation-channel.c/.asm`, `skeletal-bone-match.c/.asm` und `mesh-animation-channel.c`.

210 Workspace-Tests (200 rc-package, 4 rc-native, 6 rc-render), Clippy mit Warnungen als Fehler und Formatprüfung bestanden. Sieben neue Tests prüfen Root-Fallback, unveränderte Wiederverwendung, Aliasreihenfolge ohne Rekursion, signierte Einfügeordnung, erhaltene Payloads, Wrapping, None-/Editor-/Actor-/Kanal-Gates und Fehlergrenzen. Echte Skelette, Sequenzdaten, Frame-/Blendfortsetzung, Pose-/Skinning-Ausgabe, Replikation und Instanzerzeugung bleiben offen. Androidprüfung weiterhin zum Schluss.
