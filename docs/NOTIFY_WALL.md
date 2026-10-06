# NotifyHitWall: native Ereignisprüfung

`AController.NotifyHitWall` an `103604f0` prüft die Ereignismaske vor `ProcessEvent`. Der Wrapper liest den Names-Tabellenslot `0x161` (Byteoffset `0x584`). Der daraus gelesene FName-Index ist davon zu unterscheiden und wurde als Runtimewert noch nicht bestimmt.

Für aufgelöste Indizes von 300 bis einschließlich 363 und einen vorhandenen StateFrame-Zeiger (`Controller+0xc`) prüft der Wrapper das Bit `1u64 << (index-300)` gegen die beiden Wörter an `+0x1c/+0x20`. Ohne dieses Bit liefert er null, ohne das Ereignis aufzurufen. Ohne StateFrame oder außerhalb dieses Namensbereichs entfällt diese Maskenprüfung.

Vor dem Aufruf werden HitNormal, Wall und ein Rückgabefeld mit null vorbereitet. Der originale Basisexport `Controller.NotifyHitWall` aus engine.u enthält exakt `Return` mit `Nothing`: zwei Scriptbytes, NativeIndex null, Flags `0x20800`. Für diesen verifizierten leeren Handler bleibt der vorbereitete Rückgabewert false. Der enge Rust-Adapter erkennt diese Struktur; er führt keinen allgemeinen Scriptcode aus.

`notify_wall::notify_hit_wall` nimmt den aufgelösten Namensindex, eine optionale StateFrame-Maske und einen expliziten Handler entgegen. Er unterscheidet MaskedOut, EmptyBase und SuppliedResult. Ein aktivierter unbekannter Handler liefert einen Fehler. Maskierte unbekannte Handler dürfen false liefern, weil die native Funktion sie ebenfalls nicht aufruft.

Die API verlangt, dass der Aufrufer einen tatsächlich aufgelösten Basisexport übergibt, wenn er EmptyBase auswählt. Das Vorhandensein des Basisexports belegt keine Auswahl dieses Handlers für eine reale Controllerinstanz. Abgeleitete Klassen, Zustandsüberschreibungen, native ProcessEvent-Auflösung, Ereignisseiteneffekte und VM-Ausführung bleiben offen. Deshalb behalten bestehende KI-Kartendiagnosen ihre ausdrücklich gelieferten false-Snapshots. Keine Behauptung zusätzlicher Kartenabdeckung.

Nachweise: `analysis/decompiled/controller-notify-wall.c`, `.asm`, `analysis/reports/controller-wall-bytecode.json`, `notify-wall-base-validation.json` und `notify-wall-validation.json`. Wiederholbare Originalprobe: `rc-notify-wall-probe <engine.u> <report.json>`. `scripts/Record-NotifyWall.py` kontrolliert Exportinstruktionen und Berichte, aktualisiert Evidence und Wiki.

151 Workspace-Tests, Clippy mit Warnungen als Fehler und Formatprüfung bestanden. Tests decken beide Maskenwörter und Bereichsgrenzen, fehlenden StateFrame, unbekannte aktivierte Handler, gelieferte Antworten und abweichende Basis-ASTs ab. Die Probe auf originalem engine.u bestätigt den leeren Basisexport. Android-/Emulatorprüfung bleibt nach Nutzerwunsch bis zum Schluss verschoben.
