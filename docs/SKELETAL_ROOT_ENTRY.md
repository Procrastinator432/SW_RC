# Root-only-Einstieg mit gemeinsamer Vorbereitung

Stand: 2026-10-08. `apply_animation_root` verbindet den Root-only-Pfad mit derselben Instanzvorbereitung wie `apply_animation_full`: Pufferanpassung, Transformabfrage, Referenzcache und geordnete Linkup-Aufrufe.

## Verhalten

Im Original liegt die Root-only-Entscheidung vor dem Vollpose-Cachegate. Deshalb wird der Root auch bei gesetztem +61 ausgewertet. Nach der gemeinsamen Vorbereitung übernimmt der Einstieg den Referenzroot und führt die bereits rekonstruierte Auswahl geeigneter Root-Kanäle aus. Der letzte erfolgreiche Sample gewinnt. Anschließend wird ausschließlich die Rootmatrix erstellt.

Der Einstieg verändert keine anderen Knochen und greift nicht auf den gemeinsamen Scratch zu. Bounds und die Vollpose-Abschlussflags +60/+61/+179 bleiben nach der Vorbereitung unverändert. Die Vorbereitung selbst darf +61 bei Größenänderung oder leerem Referenzcache auf 0 setzen. Root-only setzt diesen Wert anschließend nicht auf 1.

Bei einem Samplingfehler bleiben bereits geschriebene Quaternion-/Positionswerte im Root erhalten. Die vorherige Rootmatrix bleibt bestehen, weil sie erst nach erfolgreicher Kanalschleife übernommen wird. Bei einem Fehler in der Vorbereitung wird der Root noch nicht auf die Referenzpose gesetzt. Ein leeres Skelett liefert nach der Vorbereitung einen kontrollierten Rust-Fehler, statt auf einen nicht vorhandenen Root zuzugreifen.

## Prüfung

`rc-root-entry-check` lädt die Original-Animationstracks erneut und verwendet die erste Sequenz je auflösbarem Linkup. 225 Läufe liefern je sieben Fälle: Frames 0, 0,25, 0,75, 1, ein negativer Frame, NaN und eine blockierte Root-Auswertung. Instanzpuffer beginnen pro Lauf leer; der mesh-eigene Referenzcache wird zwischen Läufen wiederverwendet.

`Record-RootAnimationEntry.py` liest die Tracks unabhängig aus den Originalarchiven und berechnet Schlüsselwahl, Quaternionen, Positionen und Rootmatrizen nach. **1.575 Fälle**, davon **1.344 gesampelte Roots** und **231 Referenzroots**, stimmen überein. Bei den geprüften Quaternionen und Positionen beträgt die beobachtete Abweichung 0; die festgelegten Toleranzen bleiben 2e-6 beziehungsweise 2e-5 relativ. Alle Rootmatrizen stimmen bitgenau mit dem unabhängigen ASM-Konstruktor-Orakel überein.

Zusätzlich werden 1.575 Transformabfragen, 7.973 Linkup-Hostaufrufe, Cacheinvalidierungen und unveränderte Bounds geprüft. Der Rust-Diagnoselauf prüft nach jedem Aufruf sämtliche Nicht-Root-Einträge auf die Nullwerte aus der Kaltstart-Vorbereitung. Sechs neue Integrationstests ergänzen gesetzten Pose-Cache, erhaltene bereits vorhandene Nicht-Root-Werte, Teilzustände bei Sampling-/Vorbereitungsfehlern, leere Skelette und mehrere geeignete Kanäle. **371 Workspace-Tests**, Clippy mit `-D warnings` und Formatprüfung bestanden.

## Grenzen

Die Root- und Vollpose-Einstiege bleiben getrennt nutzbar. Die [gemeinsame GetFrame-Zweigauswahl](SKELETAL_FRAME_DISPATCH.md) wählt inzwischen anhand des ursprünglichen Frame-Typs den passenden Einstieg. Transform-/Linkup-Hosts und Sequenzauflösung bleiben ausdrückliche Schnittstellen. Der Diagnosehost verwendet feste Originalsequenzen und Mapping-Snapshots, keine echte Actor-/Script-Anbindung.

Frame-Multiplikation wird im Diagnosehost mit f64 und abschließendem f32-Speichern angenähert, Quaternionmathematik folgt der bisherigen portablen Policy. Native x87-Präzisionsmodi und CPU-Sondermodi werden nicht emuliert. Echtes Ticking, konkrete Runtime-Hosts, Skinning und sichtbare Animation bleiben offen. Androidprüfung am Projektende.

Nachweise: `analysis/reports/root-animation-entry.json`, `root-animation-entry-validation.json`, `root-entry-tests.log`, `analysis/decompiled/skeletal-root-frame.asm` und `analysis/evidence.json`. Verwandt: [Vollpose-Einstieg](SKELETAL_ANIMATION_ENTRY.md), [früherer Root-only-Meilenstein](SKELETAL_ROOT_POSE.md).


Fortsetzung 2026-10-08: [Konkrete Hosts fuer geladene Animationen](SKELETAL_RUNTIME_HOSTS.md) verbinden jetzt GetSequence, Originaltracks und tatsaechlich aktualisierte gemeinsame Linkup-Mappings ueber den Frame-Einstieg. 386 Workspace-Tests bestanden; dynamische Actor-/Script-Anbindung, Ticking und Skinning bleiben offen.
