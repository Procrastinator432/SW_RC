# GetMoveCoords und aktive Animationskanäle

Folgemeilenstein: [Vorbereiteter Root-only-Posezweig](SKELETAL_ROOT_POSE.md) bestimmt die GetFrame-Weiterleitung und ergänzt die Root-Auswahl sowie die native Quaternion-/Translationsmatrix. Die vorherige Probe bleibt unverändert; echte Vorbereitung und Trackauswertung sind weiterhin offen.

`move_coords.rs` ergänzt USkeletalMeshInstance.IsChannelActive (`10500930`) und den absoluten Aufruf von GetMoveCoords (`10500a30`). Die bisherige PostInit-Root-Grenze ist damit konkreter bestimmt: Die originale Skelett-Vtable bei `1066a5c8`, Slot +e0, zeigt auf GetMoveCoords. Die vorherigen Diagnosen verwendeten für diese unbekannte Grenze die beschreibende Hostbezeichnung GetRootLocation; das war kein belegter nativer Funktionsname.

PostInit ruft GetMoveCoords mit der persistenten Matrix bei Instanz +128 und relative=false auf. Diese Variante ist jetzt als Rust-Auswahlablauf umgesetzt und in der Diagnose an `skeletal_post_init` angeschlossen. Relative=true sowie tatsächliche Frame-/Poseauswertung sind weiterhin offen.

## Kanalaktivität

`channel_active` prüft die vorzeichenbehafteten Index-/Bonebedingungen und die Überdeckung durch spätere Kanäle. Ungültige Kanalindizes liefern false. Kanal 0 benötigt keinen positiven Wert im Floatfeld +38; alle anderen Kanäle benötigen +38 > 0. NaN passiert diesen Vergleich nicht.

Bei nicht negativem Bone muss dieser im halboffenen Intervall `[Kanal.+3c, Kanal.+40)` liegen. Negative Bonewerte überspringen diese Punktprüfung. Anschließend werden ausschließlich spätere Kanäle untersucht. Nur wenn deren f32-Produkt aus +38 und +2c exakt 1 ist, können sie den untersuchten Kanal verdecken. NaN und bloß annähernd 1 genügen nicht.

Bei einer Punktabfrage wird der Kanal inaktiv, wenn ein solcher späterer Kanal den Bone enthält. Bei einer negativen Boneabfrage muss ein späterer Kanal das gesamte Intervall des untersuchten Kanals enthalten. Teilüberdeckung lässt ihn aktiv. Die Vergleiche sind vorzeichenbehaftet; die Kanalnummer ist für diese Prüfung nicht maßgeblich.

## GetMoveCoords(false)

Die Funktion durchläuft die Kanalreihenfolge und führt je Kanal folgende Prüfungen aus:

1. Loop-Byte +04 muss null sein.
2. Boneindex +3c muss dem Meshwert bei +1c0 entsprechen.
3. IsChannelActive(index, −1) muss true liefern.
4. Rate +18 muss ungleich null sein; NaN wird im Original ebenfalls als ungleich null behandelt.
5. GetSequence(index) muss eine Sequenz liefern.
6. Deren Byte bei +54 muss Bit 0 enthalten.

Die Sequenzauflösung bleibt eine Hostoperation, einschließlich möglicher Editoraktualisierung des Kanal-Sequenzpointers. Der Adapter darf aus Kanalnamen oder Pointertokens keine vorhandenen Originalclips ableiten.

Ohne Outputmatrix werden die Voraussetzungen einschließlich Sequenzsuche geprüft, aber keine Frame-/Poseauswertung ausgelöst. Bei einem Treffer zählt die Funktion ihn dennoch und liefert am Ende true.

Mit Outputmatrix folgen GetActor und die virtuelle Instanzoperation +dc mit den im Assembler belegten Nullargumenten, Flags 3 und relative=false. Danach werden genau 16 Wörter der Instanzmatrix bei +a0 in den Output kopiert. Diese Auswertung bleibt eine explizite Hostgrenze. Die Diagnose liefert Matrizen oder hält vor der Auswertung an; sie behauptet keine tatsächliche Pose.

Die Funktion kehrt nicht beim ersten Treffer zurück. Sie verarbeitet alle geeigneten Kanäle und kopiert bei jedem erfolgreichen Treffer erneut. Deshalb gewinnt die letzte Matrix. Schlägt eine spätere Hostauswertung fehl, bleiben bereits erfolgte Sequenzaktualisierungen und frühere Matrixkopien erhalten. Kein geeigneter Kanal liefert false und lässt den Output unverändert.

Im PostInit-Anschluss führt true weiterhin zur offenen Matrixinverse-/Actor-Kompositionsgrenze. False beendet PostInit ohne diesen Aufruf. Die tatsächliche Zusammensetzung der Root-/Actortransformation ist weiterhin nicht portiert.

## Diagnose und Nachweise

Die Probe übernimmt die 768 bisherigen NewSequence-Kanalsnapshots und prüft je zehn ausdrücklich gelieferte Varianten. Loop-/Rate-/Gewichtswerte, Move-Bone, Sequenzflag, Pointeraktualisierungen und Matrizen sind Diagnosedaten. Die Quelldaten und Zuordnung zu den früheren AnimProp-Anforderungen bleiben im Bericht nachvollziehbar.

| Ergebnis | Fälle |
|---|---:|
| RootUnavailable | 4.608 |
| Treffer ohne Outputmatrix | 768 |
| Offene Matrixkomposition nach erfolgter Kopie | 768 |
| Offene Frame-/Poseauswertung | 768 |
| Abschluss mit ausdrücklich gelieferter Komposition | 768 |

Die unabhängige Prüfung berechnet alle 7.680 Fälle einschließlich Kanalwörter, Matrixwörter, Ereignisreihenfolge und Abbruchgrenzen nach. Der exportierte Vtable-Slot wird direkt gegen engine.dll geprüft. Die Assemblernachweise umfassen Kanalüberdeckung, Loop-/Bone-/Rate-/Sequenz-/Outputprüfungen sowie die Fortsetzung der Schleife nach einem Treffer. Frühere Berichte bleiben unverändert.

```text
cargo run -p rc-inspect --bin rc-move-coords-probe -- analysis/reports/channel-playback.json analysis/reports/move-coordinates.json
python scripts/Record-MoveCoordinates.py
```

Nachweise: `analysis/reports/move-coordinates.json`, `move-coordinates-validation.json`, `analysis/decompiled/animation-move-coords.c/.asm`, `animation-channel-active.c/.asm` und `analysis/evidence.json`.

229 Workspace-Tests (219 rc-package, 4 rc-native, 6 rc-render), Clippy mit Warnungen als Fehler und Formatprüfung bestanden. Sechs neue Tests prüfen Index-0-Ausnahme, Boneintervalle, vollständige und punktuelle Überdeckung, exaktes Floatprodukt/NaN, Auswahlgates, Sequenzschreibzugriffe ohne Output, letzte Matrix und Teilschreibzugriffe bei späteren Fehlern.

Originalskelette/-clips, echte Frame-/Poseauswertung, relative Matrixberechnung, Matrixinverse und Actor-Komposition bleiben offen. Die x87-Prozesspräzision ist ebenfalls noch nicht belegt; der Import von _controlfp in den Spielprogrammen allein bestimmt sie nicht. Androidprüfung weiterhin zum Schluss.
