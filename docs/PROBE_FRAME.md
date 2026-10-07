# Initialisierung und Änderung der Ereignismaske

`UObject.InitExecution` (`10156da0`) setzt den StateNode des neuen FStateFrame auf die Objektklasse und beide Ereignismaskenwörter auf `0xffffffff`. Das ergibt eine anfänglich vollständig gesetzte 64-Bit-Maske. Dieser Zustand unterscheidet sich von einem späteren Zustandswechsel.

`UObject.GotoState` (`1013a0c0`) setzt in seiner Zuweisungsphase StateNode auf das bereits bestimmte Ziel und berechnet die Maske als `(Class.ProbeMask | State.ProbeMask) & State.IgnoreMask`. Die IgnoreMask wird direkt mit AND verwendet, ohne sie zu invertieren. Vor dieser Phase können EndState-Callbacks den Ablauf durch weitere Zustandswechsel verändern. Nach der Phase kann BeginState ebenfalls einen Wechsel auslösen. Die Auto-Statewahl berücksichtigt zusätzlich Stateflags. Deshalb bildet eine isolierte Maskenzuweisung keinen vollständigen GotoState-Aufruf ab.

`probe_frame::ProbeFrame` modelliert ausschließlich Objektklasse, StateNode und ProbeMask. `init_execution` setzt den nativ belegten Anfangszustand. `resolved_state_masks` bildet nur die Maskenzuweisungsphase mit explizitem Ziel und Masken-Snapshots ab. Der Aufrufer muss Zielwahl und EndState-Behandlung zuvor selbst geklärt haben. Codezeiger, latente Aktionen, Objektflags und Zustandswechsel-Callbacks sind nicht enthalten.

`Enable` (`1013bbc0`) ergänzt nur das angeforderte Bit, das durch `(Class.ProbeMask | State.ProbeMask) & State.IgnoreMask` erlaubt ist. `Disable` (`1013bcb0`) löscht das einzelne Ereignisbit. Beide unterstützen nur Indizes 300..363. Die Rust-Helfer geben bei ungültigen Namen einen Fehler zurück und lassen die Maske unverändert; nativ wird eine Meldung ausgegeben. `IsProbing` (`1010d470`) ist für Namen außerhalb dieses Bereichs und ohne StateFrame immer true, sonst hängt es vom Maskenbit ab.

Die Originalpaketprobe erzeugt nun zusätzlich den belegten Initialzustand für alle 51 Controller-Klassen. StateNode zeigt jeweils auf die Klasse. Die Suche liefert Engine.Controller.NotifyHitWall mit EmptyBase; nach Disable353 ergibt derselbe Aufruf MaskedOut. Diese Probe verwendet echte Original-Klassenlisten und den festen Namensindex353, aber keine laufende Spielinstanz. Die bisherigen 3430 Ownerlisten und 51 Klassen-Suchen bleiben unverändert. Die neuen Ergebnisse stehen separat in `analysis/reports/initialized-probe-frames.json`.

164 Workspace-Tests, Clippy mit Warnungen als Fehler und Formatprüfung bestanden. Neue Tests prüfen den Unterschied zwischen Initialmaske und zusammengesetzter Maske, beide Maskenwörter, Enable mit freigegebenen/gesperrten/nicht definierten Bits, unveränderte übrige Bits, ungültige Namen und IsProbing ohne Frame. Native Nachweise: `analysis/decompiled/active-state.c` und `.asm`. Validierung und Dokumentation: `scripts/Record-ProbeFrames.py`, `analysis/reports/probe-frame-validation.json`.

Die Klassen-/Zustands-/IgnoreMasken für spätere Zustände und Enable werden weiterhin als explizite Snapshots geliefert. Ihre Auswertung aus Originaldaten, tatsächliche aktive Zustandswahl, BeginState/EndState-Ausführung, native Overrides und allgemeine VM bleiben offen. Androidprüfung weiterhin erst zum Schluss.

## Folgearbeit: Originalmasken

Originale Zustands-/Ignoremasken und beim Laden neu berechnete Klassenmasken sind jetzt angeschlossen. 3430 Kompositionen und31 ausdrücklich ausgewählte deklarierteController-States geprüft; diese31 Masken unterdrücken NotifyHitWall. Tatsächliche Zustandswahl/Wechselcallbacks bleiben offen.167Tests bestanden. Siehe [ORIGINAL_STATE_MASKS.md](ORIGINAL_STATE_MASKS.md).

## Folgearbeit: Wechselablauf

Benannte und Auto-Zielwahl sowie der Basiswechsel mit alten/neuen Ereignismasken sind jetzt verbunden. Ereignisfunktionen müssen ausdrücklich bereitgestellt werden; Originalproben halten vor ihrer unbekannten Ausführung an. 182 Tests bestanden. Siehe [STATE_TRANSITIONS.md](STATE_TRANSITIONS.md).
