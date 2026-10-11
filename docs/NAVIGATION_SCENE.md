# Gegnernavigation und ausgewählter Goal-Host M3f

Stand 2026-10-11. Lieferung: Android **0.14.0 / Code 16**, `analysis/milestone3f/republic-commando-0.14.0.apk` und `analysis/milestone3f/geo_01a-navigation.rcplay`. Beide Dateien gehören zusammen. APK installieren, Szene über **SZENE ÖFFNEN** laden, drei Geonosianer ausschalten und die Exfil-Markierung erreichen. Die alten Lieferungen bleiben erhalten.

## Spielbarer Fortschritt

Geonosianer können auf geprüften Gehwegen Hindernisse umgehen und den Weg zum bewegten Spieler neu planen. Ohne unterstützten Weg warten sie und versuchen es nach zwei Sekunden erneut. Ein laufender Angriff verwendet weiterhin die Originalanimation und ihren MeleeHit-Notify; Tod und Trefferunterbrechung löschen Angriff und Weg. Der Neustart stellt den gesamten Ausgangszustand einschließlich Navigationszustand wieder her.

Die lokal begrenzte Karte enthält 37 bodengeprüfte Original-PathNodes und 68 gerichtete ReachSpec-Verbindungen. Nur tatsächlich in `PathList` referenzierte Verbindungen mit `reachFlags == 1` und ohne `bPruned`, `bJump`, `bPerch` oder `bFly` werden übernommen. Von 90 entsprechenden lokalen Kandidaten bestehen 68 die zusätzliche Körper-/Bodenprüfung. Nicht passende Verbindungen werden mit Namen im Receipt erfasst. Die PathNode-Position wird auf die vorbereitete Gegner-Körperhöhe gesetzt; die originale Markerhöhe ist keine Körperposition.

## Originalbelege und Hostgrenzen

`analysis/milestone3f/goals.c` und `melee-goal.asm` dokumentieren ctgame.dll `UGOAL_MeleeAttack::OnActivate` 100339a0, `PreUpdate` 10033a70 und die untersuchten Follow-/Queue-Funktionen. OnActivate setzt Follow-Minimum auf 0,75 × Waffenreichweite, Maximum auf Waffenreichweite und die nächste Erreichbarkeitsprüfung auf jetzt + 1 Sekunde. PreUpdate prüft diese Frist strikt mit `>`, die Waffenreichweite einschließlich Gleichheit und die Ausrichtung strikt mit `> 0,5`. Ein neuer Angriff reduziert die verbleibende Anzahl; die Ablaufprüfung folgt danach und wartet auf das Ende der Animation. Diese Reihenfolge ist im vorbereiteten Goal-Host umgesetzt und an Grenzfällen getestet. Es gibt keinen neuen vollständigen Originalinstruktions-Replay der Goal-Funktion.

Die bereits unabhängig geprüfte `geonosian_attack`-Entscheidung ist nun angebunden: CanPath kommt aus dem Gehweg-Host. Lunge, Jump und Grenade melden bis zur Bewegungs-/Waffenintegration weiterhin nicht verfügbar. LungeDist 800, JumpDist 3000 und OffensiveJumpProbability 0,95 werden erneut aus den lokalen Originalklassendefaults aufgelöst.

Die Wegsuche ist ein eigener begrenzter Dijkstra-Host, keine Rekonstruktion von APawn.FindPath/FollowLinks. Er verwendet originale gerichtete Verbindungen, Distance und ExtraCost; bei gleichen Kosten entscheidet die stabile Knotenreihenfolge. Höchstens acht nahe Knoten je Anschluss innerhalb 1500 Einheiten werden auf Erreichbarkeit geprüft. Direkte Strecken und jede Bewegungsstrecke müssen die Körperkollision und Bodenunterstützung bestehen. Bodentests erfolgen im Abstand höchstens 24 Einheiten. Nicht unterstützte Treppen, Lücken, Sprünge, Abstiege und bewegliche Türen bleiben gesperrt. Die Szene verwendet einen lokalen Radius von 4000 Einheiten und harte Datenlimits von 512 Knoten/4096 Kanten.

Bewegungstempo 65, Körperbox (22,22,60), sofortiges Drehen, zwei Angriffe pro Goal und die abwechselnde Auswahl der beiden stationären Nahkampfclips bleiben ausdrücklich Encounter-Adapter. Das originale Bot-Feld für die Angriffsanzahl ist noch nicht identifiziert. Entfernungen, Ausrichtung und Goal-Zeit verwenden die portable Hostrechnung; native f32-/RSQRT-Bitparität wird dafür nicht behauptet. Die einsekündige Neuplanung, die flache Bodenabtastung und das Pausieren der Bewegung während stationärer Angriffe ersetzen weitere native Follow-/Animationsbedingungen. Keine vollständige native GoalQueue-, Actor-Reservierungs-, Squad-, Trigger- oder Kampagnenparität.

## Nachweis und Wiederholung

`rc-play-navigation GAME_DATA BALLISTICS_SCENE OUTPUT_DIRECTORY` baut die Szene aus dem Originalpaket `Maps/geo_01a.ctm`, den Klassendefaults und der M3e-Szene. `navigation-receipt.json` enthält die vollständigen ausgewählten Knoten und Kanten, Originalobjektreferenzen, Kosten und Ausschlüsse.

`rc-play-navigation-check SCENE OUTPUT_DIRECTORY` findet ein wirkliches Hindernis in der Originalkarte: PathNode3 → PathNode191 ist direkt gesperrt, aber über das Netz erreichbar. Der Gegner läuft 1148,33 Einheiten und trifft in Tick 1091; jede simulierte Bewegung wird nochmals gegen Körperkollision und Boden geprüft. Der erste Originalnotify-Treffer ergibt Gesundheit 100/Schilde 37,5. Weitere Prüfungen decken Zielwechsel/Neuplanung, Tod, nicht unterstützte Ziele mit begrenzten Wiederholungen, exakten Reset, Niederlage, drei Projektilabschüsse und identische Zustände bei 120/30/10 Hostbildern ab.

Die Desktop-ABI-Prüfung und die Regressionen für den historischen 1002-Kommandos-Replay, M2-Animationen, M3-Kampf, M3d-Nahkampf und M3e-Ballistik bestehen. Android-Ergebnisse sowie Quellen-/Artefakthashes stehen im `validation-manifest.json`. Ein persönlicher ARM64-Gerätetest dieser Lieferung steht aus.

## Fortsetzung

Als Nächstes folgt die Übernahme der originalen Lunge-Bewegung einschließlich Machbarkeitsprüfung, Animation und Unterbrechung. Danach weitere native Goal-/Bot-Felder, GameRules/Schwierigkeitsgrad, AimError/Zielhilfe und weitere Kampfeffekte/-klänge. M3 bleibt offen; die vollständige Mission M4 folgt nach dessen Abnahme.

Abschließende Android-Prüfung: Beide vollständigen GPU-/CPU-Gefechtsläufe bestehen, einschließlich normalem Dateiauswahldialog, Touch-Zielen/Feuern/Bewegen/Sprung/Nachladen, Sieg/Niederlage, Pause, Neustart und Fehlerdatei mit Wiederherstellung. CPU/GPU-Startbilder unterscheiden sich im Mittel um 0,550/255 pro Kanal; 99,139 % der Kanäle liegen innerhalb zwei Stufen. Backendwechsel erhält den Zustand. Nach echtem Kontextneubau sind Bild und Pose exakt. Drei Wiederladungen bestehen die kurze Speichergrenzenprüfung; daraus folgt keine Langzeit-Leakfreiheits- oder physische ARM64-Leistungsbehauptung. 969 Bibliothekstests, Clippy, Format, Diff, APK-Signatur und 16-KiB-Ausrichtung beider ABIs bestehen.


2026-10-11: M3g integriert die originalen Move-Track-Kurven von Lunge/Lunge2, vollständige statische Kurvenprüfung, Bewegungs-Sweeps, MeleeHit und Abbruch/Absetzen. Neue APK0.15.0/Code17 plus geo_01a-lunge.rcplay unter analysis/milestone3g. Originale Root-Translation mit bestehender Szenenskalierung; kein vollständiger nativer PHYS_RootMotion-/GoalQueue-Port. Anleitung und Grenzen: docs/LUNGE_SCENE.md. Nächster Abschnitt: GameRules/Schwierigkeitsgrad und aktive Schadensmodifikation.
