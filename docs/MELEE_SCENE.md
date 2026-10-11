# Spielbare Nahkampfszene M3d

Stand: 2026-10-10. Android 0.12.0 (Versioncode 14), Szene `analysis/milestone3d/geo_01a-melee.rcplay`. Die Szene benötigt die neue APK; ältere APKs kennen das optionale Nahkampfprofil nicht.

## Verhalten

Geonosian-Krieger nähern sich und wechseln zwischen den Originalclips MeleeAttack und MeleeAttack2. Der Schaden entsteht beim originalen MeleeHit-Notify, nicht beim Beginn des Ausholens. Die Ausrichtung bleibt während des Angriffs fest: Zurückweichen kann den Treffer verhindern. Ein getöteter oder durch Beschuss unterbrochener Gegner führt den ausstehenden Treffer nicht aus.

Gesundheit und Schilde werden intern als f32 gespeichert; das HUD rundet verbleibende positive Werte auf. Die Originaldaten liefern 100 Gesundheit, 100 Schilde, eine Sekunde Regenerationsverzögerung und zehn Schildpunkte pro Sekunde. Die ForcePike hat 125 Basisschaden; die ausgewählte Spieler-DamageVariance ergibt 62,5 vor weiteren Spielregeln. Der erste Treffer lässt 37,5 Schilde. Vor dem zweiten Angriff können bereits Schilde regenerieren; deshalb ist dessen Gesundheitsrest nicht pauschal 75. DC17BlasterProj verwendet CTDamageEnergyDC17 und verursacht in dieser Begegnung 40 Körperschaden am Krieger.

## Herkunft und Grenzen

`rc-play-melee` baut die vier Originalclips einschließlich Lunge/Lunge2 aus GeonosianWarriorSet ein, prüft MeleeHit-Objektreferenzen und Zeitbits und bindet die ausgewählten originalen Schadenslisten samt Klassenhierarchien ein. `melee-receipt.json` enthält Werte und Herkunft. Die vorbereitete Pawn-Schadensrechnung benutzt die rekonstruierte native DamageVariance. Game.ReduceDamage bleibt eine explizite Identitätsfunktion; Schwierigkeitsgrad, Teamregeln, Bone-Multiplikatoren und vollständige Pawn-Callbacks sind noch nicht angeschlossen.

Direkte Annäherung (65/s), Sichtgrenze (1600), zwei Sekunden Angriffserholung und abwechselnde Clipwahl sind eigene Begegnungsregeln. Die vorbereitete native Manöverwahl ist noch nicht der vollständige AI-Goal-Host. Lunge/Sprung bleiben deaktiviert, bis Navigation und Bewegungsübernahme portiert sind. Kontaktprüfung nutzt die vorbereiteten Trefferkörper, Blickwinkel und statische Sichtlinie; sie behauptet keine vollständige originale MeleeTrace-Parität. Das Szenario enthält weiterhin drei vorbereitete Gegner, nicht die vollständige originale Missionslogik. Angriffs-/Trefferklänge und weitere Originaleffekte fehlen; die bisherige Schuss-, Nachlade- und Körperfall-Soundbank bleibt erhalten.

## Nachweise

`check/melee-validation.json`: wirklicher Szenen-/Projektil-/Notify-Pfad, fractional Schildüberlauf mit Regeneration, Ausweichen während des Ausholens, Abbruch durch Tod, Niederlage, drei Abschüsse, exakter Reset und gleicher Zustand bei 120/30/10 Hostaufrufen pro Sekunde. Die Ein-Gegner-Probe benutzt die öffentliche Begegnungs-API; die Todesunterbrechungsprobe setzt gezielt den Gegner-Lebensstatus. Sie ist keine Android-UI-Behauptung.

958 Workspace-Bibliothekstests bestehen. Die alte M3-Szene besteht weiterhin ihren Gefechtstest mit 15 Treffern, drei Abschüssen, Nachladen, Soundnotifies, Sieg/Niederlage und Reset. Android-Befunde stehen getrennt unter `android-gpu`, `android-cpu` und `graphics`, soweit die jeweilige Prüfung abgeschlossen ist. Der Emulator ersetzt keinen physischen ARM64-Test.

## Fortsetzung

M3 bleibt offen. Nächster zusammenhängender Schritt: ursprüngliche Laserbolzen-Bewegung und Schussursprung samt Trefferegeln rekonstruieren und in diese Szene integrieren. Danach AI-Goal/Navigation und Lunge-Bewegungsübernahme, Game.ReduceDamage samt Schwierigkeitsregeln und weitere Kampfklänge. M4-Missionsverkettung folgt nach der M3-Abnahme.

Abgeschlossene Android-Abnahme: beide vollständigen UI-Gefechtsläufe (GPU und CPU), Dateiöffnen/Fehlerbehandlung, Touch-Zielen/Feuern/Bewegen/Sprung/Nachladen, Sieg/Niederlage/Neustart und unveränderte Pause bestehen. GPU/CPU-Startbild: mittlerer Kanalfehler 0,550/255, 99,139 % innerhalb zwei Stufen. Ein tatsächlicher Kontextneubau stellt Bild und Pose exakt wieder her. Drei Wiederladungen bestehen die kurze Speichergrenzenprüfung. Signatur, ZIP-Ausrichtung und beide ABI-Builds sind geprüft. Dies ist keine physische ARM64-Leistungsabnahme.

Zum Ausprobieren: `republic-commando-0.12.0.apk` installieren und `geo_01a-melee.rcplay` über **SZENE ÖFFNEN** auswählen. Beide Dateien liegen unter `analysis/milestone3d`. Der Szeneninhalt ist vollständig enthalten; der bisherige Szenenpfad kann weiter separat aufbewahrt werden.


## M3e, 2026-10-11: Projektil- und Schussursprung-Integration

Android 0.13.0 und geo_01a-ballistics.rcplay verbinden vorbereitete native Vektorbewegung, originalen FPFireOffset, direkte Zielpunktkorrektur und die Näheprüfung vor dem Spawn mit der Nahkampfszene. Mathematik-/Kollisionsgrenzen und verbleibender AimError sind ausdrücklich dokumentiert. Aktueller Stand und nächste Schritte: [BALLISTICS_SCENE.md](BALLISTICS_SCENE.md).
