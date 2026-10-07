# AnimProp-BeginState und offene Animationsanforderungen

`AnimPropBegin` ergänzt die exakt geprüfte Funktion Engine.AnimProp.Invulnerable.BeginState. Ihr erster Aufruf ist ein FinalFunction-Token (0x1c) mit der festen Referenz auf Prop.Invulnerable.BeginState, Export 11676. Der Superaufruf wird deshalb nicht erneut anhand des inzwischen aktiven Zustands aufgelöst. Vollständige AST-Struktur, Referenzen, Sprungziele, Operanden, Größen und Offsets sind mit dem Originaldump verglichen.

Die Funktion führt zuerst den Prop-Superhandler aus und liest anschließend AnimHealthy.Anim und AnimHealthy.bLoop. None überspringt beide Animationsaufrufe; ein anderer Name wählt PlayAnim oder LoopAnim. Die Original-UFunctions belegen die NativeIndices 259 und 260. AnimHealthy ist eine StructProperty vom Typ AnimProp.PropAnimInfo; die inneren Felder sind NameProperty und BoolProperty, jeweils Dimension 1. Die Felder werden als typisierte Hosteingabe geliefert, nicht aus einer laufenden Spielinstanz übernommen.

Eine erfolgreiche Umleitung des Superhandlers nach Damagable beendet die laufende BeginState-Funktion nicht. ProcessInternal (`1013c1c0`) setzt seine lokale Funktionsschleife bis Return (0x04) fort. Die Schleife prüft nach einem Aufruf kein StateChanged-Bit. FinalFunction (`1012f6a0`) nimmt die feste Funktionsreferenz und ruft CallFunction virtuell auf. Diese untersuchte Reihenfolge ist im Adapter erhalten: Der Superaufruf liefert die danach gelesenen Animationsfelder zurück; nur ein Fehler hält vor dem Animationsabschnitt an.

NotEqual_NameName hat NativeIndex 255. Der Assembler von `10138a70` vergleicht die FName-Handles mit CMP/SETNZ. Die Decompiler-C-Ausgabe zeigt hier erneut fälschlich eine konstante Null; der Assembler ist maßgeblich. Der Adapter vergleicht deshalb die None-Handleidentität, nicht einen Namenstext oder den numerischen Index.

Die Originalprobe umfasst die 32 Klassen, deren Auto-Ziel tatsächlich AnimProp.Invulnerable.BeginState auswählt. Sechs gelieferte Health-/Display-Eingaben werden mit drei Animationsvarianten kombiniert: None, Play und Loop. Für die beiden nicht leeren Varianten dient der feste FName Tick (Index 336) ausschließlich als Diagnosetoken. Das belegt weder einen vorhandenen Animationsclip noch erfolgreiche Wiedergabe.

| Ergebnis | Fälle | Grenze |
|---|---:|---|
| Success | 64 | Keine Zustandsumleitung, keine Animation nötig. |
| Preempted | 64 | Innerer Damagable-Wechsel abgeschlossen, keine Animation nötig. |
| UnresolvedAnimation | 256 | PlayAnim beziehungsweise LoopAnim angefordert; vor nativer Wiedergabe angehalten. |
| UnresolvedBroadcast | 192 | Bereits der Superhandler benötigt Broadcast; kein Animationsaufruf. |

Insgesamt sind 576 Fälle geprüft. Es gibt 128 PlayAnim- und 128 LoopAnim-Anforderungen. 128 dieser Anforderungen folgen einem bereits abgeschlossenen inneren Wechsel nach Damagable. Dessen StateChanged-Bit ist dann gesetzt, obwohl der äußere BeginState-Aufruf noch an einer offenen Animation hängt. Der Bericht unterscheidet diesen Fall ausdrücklich von einem vollständig abgeschlossenen äußeren Wechsel.

Die Prüfungen berechnen Zustands-/Handlerwahl, Masken und alle Snapshotfelder unabhängig aus Original-Kinderlisten und Elternketten nach. Feste Superreferenz, Ereignisreihenfolge, fehlende Labels und Fehlergrenzen sind geprüft. Frühere Prop-Proben und sämtliche übrigen Berichtswerte bleiben unverändert.

```text
rc-script-dump <GameData>/System/engine.u analysis/reports/anim-prop-dependencies.json AnimProp.Invulnerable.BeginState Actor.PlayAnim Actor.LoopAnim
rc-script-dump <GameData>/System/core.u analysis/reports/anim-prop-core-dependencies.json Object.NotEqual_NameName
python scripts/Generate-PropShapes.py
rc-state-link-probe <GameData> analysis/reports/anim-prop-transitions.json --native-hardcoded-names --original-masks --named-states --state-transitions --prop-transitions --anim-prop-transitions
python scripts/Record-AnimPropTransitions.py
```

Nachweise: `anim-prop-transitions.json`, `anim-prop-transitions-validation.json`, `anim-prop-dependencies.json`, `anim-prop-core-dependencies.json` und `analysis/decompiled/anim-prop-flow.c/.asm`. Der gemeinsame Formgenerator prüft jetzt sowohl Prop- als auch AnimProp-Dumps.

198 Workspace-Tests (188 rc-package, 4 rc-native, 6 rc-render), Clippy mit Warnungen als Fehler und Formatprüfung bestanden. Neue Tests prüfen die Reihenfolge nach dem Superaufruf, None unabhängig vom Loop-Flag, Fortsetzung nach einer Zustandsumleitung, Fehlerweitergabe sowie veränderte Superreferenzen, NativeIndices, Sprungziele und Strukturtypen.

Zum Stand dieses Meilensteins blieben native Animationswiedergabe, optionale PlayAnim-/LoopAnim-Parameter und ihre Defaults, Clipdaten, Broadcast, allgemeine VM-/ProcessEvent-Ausführung und virtuelle Overrides offen. Ein erfolgreich verarbeiteter Hostrequest wäre noch kein Nachweis nativer Animation. Hier halten die Originalproben bei jedem nicht leeren Animationsrequest an. Echte Default-/Config-Initialisierung und Android-Ausführung wurden nicht geprüft. Androidprüfung weiterhin zum Schluss.

Der folgende Meilenstein [Native Animationsaufrufe](ANIMATION_CALLS.md) ergänzt die nachgewiesenen optionalen Argumentdefaults und untersucht die Basisfunktion bis zur Mesh-Wiedergabegrenze. Dieser frühere Bericht und seine Zustandsresultate bleiben unverändert.
