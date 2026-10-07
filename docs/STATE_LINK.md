# Erstaufbau der Ereignis-Hashtabellen

`UState.Link` (`core.dll`, `10127a30`) ruft zunächst `UStruct.Link` auf. Der Struct-Iterator (`10121750`) beginnt bei Children `+0x3c`, folgt Next `+0x28` und filtert anhand der Klassenhierarchie auf UStruct. Link beendet den Aufbau, sobald der gefundene Struct nicht mehr Outer `this` besitzt. Geerbte Structs werden daher nicht erneut als eigene Einträge eingefügt.

Die erste eigene Structdefinition löst die Anlage einer Tabelle mit 128 nullinitialisierten Buckets aus (`0x200` Bytes im Original-PE32). Dabei werden die Bucketköpfe der nächsten Vorfahrentabelle kopiert, wobei Vorfahren ohne Tabelle übersprungen werden. Für jedes eigene Struct wird `HashNext` (`+0x2c`) auf den bisherigen Bucketkopf gesetzt; das Struct wird neuer Kopf. Der Bucket ergibt sich aus `resolved FName index & 0x7f`. Die Reihenfolge der eigenen Kinder ist damit relevant. Ein Zustand ohne eigene Structs erhält keine Tabelle; FindStruct folgt in diesem Fall weiterhin dem Parent.

`rc-package::state_link::link_states` baut frische Tabellen nach diesem Ablauf. Es ordnet Eltern vor Kindern, übernimmt vorhandene geerbte Ketten und erzeugt direkt den vom Ereignisadapter verwendeten EventLookupSnapshot. Die Konstruktion liefert bei ungültigen Eltern-/Kinderindizes, mehrfacher direkter Ownership, widersprüchlichen globalen Namensidentitäten oder Zyklen einen Fehler. Eingabedaten werden nicht verändert.

Die Eingabe verlangt bereits geordnete, auf eigene UStruct-Kinder gefilterte Listen und konsistente globale FName-Handles samt aufgelösten Indizes. Paketlokale Name-Indizes sind keine gültige Ersatzannahme. Die tatsächliche Extraktion der Kinderlisten aus Originalpaketen und die globale Namensregistrierung sind noch nicht angeschlossen. Der Builder bildet nur den Erstaufbau auf leeren Tabellen ab; bestehende native Tabellen erneut zu linken sowie andere UStruct.Link-Seiteneffekte sind nicht implementiert.

157 Workspace-Tests, Clippy mit Warnungen als Fehler und Formatprüfung bestanden. Neue Tests prüfen Vererbung über tabellenlose Zwischenzustände, überschriebene Handler, Hashkollisionen und Einfügereihenfolge, unveränderte Eltern- und Geschwisterketten, nichtfunktionale Überschreibungen, ungültige Eingaben und leere Graphen. Diese Nachweise verwenden synthetische Kinderlisten; sie belegen keine zusätzliche Originalkarten- oder VM-Abdeckung.

Nachweise: `analysis/decompiled/state-link.c`, `state-struct-iterator.c`, `analysis/reports/state-link-validation.json`; wiederholbare Dokumentation über `scripts/Record-StateLink.py`. Die tatsächliche aktive Zustandswahl und die Ausführung der Handler bleiben offen. Android-/Emulatorprüfung bleibt nach Nutzerwunsch bis zum Schluss verschoben.

## Folgearbeit: Original-Kinderlisten

Die geordneten Kinderlisten sind jetzt direkt aus Originalklassen und -zuständen angeschlossen. Die Diagnose baut Tabellen mit eigenen global konsistenten Namenskennungen; native FName-Werte und aktive Zustandswahl bleiben offen. 3170 Klassen, 260 Zustände, 15915 Felder und 51 Controller-Klassensuchen geprüft. 160 Tests bestanden. Siehe [ORIGINAL_STATE_LINK.md](ORIGINAL_STATE_LINK.md).
