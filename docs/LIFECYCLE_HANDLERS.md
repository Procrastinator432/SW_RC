# Originale Zustandsereignisse und PendingMatch

Die bisherigen Eintritts-/Austrittsproben wählen in 451 Ereignisanforderungen 21 unterschiedliche BeginState-/EndState-Funktionen aus sechs Originalpaketen aus. Alle 21 Funktionen lassen sich jetzt vollständig mit dem begrenzten AST-Leser untersuchen. Diese Menge umfasst die ausgewählten Funktionen der bisherigen Auto-Proben, nicht sämtliche Ereignisfunktionen des Spiels.

Der fehlende Token war ANSI StringConst (0x1f) in Prop.Invulnerable.BeginState. UStruct.SerializeExpr (`10120d00`) liest einzelne Bytes bis einschließlich Nullterminator; UObject.execStringConst (`1013ce00`) verwendet appFromAnsi und rückt Code hinter den Terminator. Der AST bewahrt jedes Byte als gleichwertigen Unicode-Codepoint, ohne eine native Codepage-Konvertierung zu behaupten. Die logische Größe enthält Token, Zeichenbytes und Terminator. Der Leser stoppt an der deklarierten Skriptgrenze und liest keinen vermeintlichen Terminator aus nachfolgenden Metadaten. UnicodeStringConst (0x34) bleibt explizit ununterstützt.

Die originale Prop-Funktion enthält zwei Strings: eine leere Zeichenkette und ` Is Invulnerable`. Ihre Rohbytes sind gegen das Originalpaket geprüft. Zuvor prüft die Funktion Health gegen 0 und ruft bei positivem Wert GotoState('Damagable') auf. Die Original-Core-Funktionen belegen Greater_IntInt mit NativeIndex 151 und GotoState mit 113. Das erklärt, warum die reine Auto-Zielwahl keine vollständige Aussage über den später aktiven Zustand macht. Die tatsächliche Umleitung und gegebenenfalls Broadcast-Ausführung sind noch nicht angeschlossen.

`pending_match::PendingMatchBegin` implementiert eine vollständig überschaubare Ereignisfunktion: MPGame.DMGame.PendingMatch.BeginState setzt `bWaitingToStartMatch=true` und `StartupStage=0`, danach folgt Return/Nothing. Vor Freigabe des privaten Tokens werden vollständige AST-Struktur, Operanden, Objektpfade/-referenzen, Offsets, Größen, Funktionstyp und Flags mit dem geprüften Original verglichen. Die originalen Felder sind BoolProperty und ByteProperty, beide Dimension 1 und Flags 0. Replizierte Felder werden abgelehnt, da deren native Zuweisung zusätzliche Hooks auslösen kann. Native execLetBool, execLet, execTrue und execByteConst sind separat exportiert und untersucht.

Die semantische Ausführung schreibt typisierte Hostfelder. Sie interpretiert keinen allgemeinen Bytecode, bildet keine gepackten nativen Bool-Speicherwörter ab und ersetzt weder ProcessEvent noch dessen globale/virtuelle Voraussetzungen. Der Aufrufer muss die Identität des tatsächlich gewählten Handlers belegen.

`rc-pending-match-probe` verbindet diesen Adapter mit dem vorhandenen GotoState-Ablauf. Sie verwendet gespeicherte, unabhängig geprüfte Original-Lookup-/Maskensnapshots und liest den Handler sowie die Felddefinitionen erneut aus engine.u und mpgame.u. Fünf Klassen sind betroffen: ASGame, CTFGame, DMGame, TDGame und TDGameDefaults. Nur DMGame wählt DMGame.PendingMatch direkt; die übrigen wählen TDGame.PendingMatch und erben dessen BeginState aus DMGame. Die Probe unterscheidet Zustandsidentität und Handleridentität ausdrücklich. Vier gelieferte skalare Eingaben je Klasse ergeben 20 abgeschlossene Wechsel mit einem BeginState-Aufruf und Ergebnis Success.

```text
cargo build -p rc-inspect --bin rc-script-dump
python scripts/Dump-LifecycleFunctions.py
rc-script-dump <GameData>/System/core.u analysis/reports/lifecycle-core-dependencies.json Object.Greater_IntInt Object.GotoState
rc-pending-match-probe <GameData> analysis/reports/state-transitions.json analysis/reports/pending-match-begin.json
python scripts/Record-LifecycleHandlers.py
```

Nachweise liegen unter `analysis/reports/lifecycle-functions/`, in `lifecycle-handlers-validation.json`, `pending-match-begin.json` und `analysis/decompiled/lifecycle-scalars.c`. Das Inventar listet Aufrufe und Objektreferenzen jeder ausgewählten Funktion für die weitere Rekonstruktion auf. Der bestehende Zustandswechselbericht bleibt als Nachweis seiner damaligen, vor den Callbacks anhaltenden Probe erhalten.

187 Workspace-Tests (177 rc-package, 4 rc-native, 6 rc-render), Clippy mit Warnungen als Fehler und Formatprüfung bestanden. Neue Tests prüfen Zeichenketten, Terminierung, Sprunggrenzen, Trunkierung, veränderte ASTs/Feldtypen, alle Bool-/Byte-Eingabewerte und den Callback im Zustandswechsel.

Die anderen 20 ausgewählten Handler sind lesbar, ihre Ausführung bleibt offen. Dazu gehören Animation, Navigation, Timer, Spieler-/Objektreferenzen, virtuelle Aufrufe und weitere Zustandswechsel. PendingMatch-Startlabels, Konstruktoren, tatsächliche Default-/Config-Initialisierung, allgemeine VM-/ProcessEvent-Ausführung und native Overrides fehlen weiterhin. Es wurde keine laufende Spielsitzung oder Android-Ausführung geprüft. Androidprüfung weiterhin zum Schluss.

## Folgearbeit: Prop-Umleitung

Die Prop-Basishandler für Invulnerable und Damagable sind jetzt als verifizierte Verzweigungen angeschlossen. 1.008 Original-Metadatenfälle für 168 Klassen prüfen rekursive Umleitung und den zusätzlichen skriptseitigen Begin-Labelversuch. Broadcast bleibt offen, AnimProp wird nicht mit dem Basishandler gleichgesetzt. 194 Tests bestanden. Siehe [PROP_TRANSITIONS.md](PROP_TRANSITIONS.md).
