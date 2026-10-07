# Ereignismasken aus Originaldaten

`state_masks::read` liest ProbeMask, IgnoreMask, LabelTableOffset und StateFlags nach dem serialisierten Struct-Script. Dafür traversiert ein begrenzter Layoutleser den Code gemäß `UStruct.SerializeExpr` an core.dll:10120d00. Kompakte Objekt-/Namensreferenzen zählen logisch vier Bytes; Labeltabelleneinträge zählen acht Bytes einschließlich der None-Endmarke. Unbekannte Tokens, Überschreitungen und Trunkierungen liefern Fehler. Es findet keine Ausführung, AST-Auswertung oder Prüfung aller Sprungziele statt. Nicht implementierte Layoutformen werden ausdrücklich abgelehnt.

Bei UState muss der Payload nach den Maskenfeldern vollständig verbraucht sein. Bei UClass folgt weiterer Klassendateninhalt; dieser Helfer liest nur bis einschließlich der State-Metadaten. `function_flags` traversiert die relevanten Funktionslayouts bis zum Ende und berücksichtigt den optionalen ReplicationOffset nach den Flags, statt Flags aus der letzten Payloadposition zu erraten.

Der native UClass-Serializer (`10126200`) übernimmt beim Laden die ProbeMask der Elternklasse und ergänzt Bits eigener Probe-Funktionen, deren Funktionsflags Bit2wert (`flags & 2`) enthalten. Gemeint ist Bitwert2, nicht Bitnummer2. Die Funktion muss einen fest registrierten Namensindex im Bereich300..363 besitzen. Die gespeicherte Klassen-ProbeMask darf deshalb nicht ungeprüft als Laufzeitmaske verwendet werden. Originale UState-Probe-/Ignoremasken werden separat gelesen.

`rc-state-link-probe <GameData> <report.json> --native-hardcoded-names --original-masks` verbindet diese Daten mit dem Tabellenaufbau. Alle3170Klassen und260Zustände konnten gelesen werden. Klassenmasken werden elternweise aus Flags neu berechnet, anschließend alle3430Maskenkompositionen `(ClassProbe | StateProbe) & StateIgnore` erzeugt. Bei Klassenzielen wird die neu berechnete Klassenmaske auch für StateProbe verwendet. Bei28Klassen weicht die berechnete ProbeMask vom gespeicherten Wert ab.

Für31 deklarierte Controller-Zustände wählt die Probe ausdrücklich die deklarierende Klasse als Objektklasse und den jeweiligen Zustand als Ziel. In allen31Fällen ergibt die Originalkomposition für NotifyHitWall MaskedOut. Es wird kein tatsächlicher Runtimezustand ermittelt und kein vollständiger GotoState-Aufruf ausgeführt. Geerbte Zustände unter allen denkbaren abgeleiteten Objektklassen sind damit nicht vollständig abgedeckt. Die Ergebnisse ersetzen nur für diese expliziten Kombinationen die früher gelieferten Masken.

167Workspace-Tests, Clippy mit Warnungen als Fehler und Formatprüfung bestanden. Neue Tests prüfen Labeltabellenbreiten, Maskenwörter, alle Trunkierungen der Fixtures, State-Payloadende, Function-Net-Trailer sowie unbekannte Tokens, Überläufe und nicht abgeschlossene Strings/Labeltabellen. Die Originaldatenprobe wurde zusätzlich mit `scripts/Record-OriginalMasks.py` unabhängig aus Flags und Elternhierarchie nachgerechnet. Bisherige Kinderlisten,51Controller-Klassensuchen und Initialframe-Proben bleiben unverändert.

Nachweise: `analysis/reports/original-state-masks.json`, `original-state-masks-validation.json`, native Serializer in `analysis/decompiled/class-serializers.c`. Weitere Grenzen: dynamische Namensregistrierung, tatsächliche aktive/automatische Zustandswahl, BeginState/EndState und andere VM-Seiteneffekte, native Overrides. Keine neue Karten- oder Androidprüfung; Android bleibt bis zum Schluss verschoben.

## Folgearbeit: benannte Zustandswahl

Die Probe löst jetzt benannte Zustände über die tatsächliche anfragende Controller-Klasse auf, einschließlich Vererbung und Klassenfallback. 289gültige Kombinationen aus51Klassen/143Namen geprüft; Masken unterdrücken NotifyHitWall in diesen289Fällen.170Tests bestanden. Siehe [NAMED_STATES.md](NAMED_STATES.md).
