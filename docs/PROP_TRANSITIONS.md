# Prop-Zustandsumleitung und skriptseitiges GotoState

`prop_state::PropBegin` erkennt exakt die untersuchten Originalfunktionen Prop.Invulnerable.BeginState und Prop.Damagable.BeginState. Der Verifizierer prüft Metadaten und die vollständigen ASTs einschließlich Sprungziele, Operanden, Referenzen, Strings, Offsets und Kinderstruktur. Die Vergleichstabellen werden reproduzierbar aus dem Originaldump erzeugt; sie bestimmen nicht selbst die Semantik.

Invulnerable.BeginState vergleicht die gelieferte Health-Ganzzahl vor dem Display-Flag. Bei Health>0 fordert die Funktion GotoState('Damagable') an. Andernfalls kehrt sie ohne Zustandsmeldung zurück oder benötigt Broadcast. Damagable.BeginState benötigt bei aktiviertem Display-Flag ebenfalls Broadcast, sonst folgt unmittelbar Return. Die Originalfelder sind Core.IntProperty und Core.BoolProperty, jeweils Dimension 1 und Flags 1. Der Adapter liest typisierte Hosteingaben; Broadcast, Objekt-zu-String-Konvertierung und Level/Game-Referenzen bleiben externe Abhängigkeiten.

Der native Ganzzahlvergleich `10130c00` verwendet CMP und **SETG**, also einen signierten Vergleich. Die Ghidra-C-Ausgabe reduziert diese Funktion fehlerhaft auf eine konstante Null; maßgeblich ist deshalb der exportierte Assembler `analysis/decompiled/script-goto-state.asm`. Grenzfälle einschließlich i32::MIN und i32::MAX sind geprüft.

`script_state::script_goto_state` ergänzt den Basiswechsel um den Ablauf von execGotoState (`1013ba80`): Bei gleichem angefordertem und bisherigem Namen wird GotoState übersprungen und direkt das Label gesucht. Nach einem Basiswechsel erfolgt die Labelsuche nur bei Ergebnis 1; Ergebnis 2 (Preempted) löst keine weitere Suche aus. Eine fehlende oder None-Labelangabe verwendet den festen Namen Begin mit Index 100. Fehlende explizite Labels und unbekannte Zustände werden als getrennte Warnwerte ausgegeben. Ohne explizites Label verursacht ein fehlendes Begin-Label keine Warnung; fehlende Zustände None und Auto sind von der Zustandswarnung ausgenommen.

Der Wrapper arbeitet mit bereits gelieferten Argumenten ohne zustandsändernde Auswertungseffekte. Native Argumentauswertung, ProcessEvent, Logging-Ausgabe und virtuelle Überschreibungen werden nicht ausgeführt. Die Labeloperation muss der Host ausdrücklich bereitstellen. Die Original-Prop-Probe erlaubt einen fehlenden Labeltreffer nur, wenn bei sämtlichen tatsächlichen State-Vorfahren LabelTableOffset=0xffff oder die Skriptgröße 0 ist. Native GotoLabel (`10137920`) überspringt solche Tabellen, löscht LatentAction und setzt Code bei einem Fehlschlag auf null. Gefundene Labeltabellen werden hier noch nicht ausgeführt.

Die neue Probe ist mit den frisch gelinkten Originaltabellen, Namen und Masken verbunden. Sie umfasst die 168 Klassen, deren Auto-Ziel tatsächlich den Prop-Basishandler auswählt. AnimProp.Invulnerable.BeginState mit zusätzlicher Animation wird nicht als Prop-Basishandler ausgegeben. Zustände und Handler werden getrennt behandelt: KarmaProp hat eigene Invulnerable-/Damagable-Zustände mit geerbten Prop-Ereignisfunktionen.

Sechs explizite Eingaben je Klasse ergeben 1.008 Fälle:

| Ergebnis | Fälle | Bedeutung |
|---|---:|---|
| Success | 336 | Health<=0 und Display=false; Invulnerable bleibt aktiv. |
| Preempted | 336 | Health>0 und Display=false; der innere Wechsel nach dem klassenspezifischen Damagable-Zustand ist abgeschlossen, der äußere Wechsel wird korrekt abgebrochen. |
| UnresolvedBroadcast | 336 | Display=true; die Probe hält vor Broadcast in Invulnerable beziehungsweise Damagable an. |

Alle 336 abgeschlossenen inneren Wechsel suchen Begin, finden nachweislich kein Label und erzeugen dafür keine Warnung. Bei angehaltenem Broadcast im inneren BeginState findet noch keine Labelsuche statt. Diese Phasen werden im Bericht getrennt protokolliert; ein angehaltener Snapshot ist keine fertig ausgeführte Spielinstanz.

```text
rc-script-dump <GameData>/System/engine.u analysis/reports/prop-state-dependencies.json Prop.Invulnerable.BeginState Prop.Damagable.BeginState
python scripts/Generate-PropShapes.py
rc-state-link-probe <GameData> analysis/reports/prop-transitions.json --native-hardcoded-names --original-masks --named-states --state-transitions --prop-transitions
python scripts/Record-PropTransitions.py
```

`Record-PropTransitions.py` berechnet die benannte Zustandswahl, geerbte Handlerwahl und Masken unabhängig aus Original-Kinderlisten und Elternketten nach. Alle 1.008 Snapshot-Ergebnisse, Ereignisreihenfolgen, inneren Wechsel und Labelbedingungen werden geprüft. Die bisherigen Berichtswerte sind unverändert. Nachweise: `prop-transitions.json`, `prop-transitions-validation.json`, `prop-state-dependencies.json` und `script-goto-state.c/.asm`.

194 Workspace-Tests (184 rc-package, 4 rc-native, 6 rc-render), Clippy mit Warnungen als Fehler und Formatprüfung bestanden. Neue Tests prüfen signierte Gesundheitsgrenzen, Vorrang der Umleitung, veränderte ASTs/Feldtypen, rekursive Wechsel einschließlich Labelversuch sowie gleiche Namen, fehlende Zustände und explizite/implizite Labels.

Die Health-/Display-Werte sind gelieferte Diagnoseeingaben, keine rekonstruierte Default- oder Config-Initialisierung. Broadcast, AnimProp-Animationen, allgemeine Labelausführung, VM, native Overrides und tatsächliche Spielsitzungen bleiben offen. Androidprüfung weiterhin zum Schluss.

## Folgearbeit: AnimProp-Wrapper

Die zusätzliche AnimProp-Funktion ist jetzt mit festem Prop-Superaufruf und anschließendem Animationsabschnitt verbunden. 576 Fälle für 32 Klassen prüfen insbesondere die Fortsetzung nach einer Damagable-Umleitung. None schließt ab; nicht leere PlayAnim-/LoopAnim-Anforderungen halten weiterhin vor der Wiedergabe an. 198 Tests bestanden. Siehe [ANIM_PROP_TRANSITIONS.md](ANIM_PROP_TRANSITIONS.md).
