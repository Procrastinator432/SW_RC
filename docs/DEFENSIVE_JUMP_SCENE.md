# M3l: spielbare defensive Sprünge

Version **0.18.0-defensive-jump-scene / Code20**, 2026-10-11. Die begrenzte geo_01a-Szene führt nach dem Nahkampf defensive Sprünge zu originalen JumpDest-Knoten aus: **TakeOff → FallBreathe → Land → 0,5 s Wartephase → Verfolgung/Rückkehr**. M3 als gesamter Original-Kampfablauf bleibt in Arbeit.

## Ausprobieren

Zusammengehörige Dateien unter `analysis/milestone3l`:

- `republic-commando-0.18.0.apk` für ARM64 und x86_64 installieren.
- `geo_01a-easy.rcplay`, `geo_01a-normal.rcplay` oder `geo_01a-hard.rcplay` auf das Gerät kopieren und über **Spielabschnitt geo_01a öffnen → Szene öffnen** laden.
- Zum Beobachten der Sprünge zunächst **Leicht** wählen. Normal und Schwer behalten die Original-Schadensfaktoren; ohne Ausweichen kann der Spieler dort bereits vor dem ersten Sprung sterben.

Die vorhandene Szene, drei Gegner, DC-17m, Exfil und normale Touch-Bedienung bleiben erhalten. APK und Szenen bilden eine neue Lieferung; ältere Versionen liegen weiterhin in ihren bisherigen Verzeichnissen.

## Originalbelege und Umsetzung

Die nachgelagerte Zufallsziehung erfolgt jetzt beim Einreihen des Melee-Goals, wie im Originalzweig 1003a2f1..1003a328. Nach dessen Abschluss wird eine vorgemerkte Sprunganforderung ausgeführt. Quelle sind die Originalwahrscheinlichkeiten 0,2 beziehungsweise 0,8 für Spielerziele. Verwendet wird weiterhin der deterministische Encounter-Zufallsstrom; das ist kein identischer globaler PC-Prozesszustand.

Die 15 originalen `Engine.JumpDest`-Actors werden in der Reihenfolge der originalen Navigationsliste eingebunden. Native Knotenauswahl und defensive Goal-Planung werden mit vorbereiteten Verfügbarkeits-/Erreichbarkeitssnapshots aufgerufen. Gleichzeitige Sprünge reservieren unterschiedliche Ziele. Perches werden von diesem Ausführungshost noch nicht angeboten.

Der bisher ungeklärte Sequenzversatz ist durch `UMeshAnimation::PostLoad` belegt: Bei erstem Referenzknochen **Move** und `word_50=0` werden erste und letzte signierte komprimierte Position des ersten Tracks separat dekodiert und subtrahiert. TakeOff ergibt **[165, 0, 49.998016]** vor der bestehenden Encounter-Skalierung. `FName::StaticInit` in Core.dll belegt **TakeOff=0x374** und **Land=0x375**, passend zu den JumpTo-Zugriffen `Names+dd0/dd4`.

`APawn::GetMaxSpeed` liest bei Flugphysik `Pawn+530`; zusammen mit den Pawn-Feldern ist dies **AirSpeed**, hier original **2000**. `GOAL_ChangePhysics::OnActivate` belegt die Startmaske **0x15**: Fallphysik2, Idle2 und vorbereitete Geschwindigkeit. Die vorhandene Flugbahnberechnung bestimmt diese Geschwindigkeit. Die gewöhnliche Landung verwendet gestarteten Sprung, Walking1 und Abstand² **< Radius²**, hier Radius50. Exakte Gleichheit bleibt aktiv; ein zu großer Abstand führt zum Fehler.

Die Originalclips TakeOff, FallBreathe und Land sind im Szenenpaket enthalten. TakeOff verschiebt den Actor entlang seiner Originalkurve; die Renderpose sperrt den bereits angewendeten Move-Track gegen doppelte Translation. Flug, Landung und Rückkehr benutzen dieselbe Kollisionswelt wie die übrige Szene. Treffer, Gegnertod, Spielertod, fehlerhafte Kollision und Neustart geben laufende Anforderung/Reservierung frei. Fehler in unvollständigen Weltabfragen rollen den gesamten festen Takt zurück.

## Begrenzter Host

Die komplette Start-/Flug-/Landekette ist spielbar. Native GoalQueue, physFalling, dynamische Actor-Kollision, Wand-/Decken-Perches und Rotationsinterpolation sind weiterhin nicht vollständig portiert. Die folgenden Regeln sind Port-Adapter:

- Körpergröße, Mesh-Skalierung, sofortige Ausrichtung und Animationswahl.
- Erreichbarkeit durch vorab geprüfte statische Flugbahnen, gleiche ausgewählte Physikzone und vorhandenes Gehnetz; dies ist keine originale ReachSpec-/ZoneInfo-Abnahme.
- Ballistische Bewegung mit f64-Positionen, eingefrorener zonaler Gravitation und erneuten Sweeps je festem Takt. Portable Kehrwurzel-Schätzung bleibt eine explizite Mathematikgrenze.
- Zielzentrum durch Bodenprüfung unter dem originalen JumpDest für die kleinere Encounter-Kapsel.
- Manche Landepunkte sind über den begrenzten Gehgraphen nicht erreichbar. Der Host verlangt daher auch einen freien Rücksprung und führt ihn bei fehlendem Gehweg aus. Dieser Rücksprung zum Ausgangspunkt ist eine eigene Umsetzung des Rückkehrbedarfs, keine rekonstruierte originale Goto-Bewegung.
- Nach der Wartephase übernimmt der bestehende Geh-/Verfolgungshost. Vollständige native Follow-/Goto-Toleranzen und beliebige Goal-Kombinationen sind noch offen.

## Prüfung und Lieferung

985 Bibliothekstests, Clippy, Formatprüfung, der historische Replay mit 1.002 Kommandos und die unveränderten M3i-Szenen bestehen. Die vorbereiteten nativen Vergleichsfälle umfassen 4.240 Flugbahnen, 2.414 Planungsentscheidungen, 1.504 Landungszweige und 2.048 PostLoad-Translationen; diese 10.206 Fälle belegen die jeweils angegebenen Funktionsgrenzen, keine vollständige Engine-Parität.

Die drei neuen Szenen bestehen Schaden, Sieg mit neun Treffern, Niederlage, Neustart und Taktvergleich. Zusätzliche Prüfungen decken alle vier Sprungphasen, natürliche Anforderungen nach Nahkampf, Rückkehr, exklusive Reservierungen, Treffer-/Tod-Unterbrechung, Laufzeitkollision und vollständiges Zurückrollen unvollständiger Weltabfragen ab.

Die Android-Abnahme erfolgt mit normalen Bedienelementen im API-35-x86_64-Emulator: Leicht für interaktiven Sieg/Niederlage mit GPU und CPU sowie Grafikvergleich/Kontextneubau; alle drei Stufen zusätzlich für passiven Schaden und exakte Niederlagentakte **1904 / 114 / 38**. Normal und Schwer bleiben eigene geprüfte Szenen. Die natürliche Sprungkette auf Leicht wird über eine nur lesende Statusschnittstelle taktgenau mit der Headless-Simulation verglichen. Eine physische ARM64-Abnahme dieser Lieferung steht noch aus.

Die zunächst gemessene Sprungplanung verursachte bis zu 452,90 ms JNI-Zeit. Entfernung wird nun vor teuren Flugprüfungen ausgeschlossen; die Vorabprüfung verwendet größere Segmente mit konservativer vertikaler Erweiterung, während die Bewegung weiter jeden festen Takt prüft. Im abschließenden Sprunglauf: **74,61 ms Maximum, 9,42 ms P95, kein verworfener Simulationszeitanteil** bei 631 Frames. Dies ist eine begrenzte Emulatormessung, keine Zusage für Gerätebildraten. Der frühere Befund bleibt unter `planning-before` ausdrücklich als Messung der ersetzten Binärdatei erhalten.

Nachweise: `analysis/milestone3l/validation-manifest.json`, Quellarchiv `source-snapshot.zip` und zugehörige native Exporte/JSON-Prüfbelege.

## Nächster Meilenstein

Die verbliebenen Kampfgrenzen gezielt schließen: originale Zielhilfe/Schussstreuung und Waffen-/Treffer-FX/Audio in den ausgewählten DC-17m-Pfad übernehmen. Die noch offenen Perch-/Goal-/Physikfälle bleiben separat inventarisiert. Vor weiterer Kartenabdeckung sind vollständiger Sieg-/Niederlageablauf und Geräteprüfung jeweils erneut erforderlich.
