# M3k: vorbereitete Sprungflugbahn

Fortsetzung: **M3l / Android 0.18.0** integriert diese vorbereiteten Komponenten in die spielbare Szene. [Aktuelle Lieferung und Ausführungsgrenzen](DEFENSIVE_JUMP_SCENE.md). Der nachfolgende Stand dokumentiert die ursprüngliche Analyse.

Stand 2026-10-11. `rc-package::jump_trajectory` übernimmt die skalare Rechnung von **AActor::CalcTrajectoryVelocity, engine.dll 103bbfa0..103bc5cc**. Spielbar bleibt Android **0.17.0 / M3i**. Der Baustein wird noch nicht vom Szenen-Goal-Host aufgerufen; M3 bleibt offen.

## Übernommene Rechnung

- Ziel minus Start, horizontale Länge und Mindestflugzeit **0,5 s**. Die horizontale Null-Länge führt trotzdem durch die Kehrwurzel-Schätzung; die anschließende native Maske ergibt +0.
- Geschwindigkeit aus Kehrwert der Zeit, Zielhöhe und `Gravity.Z * -0.5`; einzelne f32-Operationen in Originalreihenfolge.
- Anfangs negative Vertikalgeschwindigkeit verlängert die Zeit auch bei erlaubten flachen Bögen. Abbruch erst bei **vz > 0**, spätestens nach vier Verlängerungen um **1,5**.
- Sonst lässt `AllowFlatArcs` die Anfangslösung bestehen. Ohne diese Freigabe beginnt die Verlängerung bei Richtungs-Dot **> 0,95** und endet bei **< 0,95**. Die Summenreihenfolge unterscheidet sich beim Anfangsvergleich und im Verlängerungszweig.
- Wiederholte Normalisierung des Zielvektors und sämtliche Seed-Aufrufe bleiben erhalten, einschließlich der Normalisierung im abwärts gerichteten Zweig.

Der öffentliche `calculate`-Pfad erhält die RSQRTSS-Schätzung als Callback. `portable` verwendet `1 / sqrt(s)` als Adapter. Die Vergleiche belegen dieselbe Rechnung **bei denselben Schätzungen**; sie belegen keine identischen Hardware-RSQRTSS-Ergebnisse. Vorgesehene vorbereitete Eingaben sind endliche Positionen/Gravitation und positive horizontale Geschwindigkeit ohne Float-Überlauf. Flugzeit und Verlängerungszahl sind zusätzliche Prüfausgaben; das Original liefert nur den Geschwindigkeitsvektor zurück.

## Nachweise

`Record-JumpTrajectory.py` interpretiert die exportierten Originalinstruktionen mit dokumentierter Seed-Grenze. 4.240 Fälle kombinieren Mindestzeitgrenzen, horizontale Null-Länge, flache Bögen, abwärts gerichtete Ziele, positive/negative/Null-Gravitation und deterministische Zufallspositionen. Zusätzlich werden Seeds um ±1 ULP variiert. `rc-jump-trajectory-check` vergleicht Geschwindigkeitsbits, Zeitbits, Verlängerungszahl sowie Eingaben/Anzahl aller Seed-Aufrufe. Alle fünf Verlängerungszahlen sind vertreten.

983 Workspace-Bibliothekstests, Clippy, Format und Whitespace-Prüfung bestehen. Die defensive Planung aus M3j, alle drei M3i-Szenen einschließlich Niederlage/Reset/Bildratenvergleich und der historische 1002-Kommandos-Replay bestehen weiterhin. Native Exporte, Recording, Prüfbeleg, Quellenarchiv und Hashbindung: `analysis/milestone3k/validation-manifest.json`. Keine neue APK, keine neue Android-Abnahme für diesen vorbereiteten Baustein.

## Geklärte Start-/Landeabhängigkeiten

`AActor::GetAnimRootTranslation` (103ba690) gibt bei fehlendem Mesh Null zurück und delegiert ansonsten an ein SkeletalMesh. `USkeletalMesh::GetAnimRootTranslation` (10501350) sucht in den eingebundenen Animationen die **erste passende Sequenz** und liest deren gespeicherten Vektor bei **+0x34/+0x38/+0x3c**. Ohne Treffer ergibt sich Null. Das ist kein Sampling der komprimierten Root-Kurve. Die Herkunft/Berechnung dieses Laufzeitfelds muss noch über Laden/PostLoad aufgeklärt werden; der aktuelle Archivparser stellt es nicht als belegten Sequenzwert bereit. Die vorhandene Lunge-Kurve darf diesen Wert nicht stillschweigend ersetzen. Die Auflösung eines passenden globalen FName ist ebenfalls eine Host-Grenze.

`JumpTo` verwendet den durch Startanimation/Rotation verschobenen Startpunkt für die Flugbahn. `GOAL_ChangePhysics` erhält **0x15 als ChangeFlags**, nicht als Physikmodus 21. Die eigentliche Flug-/Kollisionssimulation, Startanimation, dynamische Reservierung und Goal-Ausführung fehlen noch. Für die Landung unterscheidet das Original normalen Boden und Perch. Bei gewöhnlichen Zielen gilt Erfolg erst nach gestartetem Sprung, PhysicsWalking und Abstand² **< Toleranz²**; **>** führt dann zum Fehler. Exakte Gleichheit bleibt aktiv. Diese Landeregel ist hier untersucht, noch nicht implementiert/verifiziert.

## Nächster zusammenhängender Integrationsschritt

1. Gespeicherte Sequenztranslation über den originalen Lade-/PostLoad-Pfad rekonstruieren und Startanimationsnamen sowie Geschwindigkeitsfeld belegen.
2. ChangePhysics-Flags, Start-/Flug-/Landestatus und Abbruch/Tod/Reservierungsfreigabe als ausführbaren begrenzten Goal-Host übernehmen und gegen native Fälle prüfen.
3. Kollisionsgeprüfte defensive Sprünge samt post-Melee-Zufallszweig in geo_01a integrieren. Dann passende Szenen/APK liefern und Android am Ende prüfen.
