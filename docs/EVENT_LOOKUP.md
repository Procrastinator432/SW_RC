# Native Auswahl von Ereignishandlern

Die FName-Überladung von `UObject.ProcessEvent` (`10104e90`) ruft `FindFunctionChecked` mit global_only null auf und dispatcht anschließend über den virtuellen Slot `+4`. `FindFunction` (`101548c0`) und `FindFunctionChecked` (`101571d0`) belegen die Suchreihenfolge: vorhandener aktiver StateNode aus `Object+0xc`, dann `StateFrame+0x18`; bei fehlendem Treffer folgt die Objektklasse an `Object+0x24`. global_only überspringt die Zustandssuche.

`UState.FindStruct` (`10123c40`) folgt Parent `+0x30`, solange die Hashtabelle `+0x90` null ist. An der ersten vorhandenen Tabelle prüft es Bucket `resolved FName index & 0x7f`, folgt HashNext `+0x2c` und vergleicht den FName-Handle an `+0x20` direkt. Eine vorhandene leere Tabelle ist von einer fehlenden Tabelle zu unterscheiden. Bei fehlendem Namen in einer vorhandenen Tabelle findet kein zusätzlicher Parentwalk statt. Die Tabelle muss deshalb bereits die zur Laufzeit benötigten Einträge enthalten; eine einfache Suche durch Paketexporte ersetzt ihren Aufbau nicht.

Nach der Suche prüft FindFunction das Klassentypflag `0x80000` an der Klasse des gefundenen Structs. Das ist kein UFunction-Funktionsflag. Ein nichtfunktionaler Treffer im Zustand wird verworfen, ohne danach zur Objektklasse zurückzukehren.

`rc-package::event_lookup` setzt diesen Ablauf für explizite Runtime-Snapshots um. Der integrierte NotifyHitWall-Adapter prüft zuerst die Wrapper-Ereignismaske. Ein deaktiviertes Ereignis erfordert keine Handlersuche. Fehlende Tabellen-/Eintragsdaten, Zyklen und aktivierte unbekannte Handler liefern Fehler. Dies ist keine Rekonstruktion von FindFunctionCheckeds fataler nativer Fehlerbehandlung.

Die Tests prüfen Zustandshandler, Klassenfallback, global_only, nichtfunktionale Treffer, fehlende gegenüber leeren Tabellen, Parentwalk, Hashketten, Handleidentität einschließlich null, unvollständige Snapshots, Zyklen und maskierte Ereignisse. 154 Workspace-Tests, Clippy mit Warnungen als Fehler und Formatprüfung bestanden.

Das wiederholbare Inventar in `scripts/Record-EventLookup.py` prüft alle 273 bestehenden Paketberichte und findet genau einen serialisierten NotifyHitWall-Funktionsexport: Engine.Controller.NotifyHitWall. Das belegt keine Abwesenheit nativer oder zur Laufzeit geänderter Handler. Weitere Bestätigung für das leere Basisereignis: `UObject.execNothing` (`1012f350`) verändert keinen Rückgabewert.

Nachweise: `analysis/decompiled/notify-event-resolution.c`, `notify-find-struct.c` und `analysis/reports/event-lookup-validation.json`. Offen bleiben Aufbau der Hashtabellen nach dem Laden, tatsächliche aktive Zustandswahl, FName-Registrierung, GIsScriptable- und übrige ProcessEvent-Gates, virtuelle Overrides sowie Scriptausführung und Seiteneffekte. Kartendiagnosen behalten bis dahin ausdrücklich gelieferte Antwort-Snapshots; es wurde keine neue Kartenabdeckung behauptet. Androidprüfung bleibt bis zum Schluss verschoben.
