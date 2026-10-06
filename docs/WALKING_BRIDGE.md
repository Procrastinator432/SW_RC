# SpeedFactor und Walking-Probe mit Originalgeometrie

Der rekonstruierte Walking-Rechenteil ist als eigener Diagnosepfad mit Welt-/Mesh-Kollision und PhysicsVolume-Auswahl verbunden. Die bestehende Controller-Probe bleibt als Regression erhalten. Aktiviert wird die zusätzliche Probe mit `rc-mesh-probe <GameData> <map.ctm> <report.json> --walking-diagnostic`.

## Native Geschwindigkeitsfaktoren

Die unklaren Register im C-Export von APawn::SpeedFactor1048cd00 wurden anhand der vollständigen Assemblerausgabe geklärt. `analysis/decompiled/pawn-speed-factor.asm` enthält auch die ersten Zweige, die im bisherigen C-Export fehlen. Der reproduzierbare Ghidra-Exporter `scripts/ghidra/ExportInstructions.java` liest Instruktionen aus der bestehenden Datenbank.

Der Actorzeiger wird von ECX nach ESI übernommen; das Richtungsargument liegt nach dem Prolog bei ESP+0x5c. Der gespeicherte Gesamtfaktor liegt bei ESP+4. Relevante Schritte:

- Pawn+0x284 Bit4: Hocken übernimmt CrouchSpeedRatio bei+0x54c. Dieser Zweig überspringt Geh- und Gesundheitsmultiplikation.
- Sonst: Pawn+0x288 Bit0x100000 und HealthLevel bei+0x2ac kleiner2 aktivieren WoundedSpeedRatio+0x550. Das entspricht dem gesundheitsabhängigen Zweig für HL_Red/HL_Orange; der Aufrufer liefert die kombinierte Bedingung.
- Pawn+0x284 Bit1 multipliziert mit WalkSpeedRatio+0x548, falls nicht gehockt wird.
- IsHumanControlled10480780 und ein nicht-null Zeiger bei Pawn+0x2f0 übernehmen den Wert des referenzierten Objekts+0x490, andernfalls1. Derselbe Zugriff ist im bereits exportierten `GetWeaponSpeedModifier`1034c070 zu sehen; Weapon.uc deklariert PlayerSpeedModifier.
- Die Richtung wird über die Matrix aus dem virtuellen Aufruf bei VTable+0xe8 in lokale Koordinaten gebracht und normiert. Negative lokale X erhält BackSpeedRatio+0x554; lokale Y erhält SideSpeedRatio+0x558. Die Länge der gewichteten Richtung wird mit dem Gesamtfaktor multipliziert.

`SpeedFactors::local_factor` setzt diese Rechnung auf einer bereits lokalen Richtung um. Für den Diagnosepfad gibt es eine inverse Yaw-Drehung mit quantisiertem Unreal-Winkel. Pitch/Roll und native Matrix-/SSE-f32-Parität sind damit nicht bestätigt.

Beim Walking-Brückenschritt begrenzt SpeedFactor zunächst die Beschleunigung. Nach Richtungskorrektur, Reibung und Beschleunigung wird SpeedFactor erneut aus der tatsächlich entstandenen Geschwindigkeit berechnet; der End-Cap ist das Minimum aus faktorisiertem GroundSpeed und vorgegebenem MaximumDesiredSpeed. Die zusätzliche native Cap-Skalierung bei Pawn+0x288 Bit0x4000000 mit Pawn+0x568 ist nicht umgesetzt. ModifyVelocity und die automatische SetWalking-Logik aus physWalking sind ebenfalls noch nicht integriert.

Die Bodenunterstützung wird geometrisch geprüft, unabhängig vom gespeicherten grounded-Flag. Nur bei bestätigtem Boden und nichtpositiver Z-Geschwindigkeit wirkt Walking. Ohne Unterstützung ist ausschließlich passives Fallen über den vorhandenen eigenen Körperlöser erlaubt; Beschleunigung in der Luft erzeugt ausdrücklich einen Fehler. Es gibt keine neue Sprung-/Falling-Steuerung. Fehler liefern keinen neuen Zustandskandidaten.

## Prüfung vom 2026-10-06

80 Workspace-Tests, Clippy über alle Targets und Formatprüfung bestehen. Die zwei neuen Tests prüfen Richtungsfaktoren, Zustandspriorität, Waffenmultiplikation, Nullrichtung, Yaw, ungültige Werte sowie die Brücke mit tatsächlicher Bodenunterstützung, MaximumDesiredSpeed-Cap, passivem Fallen, abgewiesener Luftbeschleunigung und unverändertem Eingabezustand bei Geometriefehlern.

Die zusätzliche Originalkartenprobe beginnt bei geo_01a.PlayerStart0 und umfasst300Schritte bei1/60Sekunde:120Fall-/Ruhephase,30Diagonalbeschleunigung,60Ruhephase,30Vorwärtsbewegung im Gehmodus,60Ruhephase. Davon237Walking-Schritte und63passiveFall-Schritte. Keine Import-, Auswahl- oder Bewegungsfehler; alle Volume-Auswahlen vollständig.

| Ergebnis | Wert |
|---|---:|
| maximales diagonales Tempo | 438,89420445787545 |
| maximales Tempo im Gehmodus | 225,00000000000003 |
| Endgeschwindigkeit | 0 |
| Endzustand | am Boden |
| Höhendrift in den letzten60Frames | 0 |

Die ausgewählten Volumes wechseln bei Tick25 von DefaultPhysicsVolume zu PhysicsVolume1 und bei Tick33 zurück (nullbasierte Indizes). Beide haben hier GravityZ-1100; die ausgewählte Bodenreibung beträgt8. Der neue Pfad verwendet die Originalprofilwerte und protokolliert Auswahl, Beschleunigung, Optionen und Ergebnis je Frame. CrouchSpeedRatio0,5 und WoundedSpeedRatio~0,833 werden zusätzlich aus Engine.Pawn-Klassendeltas aufgelöst; die Kartenprobe bleibt gesund und nicht gehockt.

Die bisherige300-Schritte-Controller-Probe wird im selben Lauf erneut geprüft. Ihr Endzustand bleibt identisch zum früheren geo_01a-Volume-Bericht: Position[2488,833120497962;1758,6333345835715;-2603,49951171875], Geschwindigkeit0, grounded=true.

Berichte: `analysis/collision/geo_01a-walking-diagnostic.json`, Zusammenfassung `walking-bridge-validation.json` im selben Ordner und `analysis/evidence.json`.

## Diagnoseannahmen und verbleibende Arbeit

MaximumDesiredSpeed450 und Waffenfaktor1 sind explizite Vorgaben des Aufrufers; aktive Waffe, Gesundheit und vollständige Pawn-Runtimeflags werden nicht geladen. Hocken-/Verwundungsfaktoren sind rechnerisch getestet, aber die Probe simuliert weder Körperverkleinerung noch Gesundheit. Die Eingabe wird nach eigener Regel normiert und in Beschleunigung übersetzt. Collision-/Schritt-/Gravity-Integration bleibt der eigene statische Körperlöser. Es handelt sich um eine Verbindung rekonstruierter Rechenteile mit Originalgeometrie, nicht um einen Vergleich mit ausgeführtem Original-Gameplay.

Native Walking/Falling-Parität, weitere Cap-Flags, ModifyVelocity, automatischer Gehmodus, bewegte Actors/Volumes, Wasser, JNI und die Verbindung zur Android-Steuerung bleiben offen. Android-Prüfung auf Nutzerwunsch zum Schluss; APK und Emulator wurden in diesem Arbeitsschritt nicht angefasst.
