# Automatische Zustands-Zielwahl

Die Auto-Zielphase von `UObject.GotoState` (`1013a0c0`) verwendet den State-Iterator `10139f10`. Dieser beginnt bei der eigenen Children-Liste an +0x3c, folgt Next an +0x28, filtert auf das State-Klassentypflag0x40000 und wechselt danach zur Elternliste. GotoState wählt den ersten Zustand, dessen StateFlags an +0x88 den Bitwert2 enthalten. Das ist Vorwärtsreihenfolge der eigenen Liste vor den Vorfahren, keine Hashreihenfolge.

Der gefundene Zustandszeiger wird direkt zum Ziel. Sein Name wird für den weiteren Wechselablauf übernommen, aber nicht erneut über FindState aufgelöst. Deshalb kann Auto einen Elternzustand wählen, auch wenn eine abgeleitete Klasse denselben Namen mit einem nicht als Auto markierten Zustand überschreibt. Ohne Auto-Treffer wird die Objektklasse als Ziel gewählt.

`state_selection::AutoStateLookup` bildet ausschließlich diese Zielphase ab. Die Eingabe enthält geordnete eigene State-Kinder und die Original-Stateflags. Fehlende Listen, Flags oder Kandidaten sowie Elternzyklen liefern Fehler. Der vorhandene benannte Resolver bleibt getrennt; er leitet Auto nicht fälschlich durch die Namenssuche.

Die Originalprobe über alle3170Klassen findet517Auto-Ziele und2653Klassenfallbacks. Für51Controller-Klassen ergeben sich47Auto-Ziele und4Fallbacks.40Controller wählen CTBot.BotAI,6wählen PlayerController.PlayerWaiting und einer DemoRecSpectator.Spectating. Die komponierten NotifyHitWall-Masken sind bei allen51Controller-Zielen deaktiviert. Die Auswahl berücksichtigt dabei die konkrete Objektklasse auch bei geerbten Zuständen.

173Workspace-Tests, Clippy mit Warnungen als Fehler und Formatprüfung bestanden. Neue Tests prüfen eigene Reihenfolge, Elternfallback, einen gleichnamigen nicht-automatischen Override, fehlende Auto-Kandidaten, unvollständige Daten und Zyklen. `scripts/Record-AutoStates.py` berechnet alleOriginalziele und Masken unabhängig aus den vorwärts gelesenen Kinderlisten, Flags und Elternketten nach. Bisherige benannte Zustandsproben und Originalberichte bleiben unverändert.

Die Probe `rc-state-link-probe <GameData> <report.json> --native-hardcoded-names --original-masks --named-states` enthält nun zusätzlich Auto-Ziele. Nachweise: `analysis/reports/auto-state-selection.json`, `auto-state-selection-validation.json`, native Iteratorfunktion in `analysis/decompiled/auto-state-iterator.c` und Flagtest im vorhandenen `active-state.asm`.

Dies belegt das Ziel nach der Auswahlphase der Basisfunktion. EndState/BeginState können weitere Wechsel auslösen; native Overrides, Code-/Latentfelder und VM-Ausführung sind nicht rekonstruiert. Daraus folgt keine vollständige Startzustands- oder GotoState-Parität einer laufenden Spielinstanz. Androidprüfung weiterhin zum Schluss.

## Folgearbeit: Wechselablauf

Der Basisablauf um EndState/BeginState, LatentAction, Node/StateNode/Code und rekursive Umleitungen ist jetzt auf expliziten Snapshots implementiert. Die Originalproben halten an unbekannten Ereignisfunktionen an. 182 Tests bestanden; vollständige VM-/ProcessEvent-Ausführung bleibt offen. Siehe [STATE_TRANSITIONS.md](STATE_TRANSITIONS.md).
