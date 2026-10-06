# BSP-Kollisionshüllen und Körperprüfungen

Stand 6. Oktober 2026. `rc-package::hull` liest originale Model-Kollisionshüllen und berechnet mathematische Sweeps eines achsenparallelen Körpers. Der neue PC-Audit prüft damit einen Körper in PlayerCommando-Größe. Er ersetzt noch nicht die native UModel::LineCheck-Routine und ist noch nicht in Android-Bewegung eingebunden.

## Originales Hüllenformat

Der FBspNode-Serializer speichert nach den Back-/Front-/Coplanar-Verweisen den CompactIndex `iCollisionBound` (natives Feld +0x3c), anschließend den Render-Bound-Verweis. Der Geometrie-Reader erhält den Kollisionsverweis jetzt ausdrücklich. Der Model-Tail-Reader erhält außerdem die u32-Wörter der bereits bekannten Hull-Tabelle.

`iCollisionBound` ist ein Wortoffset in diese Tabelle. Dort folgen BSP-Node-Verweise bis zum Sentinel 0xffffffff. Bit 0x40000000 kehrt die referenzierte Ebene um; alle vier Ebenenkomponenten wechseln ihr Vorzeichen. Nach dem Sentinel stehen sechs f32-Werte als u32-Bitmuster: Minimum XYZ und Maximum XYZ. Der native Helfer besitzt Platz für 64 Ebenen. Der Reader weist fehlende Sentinels, Überschreitungen, ungültige Verweise, nicht endliche/degenerierte Ebenen, gekürzte oder invertierte Bounds ab.

Die Körperprüfung benutzt nur Hüllen an erreichbaren festen BSP-Blättern. Ein Node gilt im nativen Extent-Pfad als CSG, wenn seine Vertex-Anzahl ungleich null und Flags & 0x21 gleich null sind. Der freie Zustand lautet auf der Front-Seite outside || csg und auf der Back-Seite outside && !csg. Beim Laden maskiert der native Serializer zuvor Flags auf 0x1f. Der Reader folgt diesen Zuständen mit RootOutside, dedupliziert Hüllen und meldet feste Blätter ohne Hülle ausdrücklich. Ein solcher Mangel darf keine freie Körperprüfung ergeben.

## Mathematischer Sweep

Der Sweep verschiebt ein AABB mit frei gewählter halber Ausdehnung von Start nach Ende. Neben den Hüllenflächen werden Box-Achsen und Ebenen aus Hüllenkanten × Box-Achsen berücksichtigt. Positive Kombinationen benachbarter Flächen erzeugen gültige Kantenebenen; zusätzliche gültige Kombinationen sind redundant. Erst danach werden Ebenen um den Stützradius sum(abs(normal[i]) * extent[i]) erweitert.

Das verhindert Eckfehler einer Prüfung, die nur vorhandene BSP-Flächen verschiebt. Ein Test verwendet einen Tetraeder: Eine Box außerhalb seiner Ecke erfüllt die einzeln erweiterten Flächen, wird aber durch die zusätzliche Kantenebene korrekt ausgeschlossen.

Die Linienintervalle werden an den erweiterten Ebenen geschnitten. Ergebnis: Eintrittsanteil, Austrittsanteil, Normale, Hüllenoffset und anfängliche Überlappung. Kontakt gilt als Treffer; anfängliche Überlappung liefert Anteil 0. Es gibt noch keine Depenetration, Sliding-, Boden- oder Stufenlogik. Fehlende Hüllen und ungültige Eingaben liefern Fehler. Die Prüfung nutzt f64 und mathematische Grenzfälle; native f32-Rundung, Toleranzen, Backoff und FCheckResult-Parität sind ausdrücklich noch nicht verifiziert.

## Originaldaten-Prüfung

`rc-hull-check` lädt den Klassendefault-Katalog und löst CTCharacters.PlayerCommando.CollisionRadius=40 und CollisionHeight=84 mit Herkunftsnachweis auf. Actor.uc definiert CollisionHeight als halbe Zylinderhöhe. Für diese BSP-Diagnose wird daraus die AABB-Ausdehnung [40,40,84]. Startpunktpositionen dienen nur als Test-Körpermittelpunkte; die tatsächliche Spiel-Spawn-Auswahl und Platzierung sind nicht umgesetzt.

| Befund | Anzahl |
|---|---:|
| Karten strukturell geprüft | 79 |
| Model-Exports geprüft | 9.123 |
| Aus festen Blättern gewählte Kollisionshüllen | 26.840 |
| Hüllen-Lesefehler / feste Blätter ohne Hülle | 0 / 1 |
| Karten mit nach Namen ausgewähltem Model1 | 34 |
| Startpunkte für diese Model1-Proben | 252 |
| Körperprüfungen über je 1.000 Units | 1.512 |
| Blockiert / frei | 1.019 / 493 |
| Query-Fehler | 0 |

Alle 755 Startpunkt-Transforms werden weiter aufgelöst. 503 liegen in Karten ohne das bisher nach Namen gewählte Model1 und werden hier nicht als Körperproben ausgegeben. Die Weltmodell-Referenz muss für diese Karten aus dem Level-Objekt gelesen werden; ein geratenes Ersatzmodell wird nicht verwendet. Auch Model1 ist bislang nur eine Namensauswahl und kein allgemeiner Nachweis der Level-Bindung.

In geo_01a.Model1 sind 125 feste Hüllen mit insgesamt 601 Ebenen ausgewählt. Der Abwärtssweep vom PlayerStart0 trifft bei Anteil 0.6062015380859375, mit Normale [0,0,1]. Der Vergleichssweep ohne Ausdehnung trifft bei 0.6902015380859375: Der Körper berührt den Boden 84 Units früher. [hull-validation.json](../analysis/collision/hull-validation.json) enthält sämtliche Models, Referenzprüfungen, Körper- und Punkt-Sweeps.

53 Rust-Tests und Clippy über alle Targets bestehen. Neue Tests prüfen Hüllen-Flip, Sentinels, Grenzen und Trunkierungen, das 64-Ebenen-Limit, feste/nicht-CSG-Blätter, fehlende Hüllen, Sweep-Anteil in beiden Richtungen, anfängliche Überlappung, ungültige Ausdehnung und den Tetraeder-Eckfall. Android-APK 0.7 bleibt unverändert; diese PC-Änderung wurde nicht als neue Emulatorfunktion ausgegeben.

## Native Belege

- [FBspNode-Serializer](../analysis/decompiled/bsp-layout.c): engine.dll 0x1045e000.
- [Extent-Kontext und Traversierung](../analysis/decompiled/bsp-extent.c): 0x10553400 und 0x10556aa0.
- [Hüllen laden, Ebenen clippen, Kanten und Grundkontext](../analysis/decompiled/bsp-hulls.c): 0x105522f0, 0x10553540, 0x10553b10 und 0x10552240.
- [UModel::LineCheck](../analysis/decompiled/collision-checks.c): 0x105592b0.

Der native Pfad besitzt zusätzliche Toleranzen, Sonderfälle für Startüberlappung, Traceflags, Actor-Transformationen und Ergebnisattribute. Die oben beschriebene Mathematik ist ein überprüfter Baustein für den Port, keine Behauptung vollständiger nativer Verhaltensgleichheit.

## Reproduzieren und nächster Schritt

```powershell
cargo run -p rc-inspect --bin rc-hull-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/collision/hull-validation.json
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Als Nächstes die Level-Weltmodell-Referenz auflösen, Körperprüfungen mit Actor-transformierten Mesh-Formen zusammenführen und native Grenzfälle abgleichen. Spielerbewegung kann erst danach mit Kontakt, Sliding, Boden und Stufen eingebunden werden.


Die Weltmodell-Auswahl ist inzwischen über den Level-Serializer aufgelöst und auf alle 755 Startpunkte erweitert; gemeinsame Körperproben gegen transformierte Mesh-Hüllen sind verfügbar. Die sechs Proben in ras_02a bleiben wegen des einen hüllenlosen festen Blatts ausdrücklich unvollständig. Die ursprüngliche Angabe 0 fehlende Hüllen war ein Auswertungsfehler; das alte JSON enthielt bereits 1. Aktueller Stand: [LEVEL_BINDING_AND_BODY.md](LEVEL_BINDING_AND_BODY.md). Android-Prüfung erfolgt auf Wunsch des Nutzers am Ende.
