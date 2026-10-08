# SetBonePlace: Director-Anlage und Aktualisierung

Stand: 2026-10-07. `skeletal_set_bone_place::set_bone_place` rekonstruiert
**engine 10509270, USkeletalMeshInstance::SetBonePlace(FBoneDirector const&)**.
Der Einstieg übernimmt 22 rohe Eingabeworte, die Knochenanzahl, den veränderbaren
Cache-Bytewert +61 und die Director-Liste. Neue Tracks bestehen aus 28 Worten.

## Originalablauf

Das obere Bit des **eingehenden** Zielworts +0 unterscheidet einen Namen von
einem numerischen Knochenindex. Bei gesetztem Bit wird der signed31-Payload als
globaler Name aufgelöst und durch `MatchRefBone` in einen Knochenindex übersetzt.
Ein nichtgefundener Name wird als `0x7fffffff` gespeichert und als signed31 -1
erkannt. Numerische Eingaben werden unmittelbar als signed31 ausgewertet.

Ist das Ziel negativ oder außerhalb der Knochenanzahl, liefert der Einstieg
`false`, ohne Cache oder Director-Liste zu verändern. Andernfalls wird **+61=0**
gesetzt, bevor der Anfangsindex +4 aufgelöst wird. Ein Fehler dieser späteren
Auflösung erhält daher die bereits erfolgte Cacheinvalidierung.

Der Anfangsindex kann ebenfalls mit einem Namen markiert sein. Bleibt sein
signed31-Wert negativ, fällt er auf den gültigen Zielindex zurück. Ein positiver
Anfangsindex wird hier nicht zusätzlich gegen die Knochenanzahl begrenzt; auch
ein zu großer Wert bleibt wie im Original erhalten.

Anschließend wird der **erste** vorhandene Track mit passendem signed31-Ziel
aktualisiert. Dabei kopiert das Original genau **22 Worte / 88 Bytes**. Die
sechs anschließenden Worte bleiben vollständig erhalten: Quaternion-History
+58..+64, Winkelbudget +68 und History-Byte +6c einschließlich Padding. Aktivität
oder spätere gleiche Tracks ändern diese Auswahl nicht.

Existiert kein passender Track, wird ein neuer 28-Wort-Track angehängt:

- Die ersten 22 Worte stammen aus der aufgelösten Eingabe.
- +58..+64 erhalten das vorgegebene importierte Identity-Quaternion.
- +68 wird auf +0 gesetzt, Byte +6c auf 0.
- Die drei Padding-Bytes +6d..+6f werden im Original nicht initialisiert.
  `BonePlaceDefaults` verlangt deshalb einen ausdrücklichen Padding-Snapshot,
  anstatt für diese Bytes einen Original-Default zu erfinden.

## Grenzen der Namensbindung

`BonePlaceNames` ist die ausdrückliche Schnittstelle für positive globale
Namensindizes. `SnapshotBonePlaceNames` verbindet sie inzwischen mit der
rekonstruierten **MatchRefBone-Funktion, engine 10500410**, einschließlich der
Alias-Tabelle an Mesh +290. Die [Namensbindung](SKELETAL_BONE_NAMES.md) beschreibt
erste Alias-/Knochentreffer, optionale Matrixkopie und die Prüfung mit
Original-Referenzknochen. Globale Handle-Tabelle und Aliaswerte sind weiterhin
gelieferte Snapshots; die native dynamische Registrierung und Original-
Aliasarchive bleiben Aufgabe der Runtime-Integration.

Der Name-Payload 0 entspricht dem belegten Nullnamen und liefert -1. Negative
globale Name-Payloads liefern in Rust ausdrücklich einen Fehler statt eines
ungültigen Zugriffs auf die Original-Namenstabelle. Ein vorhandenes Mesh und
eine gültige vorbereitete Knochenanzahl sind Voraussetzung; Null-Mesh-Speicher-
zugriffe und beschädigte native Arrayzähler werden nicht emuliert.

Die Anlage setzt +61 zurück; der bereits verbundene
[Pose-/Bounds-Einstieg](SKELETAL_DIRECTED_BOUNDS.md) setzt den Wert nach
erfolgreicher vollständiger Auswertung und Publikation wieder auf 1. Die
übergeordnete Cacheentscheidung sowie der Script-/Actor-Aufruf dieses Einstiegs
sind noch nicht angeschlossen.

## Nachweis

`rc-set-bone-place-check` und `Record-SetBonePlace.py` prüfen unabhängig **512
synthetische Zustandsfälle**: 256 Anlagen, 128 Aktualisierungen und 128
Ablehnungen. Dabei werden 256 Namensauflösungen über kontrollierte Host-Antworten
ausgeführt. Alle Eingaben, Cachewerte, Aufruflisten und vollständigen Director-
Snapshots stimmen überein. Die Original-ASM-Marker belegen die unterschiedlichen
22-/28-Wort-Kopien sowie die byteweise History-Initialisierung.

Sechs Tests ergänzen History-Erhalt, erstes Match bei Duplikaten, Padding-Erhalt,
signed31-/Namensdarstellung, Anfangsindex-Rückfall, ungültige Ziele und die
unterschiedlichen Cachezustände bei frühen/späten Namensfehlern.
**330 Workspace-Tests**, Clippy mit `-D warnings` sowie Formatprüfung bestanden.
Hashes und Prüfdetails: `analysis/reports/set-bone-place-validation.json` und
`analysis/evidence.json`. Android und Emulator weiterhin am Schluss.
