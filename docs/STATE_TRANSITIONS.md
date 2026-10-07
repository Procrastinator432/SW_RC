# Zustandswechsel mit expliziten Ereignisfunktionen

`state_selection::AutoStateLookup::resolve_target` verbindet die benannte und die automatische Zielwahl. Ein erfolgreicher benannter Treffer behält die angeforderte Namensidentität; Auto übernimmt den Namen des direkt gewählten Zustands; ein Klassenfallback verwendet None. Die ursprünglichen getrennten Resolver bleiben verfügbar.

`state_transition::goto_state` rekonstruiert den geprüften Ablauf der Basisfunktion UObject.GotoState (`1013a0c0`) auf expliziten Snapshots. Die Ereignisfunktionen werden vom aufrufenden Host bereitgestellt. Es gibt keine stillschweigende Annahme, eine unbekannte Funktion sei leer.

1. Ohne StateFrame lautet das Ergebnis 0. Mit Frame wird LatentAction (+0x24) gelöscht. Als bisheriger Name gilt None, wenn StateNode die Objektklasse ist, sonst der gespeicherte Zustandsname.
2. Bei einem anderen Zielnamen wird EndState nur für einen bisherigen Namen ungleich None, eine aktive alte Probe-Maske (Index 317) und einen freien Rekursionsguard 0x2000 angefordert. Vorher wird 0x1000 gelöscht und 0x2000 gesetzt. Nach dem Ereignis wird 0x2000 gelöscht; gesetztes 0x1000 liefert Ergebnis 2 vor der äußeren Zielzuweisung.
3. Node (+4) und StateNode (+0x18) erhalten das Ziel, Code (+0xc) wird null. Die neue Probe-Maske ist `(ClassProbe | TargetProbe) & TargetIgnore`.
4. None liefert Ergebnis 0. Bei geändertem Namen prüft BeginState die neue Maske (Index 316). Vor dem Ereignis wird 0x1000 gelöscht; ein danach gesetztes Bit liefert Ergebnis 2. Der normale Abschluss setzt 0x1000 und liefert Ergebnis 1.

EndState ruft den nativen Wrapper `101066d0` auf. Dieser sucht EndState über FindFunctionChecked und ruft ProcessEvent virtuell auf. Der neue Export `analysis/decompiled/end-state-event.c`, der vorhandene GotoState-Assembler und die geprüfte feste Namenstabelle belegen die Ereignisindizes und die Reihenfolge. Namen werden über Handleidentität verglichen, nicht über Zustandspointer: Ein gleichnamiges anderes Ziel unterdrückt die Ereignisse, weist aber den neuen Zustand und die Maske zu.

Die synthetischen Tests führen tatsächliche rekursive Aufrufe des Rust-Adapters aus. Sie prüfen den EndState-Rekursionsguard, Umleitungen aus EndState und BeginState, den Sonderfall einer rekursiven None-Umleitung, alte gegenüber neuen Masken, identische Namen, fehlende Frames und Phasenabbrüche. Eine None-Umleitung setzt selbst kein 0x1000; der äußere Wechsel kann deshalb weiterlaufen. Ein bereits vorhandenes 0x1000 bleibt bei einem None-Fallback ohne EndState erhalten.

Die Originalprobe umfasst 3.170 Auto-Ziele: 2.653 Klassenfallbacks, 143 Zustände ohne aktivierten BeginState-Aufruf und 374 angeforderte BeginState-Ereignisse. Bei diesen 374 hält die Probe an und protokolliert die gefundene Funktion. Zusätzlich werden 517 separat gelieferte aktive Auto-State-Snapshots nach None gewechselt: 440 benötigen keinen EndState-Aufruf, 77 halten vor der unbekannten EndState-Ausführung an. Diese Austrittsproben sind keine Fortsetzungen hinter den angehaltenen BeginState-Aufrufen.

`scripts/Record-StateTransitions.py` prüft Maskengates, Handlerwahl und alle geschriebenen Snapshotfelder unabhängig aus Original-Kinderlisten und Masken. Die Handlerwahl verwendet umgekehrte eigene Kinderreihenfolge und anschließend Elternketten; aktive Zustände haben Vorrang vor der Objektklasse. Frühere Ziel-, Masken- und Lookup-Berichte werden vollständig auf unveränderte Werte verglichen. 182 Workspace-Tests (172 rc-package, 4 rc-native, 6 rc-render), Clippy mit Warnungen als Fehler und Formatprüfung bestanden.

Aufruf:

```text
rc-state-link-probe <GameData> <report.json> --native-hardcoded-names --original-masks --named-states --state-transitions
```

Die zusätzlichen Werte LatentAction=7, ObjectFlags=0 beim Eintritt sowie Code=123 und ObjectFlags=0x1000 beim separaten Austritt sind ausdrücklich diagnostische Eingaben. Sie werden nicht als native Initialisierung ausgegeben. Das Codefeld enthält einen opaken Hostwert, keinen rekonstruierten Zeiger oder VM-Instruktionsindex. Callback-Fehler halten in der jeweiligen Phase an und behalten vorherige Schreibzugriffe bei. Ein solcher angehaltener Snapshot ist ein Diagnoseergebnis, keine fertig ausgeführte Spielinstanz.

Metadaten bleiben während eines Aufrufs unveränderliche Snapshots; die Callback-Verträge erhalten Objektklasse und Frame-Lebensdauer. Native Overrides, GIsScriptable/ProcessEvent-Ausführung, mutable Klassen-/Zustandsmetadaten, Labelsprünge und die vollständige VM fehlen weiterhin. Der Adapter beweist die untersuchte Basissteuerung, keine vollständige Spiel- oder Startzustandsparität. Androidprüfung weiterhin zum Schluss.

## Folgearbeit: Originalhandler

Alle 21 ausgewählten Originalhandler sind jetzt als AST lesbar. Der exakt geprüfte PendingMatch.BeginState-Adapter schließt 20 Fälle für fünf Klassen auf gelieferten Hostfeldern ab. Weitere 20 Handlerimplementierungen bleiben offen; Prop.BeginState enthält eine noch nicht ausgeführte Umleitung nach Damagable. 187 Tests bestanden. Siehe [LIFECYCLE_HANDLERS.md](LIFECYCLE_HANDLERS.md).
