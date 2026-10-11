# Geonosian-Angriffsentscheidung – M3, zweiter Rekonstruktionsabschnitt

Stand: 2026-10-10. Die vorbereitete native Angriffsentscheidung ist in `rc-package::geonosian_attack` portiert und geprüft. Sie ist **noch nicht an die spielbare Szene angeschlossen**. APK 0.11.0 und `analysis/milestone3/geo_01a-combat.rcplay` bleiben die letzte spielbare Lieferung; M3 bleibt offen.

## Originalbefund

`rc-geonosian-assets` liest Klassendefaults, Properties-Overlays, Karteninstanzen und Animationen unmittelbar aus den lokalen Originalpaketen. Der Bericht enthält auch nicht auflösbare Eigenschaften als `Err`; diese werden nicht als Nullwerte interpretiert.

- `GeonosianWarrior.RequiredEquipment[0]` verweist auf `CTInventory.ForcePike`. `Pawn.AddDefaultInventory` erstellt diese Ausrüstung. `ForcePike` hat `MeleeRange=350`, `MeleeDamage=125` und `MeleeDamageType=Properties.CTDamageMeleeWarrior`. Dies sind Basiseigenschaften, keine Aussage über endgültigen Spielerschaden.
- geo_01a enthält sowohl `GeonosianWarrior1` mit ForcePike als auch `GeonosianWarriorNoWeap1` mit leerem Ausrüstungseintrag. Die vorbereitete Drei-Gegner-Begegnung ist weiterhin eine eigene Auswahl; ihre Gegner werden nicht mit diesen Karteninstanzen gleichgesetzt.
- `StateGeonosianWarriorAttack` verwendet `LungeDist=800`, `JumpDist=3000`, offensive Sprungwahrscheinlichkeit 0,95 und defensive Wahrscheinlichkeit 0,8 gegen Spieler bzw. 0,2 gegen andere Ziele.
- `ComputeEssentials` (ctgame.dll **100359e0**) berechnet dreidimensionale Entfernung mit der originalen RSQRTSS-Näherung und zieht den **Kollisionsradius des Ziels** ab. Der neue Baustein erwartet bereits diese Distanz. Eine gewöhnliche Mittelpunktdistanz ist kein gleichwertiger Eingang.

## Portierter Entscheidungsblock

`capabilities` übernimmt die Bedingungen aus **10039ff6..1003a19a**. Navigation bleibt ein expliziter Host: `CanPathTo`, bedingt `CanJumpTo`, bedingt `CanMoveTo` am gedrehten Animations-Endpunkt, anschließend `CanThrowGrenadeTo`. Die Reihenfolge bleibt erhalten, auch wenn die Granate die Auswahl später überschreibt.

`choose` übernimmt **1003a19a..1003a212**:

| Priorität | Bedingung | Auswahl |
|---|---|---|
| 1 | Granate möglich | Granate, kein Zufallswert |
| 2 | Sprung möglich und Zufallswert kleiner als offensive Wahrscheinlichkeit | Sprung |
| 3 | Ausfallschritt möglich und nächster Zufallswert größer als 0,8 | Ausfallschritt |
| 4 | Pfad möglich und Distanz strikt kleiner als JumpDist | Nahkampfziel |
| 5 | Pfad möglich | Verfolgen |
| 6 | keine der Bedingungen | Warten |

Sprungfähigkeit gilt nur strikt zwischen LungeDist und JumpDist; Ausfallschrittfähigkeit schließt Gleichheit bei LungeDist ein. Die MSVCRT-`rand()`-Ergebnisse 0..32767 werden mit dem originalen f32-Konstantenwort `0x38000100` skaliert. Der Baustein wählt keinen neuen Zufallsgenerator. Ein fehlgeschlagener Sprung kann einen weiteren Wert für den Ausfallschritt verbrauchen. `defensive_jump_after_melee` übernimmt den anschließenden Vergleich: Gleichheit reicht defensiv aus, offensiv nicht; NaN besteht keinen dieser Vergleiche.

Die Ganzzahlwerte der ausgewählten Manöver (0/8/0x200/0x400/0x800/0x1000) bleiben erhalten. Zielvalidierung, Zielwechsel, Physikprüfung, Goal-Allokation/-Queue, Taunt-/Warteausführung, defensive Navigation und Stimulusverarbeitung sind nicht Bestandteil dieses Ports.

## Treffer gehören zur Animation

`GeonosianWarriorSet.Fire` enthält keinen MeleeHit. Die eigentlichen Angriffsclips besitzen diese Notifies:

| Clip | Frames bei 30 Hz | normalisierte MeleeHit-Zeit |
|---|---|---|
| MeleeAttack | 48 | 0,3125 |
| MeleeAttack2 | 31 | 0,4003225863 |
| Lunge | 39 | 0,4871794879 |
| Lunge2 | 49 | 0,6530612111 |

Der Bericht bindet die Zeiten als originale f32-Bits an konkrete Notify-Exporte. Ribbon-, Sound- und Footstep-Ereignisse bleiben separat erfasst. `UAnimNotify_MeleeHit::Notify` (**engine.dll 10505b00**) ruft bei einem Pawn `APawn::DoMeleeAttack(BoneName,Offset)` auf, bei einer Waffe den entsprechenden Waffenpfad.

`Pawn.DoMeleeAttack` prüft sichtbar kollidierende Actors im Waffenradius, Höhenabstand bis zum 1,5-Fachen der größeren Kollisionshöhe und einen Richtungs-Dotwert strikt größer als 0,8; Accessory-Actors sind ausgeschlossen. Anschließend läuft der reguläre TakeDamage-Pfad. Die Auswahl des Nahkampfmanövers bei Distanz unter 3000 ist deshalb **kein Treffer mit 3000 Reichweite**: `GOAL_MeleeAttack` verfolgt zunächst das Ziel und startet den Angriff erst mit eigenen Reichweiten-/Ausrichtungsbedingungen.

## Prüfung und Grenzen

- `Record-GeonosianAttack.py` interpretiert ausschließlich die exportierten Originalinstruktionen **1003a19a..1003a20c exklusiv**. Capability-Masken, Wahrscheinlichkeit und `rand()`-Resultate werden vorgegeben. Original-DLL und Assembly sind durch SHA256 gebunden; die Konstanten werden zusätzlich aus der PE-Datei gelesen.
- `rc-geonosian-check` vergleicht **9.408 Fälle** mit Rust: alle 32 Capability-Kombinationen, Zufallsgrenzen, Wahrscheinlichkeiten einschließlich NaN/Unendlich sowie die Zahl verbrauchter Zufallswerte. Alle bestehen. Dies ist ein begrenzter Instruktions-Replay, keine Ausführung des vollständigen Spiels oder der DLL.
- Fünf neue Unit-Tests prüfen Distanzgrenzen, Navigation-Aufrufreihenfolge, Priorität, Zufallsverbrauch und offensive/defensive Gleichheit. Alle **898 rc-package-Tests**, gezieltes Clippy und Formatprüfung bestehen.
- Kein Android-Test in diesem Abschnitt: Die Android-Laufzeit und das Szenenformat wurden nicht geändert.

## Nächste Integration

Vor der spielbaren Umstellung sind die `GOAL_MeleeAttack.PreUpdate`-Bedingungen, Animationsauswahl und Original-Trefferzeit zusammen anzuschließen. Dazu fehlen im Szenenpaket noch die vier Angriffsclips und deren ausführbare MeleeHit-Ereignisse. Direkte Verfolgung kann vorerst ein gekennzeichneter Adapter bleiben; Sprung-/Ausfallschrittbedingungen dürfen erst mit Navigation und Root Motion aktiviert werden.

Der Schaden braucht außerdem `CalculateDamageFrom`, `Game.ReduceDamage`, Schild-/Armor- und Schwierigkeitsregeln. Der exportierte Decompilertext von `execCalculateDamageFrom` enthält fehlerhaft rekonstruierte Stackparameter; vor einer Portierung sind dessen Instruktionen und VM-Argumentgrenzen abzugleichen. 125 Basisschaden unverändert vom Spieler-HUD abzuziehen wäre daher unbelegt. Danach neue Szene/APK getrennt ablegen und den normalen Android-Kampflauf am Ende der Integration prüfen.

Nachweise: `analysis/milestone3b/geonosian-assets.json`, `attack-oracle.json`, `attack-validation.json`, `validation-manifest.json`; native Exporte `analysis/decompiled/geonosian-attack.c`, `.asm`, `geonosian-melee-goal.c`, `geonosian-essentials.c`, `geonosian-melee-notify.c`.


M3-Fortsetzung 2026-10-10: Die native DamageVariance-Argumentbelegung ist anhand der Instruktionen geklärt. Varianz und vorbereitete Pawn-Schadensrechnung sind portiert; 4.038 Originalfälle und 958 Workspace-Bibliothekstests bestehen. Originaldaten zeigen 100 CloneCommando-Schilde, 62,5 ForcePike-Schaden vor weiteren Spielregeln und unterschiedliche DC-17-Schadenstypfaktoren. Noch keine Anbindung an die spielbare Szene oder neue APK. Nächste Integration: f32-Gesundheit/Schilde, Varianz/GameRules-Grenze und Melee-Goal/Clip/Notify zusammenführen. [Schadenspfad und Nachweisgrenzen](DAMAGE_PATH.md).


## M3d: spielbare Nahkampfintegration

Die neue Szene und APK 0.12.0 verbinden Original-Angriffsclips/Hit-Notifies mit fractional Gesundheit, Schildregeneration und ausgewählten DamageVariance-Listen. Bisherige vorbereitete Bausteine sind teilweise integriert; Navigation, native Goal-Ausführung, Lunge-Bewegung und GameRules bleiben offen. Aktuelle Nachweise, Lieferdateien und Abgrenzungen: [MELEE_SCENE.md](MELEE_SCENE.md).
