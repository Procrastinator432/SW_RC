# Granatenflug in der Originalwelt und korrigierte Arena-Oberflächen

Stand:12. Oktober2026. Belege und korrigierte Szene liegen unter `analysis/mission4-world-flight`.

## Direkt ausprobierbare Texturkorrektur

**`geo_01a-grenades-arena.rcplay`** lässt sich mit der vorhandenen App **0.21.0** öffnen. Sie ersetzt die bisherige Granatenszene und enthält denselben spielbaren Ablauf bis zur Aufnahme von fünf Thermaldetonatoren. Eine neue APK ist für diese reine Szenenkorrektur nicht nötig. Der Wurf und Originalziel3 sind noch nicht im Game-/Android-Host integriert.

Die Prüfansicht am Geschütz zeigte große graue Rasterflächen. Die Materialinventur weist14 originale Arena-Combiner mit `CO_Multiply` nach: Material1 ist eine Lichttextur, Material2 enthält die eigentliche Oberflächentextur. Der bisherige einfache Diffuse-Export wählte nur Material1. Eine vorhandene Texturbindung war deshalb noch kein Beleg für eine korrekt dargestellte Oberfläche.

`rc-arena-surface-repair` bindet **13.743 statische Flächen** auf die originalen Oberflächeneingänge um. Die verschachtelten TexCoordSource-Objekte bestimmen den jeweiligen UV-Kanal; `UpperW_Scalar` verwendet die inverse UScale aus `UTexScaler::GetMatrix`10452400. Nichtzero Offsets, andere verschachtelte Operationen und widersprüchliche Bindungen auf deckungsgleichen Dreiecken werden abgelehnt. Die Zuordnung erfolgt über exakte geordnete Original-Weltkoordinaten, da frühere Exporte diagnostische Instance-Farboffsets auf die Surface-Slots addierten und spätere Builds Actors/FX entfernten.

Das ist weiterhin eine **Oberflächenvorschau**: Lichttextur-Multiplikation und Vertex-Mischmasken werden nicht vollständig ausgeführt. Die neue Ansicht zeigt ursprünglichen Stein, Putz und Stufen statt einer isolierten Lichttextur. Drei CPU-Ansichten der tatsächlichen Flugpositionen sind gespeichert und mit Welt-Tiefe geprüft. Gemeinsame GPU-Framepakete sind erzeugt; eine Android-GPU-Abnahme dieser Szenenkorrektur ist noch nicht erfolgt.

Metadaten, Weltgeometrie und Missionsregeln bleiben erhalten. Der initiale Spielzustand ist identisch. Der vollständige Rust-Lauf endet wie zuvor anTick5030 mit fünf Abschüssen, fünf Granaten, allen13 Audio-Rollen und exaktem Neustart. Derselbe Lauf besteht über die bestehende native Desktop-ABI einschließlich Puffer-/Handle-Prüfungen. Der Szenenhash lautet `5b6f66be5bb35f940f75aa092bf89aa9409d064a938425824fec1aafbee79e5c`.

## Physischer Anschluss an Originaltrigger621

`detonator_flight.rs` führt einen Punkt-Actor mit Originalkollisionsradius/-höhe0 durch die übergebene Kollisionswelt. Er verwendet die zuvor bitgenau geprüfte Actor-Fallarithmetik und das native Budget von acht Iterationen mit zwei möglichen Bounce-Zeitrefunds. Jedes Segment endet an der ersten blockierenden Fläche. Nur diese tatsächlich durchlaufenen Segmente werden dem GrenadeProj-Kontaktgeber übergeben. Ein Test mit und ohne Wand vor demselben Ziel stellt sicher, dass der Auslöser nicht durch die Wand reagiert.

`GetNetBuoyancy`1048dd40 ist lokal aus der Originalengine exportiert. Im trockenen Standardgravitationsfall bleiben die zuvor nullinitialisierten Ausgaben null. Abweichende vertikale Gravitation ergibt halbe Actor-Buoyancy und halbe Volume-FluidFriction. Die echten Projektildefaults sind Mass100 und Buoyancy0; drei geladene Volumen liefern FluidFriction0.3. Die Auswahl wird vor jedem Bewegungsschritt erneut geprüft. Wasser, bewegte Volumen, unvollständige Volumen-/Kollisionsdaten und eindringende Ausgangslagen werden ausdrücklich abgelehnt; Fehler setzen weder Bewegungszustand noch Zünder teilweise fort.

Der Weltprüfer läuft kontinuierlich vom Original-Pickup610 über fünf weitere Wegpunkte bis PathNode125/Export674. Er benutzt den tatsächlich erreichten Pawn-Standpunkt, dessen Auge und den ursprünglichen FPGrenadeOffset `[57.4000015,-30,-10.5600004]`. Nach sechs geprüften Wurfwinkeln erreicht die Granate Trigger621 bei aktivem Zünder. Sie trifft vorher `Prop4` und prallt ab: Der erfolgreiche Kontakt entsteht aus der danach weiterhin kollisionsbegrenzten Flugbahn. Die vollständige Flugspur ist vor und nach der Texturkorrektur bytegleich.

Dies ist ein physischer Machbarkeitsnachweis im vorbereiteten Host, noch kein vom Benutzer ausgelöster Wurf im Spiel. Die bisherige Kollisionswelt ist aus der Spieler-Blockerauswahl übernommen; sämtliche originalen Projektilfilter, Actor-Touches, LastTouchedActor und bewegte Kollisionskörper sind noch nicht vollständig rekonstruiert.

## Ruhe, Zünder und verbleibende Grenzen

Das Original `AActor::processLanded`104906c0 setzt nach Landed bei weiter bestehendem PHYS_Falling auf PHYS_None und Nullgeschwindigkeit. Der begrenzte Host übernimmt diesen Übergang bei einem Bodenkontakt nach abgeschaltetem Bouncing und Normal.Z>=0.7. Die0.7-Konstante stammt aus der Originalengine. Der anschließende Zünder läuft weiter und erzeugt genau einmal Explode/Destroy.

Der Übergang nach RestSpeed bleibt ein **begrenzter Adapter**: Ohne Bouncing wird bei seitlichen Wänden die einwärts gerichtete Geschwindigkeit entfernt. Native TwoWallAdjust-, Slide- und Geschwindigkeitsrekonstruktionspfade, Landed-Scriptoverrides, Base-Anheftung und Rotationsanpassung sind noch nicht gleichwertig. Auch die Kontaktseparation von0.01Einheiten ist eine eigene Hosttoleranz. Die Scriptuhren laufen im Host vor der Bewegung des jeweiligen Frames; ein abgelaufener Actor bewegt sich nicht weiter. Diese Reihenfolge ist eine explizite Hostentscheidung und kein Nachweis der gesamten nativen Tickreihenfolge.

**1031 Bibliothekstests** bestehen, darunter neun neue Fälle zu trockener Buoyancy, Weltkontakt, Wandabschirmung, Ruhe/Detonation, Lebensdauer, nicht unterstützten Volumen und atomarem Fehlerverhalten. Der reale Weltlauf, Texturprüfung und native Regression sind zusätzlich abgelegt. Ältere Lieferungen und ihre archivierten Belege bleiben unverändert.

Nächste Integration: Flug-/Wurfzustand optional an die Mission binden, Game-/JNI-/Touch-Wurfeingabe und tatsächliche Waffensperre verbinden, Originalmesh und Foley/Bounce/Explosion wiedergeben, sichtbare Schadensziele auswählen und Bunker/Hatch/Kollision nach den Originalereignissen verändern. Danach folgt der komplette Android-Lauf bis zum Geschützabschluss und eine neue APK samt passender Szene.
