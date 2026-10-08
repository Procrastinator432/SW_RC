# GetFrame-Zweigauswahl und GetSequence

Stand: 2026-10-08. `evaluate_animation_frame` bildet die Zweigauswahl aus `USkeletalMeshInstance::GetFrame`, engine `1050b330`, ab. Die Originalroutine vergleicht den Frame-Typ mit 3 und übergibt das Ergebnis an ApplyAnimation.

## Gemeinsamer Einstieg

Typ 3 verwendet `apply_animation_root`, jeder andere i32-Wert `apply_animation_full`. Nur der ausgewählte Pfad führt seine bereits geprüfte Instanzvorbereitung aus. Der Dispatcher führt sie nicht zusätzlich aus. Der Root-Pfad umgeht weiterhin das Vollpose-Cachegate; der Vollpose-Pfad behält sein Cacheverhalten. Hosts des nicht ausgewählten Pfads werden nicht aufgerufen.

Die gemeinsamen Eingabestrukturen enthalten auch Vollpose-Parameter. Der Root-Pfad verwendet davon nur Referenzknochen, Linkupanzahl und Blockierwort; er prüft weder Hierarchie noch Scratch. Vorbereitung und alle Fehlerteilzustände bleiben in den zuvor geprüften Root-/Vollpose-Funktionen. Ein Fehler in der Vorbereitung propagiert aus dem ausgewählten Pfad, ohne den anderen Pfad aufzurufen.

Drei Integrationstests prüfen Frame-Typen 3, −1, 0, 1, 2, 4, i32::MIN und i32::MAX sowie Vorbereitungsfehler in beiden Zweigen. Sie belegen eine einzige Transform-/Linkup-Vorbereitung und ausschließlich die jeweils gewählten Pose-Hosts. Die zugrunde liegenden Originaltrack-Prüfungen bleiben die früheren Root-/Vollpose-Berichte; dieser Meilenstein erzeugt keine neuen Originaltrack-Samplingzahlen.

## Sequenzabfrage

`skeletal_sequence::get_sequence` übernimmt `ULodMeshInstance::GetSequence`, engine `1044faf0`. Außerhalb des Editors liefert es das gespeicherte Kanalwort +44 (Wort 17), ohne Suche oder Identitätsprüfung. Auch unbekannte rohe Identitäten bleiben dadurch erhalten. Identität 0 ist der Nullzeigerwert.

Im Editor wird zuerst die virtuelle Sequenzsuche des Meshes (+ac) mit dem Kanalnamen aus Wort 0 und `load=false` aufgerufen. Ihr Ergebnis ersetzt Wort 17, auch wenn es 0 ist. Bei einem kontrollierten Hostfehler bleibt der bisherige Kanalwert erhalten. Negative oder außerhalb des Arrays liegende Kanalindizes liefern Rust-Fehler statt ungültiger nativer Speicherzugriffe.

`SequenceLookupHost` ist die Schnittstelle zur tatsächlichen virtuellen Suche. `SnapshotSequenceLookup` bietet eine konkrete Suche über gelieferte opake Namens-/Sequenzbindungen; der erste exakte Namenstreffer gewinnt. Diese Bindings sind diagnostisch und bestimmen nicht die native dynamische Registrierung oder das Laden von Assets. Eine vollständige Runtime muss sowohl die tatsächliche Mesh-Suche als auch die Auflösung einer Sequenzidentität zu Tracks und Linkups anschließen. Die bestehende Pose-Host-Schnittstelle wird in diesem Abschnitt noch nicht durch einen solchen Runtime-Host ersetzt.

## Prüfung und Grenzen

`rc-sequence-check` und `Record-FrameDispatch.py` prüfen unabhängig **512 vollständige Kanalzustände**: 256 rohe Cachelesevorgänge und 256 Editor-Suchen, davon 128 mit Nullergebnis. Alle Kanalwörter und Hostereignisse stimmen; vier zusätzliche Tests ergänzen Fehler, ungültige Indizes und doppelte Bindings. Zusammen mit drei Dispatcher-Tests sind **378 Workspace-Tests**, Clippy mit `-D warnings` und Formatprüfung bestanden.

Gemeinsame Zweigauswahl und ursprüngliche Sequenzcache-Regeln sind damit verfügbar. Die [konkreten Hosts für geladene Animationen](SKELETAL_RUNTIME_HOSTS.md) verbinden inzwischen diese Sequenzabfrage, Originaltracks und gemeinsam aktualisierte Linkup-Mappings mit beiden Posepfaden. Dynamische Actor-/Script-Anbindung, Ticking, Skinning und sichtbare Animation bleiben offen. Androidprüfung am Projektende.

Nachweise: `analysis/reports/sequence-lookups.json`, `frame-dispatch-validation.json`, `frame-dispatch-tests.log`, `analysis/decompiled/animation-replication-support.c/.asm`, `skeletal-root-frame.asm` und `analysis/evidence.json`. Verwandt: [Root-Einstieg](SKELETAL_ROOT_ENTRY.md), [Vollpose-Einstieg](SKELETAL_ANIMATION_ENTRY.md).
