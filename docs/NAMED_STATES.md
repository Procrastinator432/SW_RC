# Benannte Zustandswahl aus Originaltabellen

`UObject.FindState` (`core.dll:10154910`) sucht den Namen über FindStruct in der Objektklasse und prüft am gefundenen Kandidaten das Klassentypflag `0x40000`. Ein gleichnamiger Treffer ohne State-Typ wird verworfen; die Suche setzt danach nicht in einer weiteren Vorfahrentabelle fort. Die benannte Zielphase von GotoState verwendet bei fehlendem Zustand die Objektklasse als StateNode.

`state_selection::NamedStateLookup` setzt diese Suche für die vorhandenen Hash-Snapshots um. Die Typzuordnung ist ausdrücklich Teil der Eingabe. Fehlende oder unbekannte Typzuordnungen und ungültige StateNode-Verweise liefern Fehler. Die Rust-Zielauflösung liefert NamedState oder ClassFallback. Auto ist durch den festen Namensindex 690 gekennzeichnet und wird als separat zu implementierende Auswahl abgelehnt.

Die Originalprobe akzeptiert bekannte Core.State-Exporte als State-Kandidaten. Weitere Owner-/Metaklassen-Kandidaten bleiben Unresolved, da deren native Metaklassenhierarchie nicht in den geladenen Klassenexporten vollständig vorliegt. Daraus wird kein nicht belegter Typvergleich abgeleitet.

`rc-state-link-probe <GameData> <report.json> --native-hardcoded-names --original-masks --named-states` fragt 143 unterschiedliche Original-Zustandsnamen über 51 Controller-Klassen ab. Ergebnis: 289 gültige Kombinationen und 7004 fehlende Namen mit Klassenfallback. Zusätzlich führen None und der Nicht-State-Name NotifyHitWall für jede Klasse auf ClassFallback: 102 Fälle. Die Treffer werden unabhängig von den Hashtabellen anhand der umgekehrten eigenen Kinderliste und der Elternkette nachgeprüft.

Die Maskenzusammensetzung verwendet jetzt die tatsächlich anfragende Objektklasse und den gefundenen, gegebenenfalls geerbten oder überschriebenen Zustand. Das erweitert die frühere Probe, die nur Zustände unter ihrer deklarierenden Klasse auswählte. Für alle 289 Treffer unterdrückt die Originalmaske NotifyHitWall. Die übrigen bisher geprüften Originaldaten, Masken und Initialframes bleiben unverändert.

170 Workspace-Tests, Clippy mit Warnungen als Fehler und Formatprüfung bestanden. Neue Tests prüfen geerbte Zustände, Überschreibungen, nichtfunktionale beziehungsweise Nicht-State-Schatten, fehlende/None-Namen, Auto-Abgrenzung und unvollständige Eingaben. Nachweise: `analysis/reports/named-state-selection.json`, `named-state-selection-validation.json`, wiederholbare unabhängige Prüfung über `scripts/Record-NamedStates.py`.

Die Anfragen sind explizite Proben, keine Ermittlung des tatsächlich im Spiel aktiven Zustands. Auto-Auswahl, BeginState/EndState, deren rekursive Wechsel, Code-/Latentfelder und VM-Ausführung bleiben offen. Dynamische Namen sind weiterhin diagnostisch, Handles opaque; native Overrides sind nicht rekonstruiert. Androidprüfung bleibt bis zum Schluss verschoben.

## Folgearbeit: Auto-Zielwahl

Die Auto-Zielphase ist jetzt separat implementiert und an Original-Stateflags angeschlossen:517Auto-Ziele und2653Klassenfallbacks,51Controller-Masken geprüft. BeginState/EndState und Runtime-Overrides bleiben offen.173Tests bestanden. Siehe [AUTO_STATES.md](AUTO_STATES.md).
