# Schadenspfad – M3, dritter Rekonstruktionsabschnitt

Stand: 2026-10-10. Native Schadensvarianz und vorbereitete Pawn-Schadensrechnung sind in Rust umgesetzt und geprüft. **Noch keine Anbindung an die spielbare Szene:** APK 0.11.0 behält ihre bisherigen Kampfregeln. Die Android-Prüfung folgt am Ende der zusammenhängenden Integration.

## Geklärter nativer Pfad

Der Ghidra-C-Text von `AActor::execCalculateDamageFrom` hatte VM-Argumente und Stackvariablen falsch zusammengeführt. `analysis/decompiled/damage-variance.asm` klärt die Belegung nach dem Argumentlesen: Schadenstyp bei ESP+0xc, optionale Varianz bei +0x10, Schaden bei +0x14. Nach PUSH EBX verschieben sich die Zugriffe um vier Bytes.

`rc-package::damage_variance` portiert den vorbereiteten Block **engine.dll 104f83f3..104f8562**:

1. Eine explizite Varianz hat Vorrang vor `Actor.MyDamageVariance`. Ein unbrauchbares explizites Defaultobjekt führt zu Faktor 1, ohne Rückgriff auf die Actor-Varianz.
2. Ein null Schadenstyp überspringt die Varianz. Ein gültiger Schadenstyp wird gegen seine Klassen und Basisklassen geprüft.
3. Der erste passende Include-Eintrag bestimmt den Faktor. Erst ohne Include-Treffer wird die Ausschlussliste geprüft; dort bedeutet ein Treffer Faktor 0.
4. Eine null Klasse **in einer Liste** ist ein Wildcard. Sie ist nicht mit einem unaufgelösten Verweis gleichzusetzen. Ohne Treffer gilt Faktor 1.
5. Die abschließende Multiplikation verwendet f32 ohne Clamp.

VM-Auswertung, Klassen-/Defaultobjekterzeugung und Konstruktion der dynamischen Defaultarrays bleiben vorbereitete Hostgrenzen. Der Port erfindet für nicht aufgelöste Referenzen keinen Nullwert.

## Originaldaten und konkrete Unterschiede

`rc-damage-assets` liest die scalaren Defaults samt Overlaynachweisen und die vollständigen Include-/Exclude-Payloads der DamageVariance-Klassen. Positive Objektverweise in den dekodierten Arrays werden mit ihrem Paket qualifiziert. Die ursprünglichen Tags und Offsets bleiben daneben erhalten. Der neue begrenzte Object-Array-Parser prüft Anzahl, Verweise, abgeschnittene Daten und nachgestellte Bytes. Eine allgemeine Compound-Default-Mischung wurde nicht eingeführt.

CloneCommando-Defaults: **100 Gesundheit, 100 Schildpunkte**, ArmorFactor 0, `MyDamageVariance=DV_OrganicCTSquadDamage`, keine separate ShieldDamageVariance. GeonosianWarrior hat 100 Gesundheit, keine Schilde und `DV_GeonoshansWarriorDamage`.

Die folgenden Beispiele führen die ausgewählten serialisierten Listen und Klassenvererbungen durch den nativen Replay. Sie gelten **vor** Rüstung, Schildverteilung und Game.ReduceDamage; sie behaupten keine vollständig ausgeführte Defaultobjektkonstruktion.

| Zielliste | Schadenstyp | Basis | Ergebnis der Varianz |
|---|---|---:|---:|
| GeonosianWarrior | CTDamageMeleeWarrior | 125 | 93,75 |
| GeonosianWarrior | CTDamageEnergyDC17 | 20 | 40 |
| GeonosianWarrior | CTDamageEnergyDC17Commando | 20 | 10 |
| OrganicCTSquad | CTDamageMeleeWarrior | 125 | 62,5 |
| OrganicCTSquad | CTDamageEnergyDC17 | 20 | 6 |
| OrganicCTSquad | CTDamageEnergyDC17Commando | 20 | 6 |

Der derzeit ausgewählte `DC17BlasterProj`-Default verweist auf **CTDamageEnergyDC17**. Die abweichende Commando-Klasse darf nicht allein wegen des menschlichen Spielers eingesetzt werden. Änderungen beim Spawn bzw. durch Spielregeln müssen gesondert belegt werden. Die bisherigen 20 Körper-Schadenpunkte des Spielabschnitts sind weiterhin ein Basisschadenadapter; er führt diese Varianz noch nicht aus.

## Vorbereitete Pawn-Rechnung

`rc-package::pawn_damage` setzt die arithmetische Teilstrecke aus `Pawn.TakeDamage` und `SetShields` um, mit ausdrücklicher **Source-basierter** Grenze: noch kein Bytecode-/Ganzfunktionsabgleich.

- Authority zuerst; erster nichtleerer passender Knochenfaktor, dann `1-ArmorFactor`, dann DamageVariance, anschließend ein vom Host bereitgestellter `Game.ReduceDamage`-Aufruf.
- Eine ShieldDamageVariance wird anhand des Schildstands **vor dem Treffer** ausgewählt. Die Auswahl erfolgt auch bei einem Schadenstyp, der Schilde später umgehen würde.
- Bei nichtpositivem reduziertem Schaden bleiben Schild-/Gesundheitswerte unverändert. Sonst absorbieren Schilde den Treffer; nur der negative Schildrest wird Gesundheitsschaden. Exakte Schilderschöpfung verursacht noch keinen Gesundheitsschaden.
- Nicht umgehende Treffer verschieben den Wiederaufladetermin nach hinten, auch bei vorherigem Schildstand null. Umgehende Treffer lassen Schildstand und Termin unverändert.
- Gesundheit und Schilde bleiben f32 und werden nicht vorzeitig in Ganzzahlen gewandelt. Zwei hypothetische Treffer zu je 62,5 gegen 100 Schilde lassen zunächst 37,5 Schilde und anschließend 75 Gesundheit übrig, wenn Game.ReduceDamage unverändert durchreicht.

Diese Rechnung erwartet endliche vorbereitete Eingaben. Treffer-Metadaten, Materialwechsel, Sound, Status-/Physikwechsel, Stagger, Todesablauf und Rückgabesonderfälle bereits toter Pawns bleiben außerhalb. Die Rückgabe bezeichnet den rechnerisch abgezogenen Gesundheitsbetrag. Spielregeln, Schwierigkeit, God Mode, Neutralzonen und weitere GameRules werden nicht durch einen angenommenen Faktor ersetzt.

## Nachweis

`Record-DamageVariance.py` interpretiert die exportierten Originalinstruktionen **104f83f3..104f8560 exklusiv**. Zwei UClass-Aufrufe liefern vorbereitete Defaultobjekt-Snapshots. DLL, Assembly und Paketbericht werden gehasht.

`rc-damage-check` besteht **4.038 Fälle**: Include-Reihenfolge, Vererbung, Ausschlüsse, Wildcards, fehlende/ungültige explizite Varianz, null Schadenstyp, negative Werte, Null, Unendlich und NaN sowie sechs Beispiele aus den Originaldaten. Endliche Ergebnisse müssen bitgenau übereinstimmen; NaN-Payload und -Vorzeichen werden nicht gleichgesetzt. Dies ist kein Live-Spiel-/DLL- oder Android-Test.

Elf zusätzliche Unit-Tests prüfen Varianz, Arraygrenzen und Pawn-Reihenfolge/Schildüberlauf. Alle **958 Workspace-Bibliothekstests**, gezieltes Clippy und Formatprüfung bestehen. Berichte und Quellenbindung: `analysis/milestone3c/validation-manifest.json`.

## Verbleibende Integration

Für die spielbare Nahkampfversion sind nun zusammenzuführen: f32-Kampfzustand mit Schilden, Anbindung der belegten Varianzlisten, Game.ReduceDamage-Grenze, GOAL_MeleeAttack-Ausführung sowie die vier Angriffsclips mit MeleeHit-Notifies. Der bisherige Ganzzahl-HUD-Zustand braucht dabei eine definierte Anzeigeumwandlung. Erst danach neue Szene/APK liefern und den normalen Android-Kampflauf prüfen. Die vorbereitete Angriffsentscheidung bleibt in [GEONOSIAN_ATTACK.md](GEONOSIAN_ATTACK.md) dokumentiert; M3 ist weiterhin offen.


## M3d: spielbare Nahkampfintegration

Die neue Szene und APK 0.12.0 verbinden Original-Angriffsclips/Hit-Notifies mit fractional Gesundheit, Schildregeneration und ausgewählten DamageVariance-Listen. Bisherige vorbereitete Bausteine sind teilweise integriert; Navigation, native Goal-Ausführung, Lunge-Bewegung und GameRules bleiben offen. Aktuelle Nachweise, Lieferdateien und Abgrenzungen: [MELEE_SCENE.md](MELEE_SCENE.md).
