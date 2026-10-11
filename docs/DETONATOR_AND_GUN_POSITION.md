# Thermaldetonator und Geschützstellung: geprüfte Grundlage

Stand: 11. Oktober 2026. Die nächste Erweiterung nach 0.21.0 ist in Arbeit. Wurfberechnung, ausgewählte Granatenskriptregeln und der ursprüngliche Geschützablauf sind implementiert und separat geprüft. **Noch keine neue spielbare APK/Szene:** Der letzte spielbare Stand bleibt `analysis/mission3-playable`. Die neuen Grundlagen sind unter `analysis/mission4-detonator` dokumentiert.

## Originalregeln

`AProjectile::execGetTossVelocity` (engine.dll, 104ed730) übernimmt nicht die gesamte Spielergeschwindigkeit. Die SSE-Berechnung 104ed7e0–104ed86c ergibt aus vorbereiteter Blickrichtung `D`, Spielergeschwindigkeit `V`, Speed und TossZ:

```text
s = Speed + 0,4 * ((D.y * V.y + D.x * V.x) + V.z * D.z)
Wurfgeschwindigkeit = [D.x * s, D.y * s, TossZ + s * D.z]
```

Die Konstante bei10673858 besitzt die Bits3ecccccd. 2052 vorbereitete Maschinenbefehls-Replays stimmen bitgenau mit `projectile_motion::toss_velocity` überein. Parameter aus der Unreal-VM und `FRotator::Vector` sind dabei bewusst vorbereitete Grenzen. Das beweist diese Rechnung, keine vollständige native Flugphysik.

Die Originaldefaults von `ThermalDetonatorProj` ergeben Speed2200, MaxSpeed3000, TossZ200, FuseTime12s, PostImpactFuseTime1s, LifeSpan8s, Welt-Abpralldämpfung0.3, Actor-Dämpfung0.1, RestSpeed20, Damage400 und DamageRadius600. Ein Aufprall verkürzt den verbleibenden Zünder; bei normalem initialem Zünder verlängern weitere Kontakte ihn nicht. Unter RestSpeed werden Bouncing und Actor-Kollision abgeschaltet; die Fallphysik bleibt Aufgabe des Hosts. Der Bounce-Sound verwendet Level.TimeSeconds und eine strikt größere Pause als MinBounceSoundTime, nicht die Lebenszeit der Granate. Ein nicht getroffener Actor läuft vor seinem12s-Zünder über die8s-LifeSpan ab und wird ohne `Explode` zerstört. Exakte Gleichzeitigkeit von Zünder/LifeSpan verwendet im begrenzten Host explizit LifeSpan zuerst; das ist keine nachgewiesene native Tickreihenfolge.

`detonator.rs` implementiert die ausgewählten Zünder-, Reflexions-, Soundabstands- und Lebensdauerregeln sowie die Schadensrechnung aus `Actor.HurtRadius`. Die Auswahl sichtbarer kollidierender Actors, Sichtlinien, Ausschluss des direkten Treffers, individuelle Schadensfilter und Impulse gehören weiterhin zum integrierenden Host. Ebenso fehlen allgemeine Materialeffekte, Wasser, Zufallsspin, LastTouchedActor-Verwaltung und die Verkürzung bei externem Schaden.

## Geschützabschluss durch Granatenkontakt

Der Originalbunker ist kein gewöhnliches400-Schaden-Ziel. Trigger621 verwendet `TT_ClassProximity` und `CTInventory.GrenadeProj`, liegt bei `[3965.687012,-6619.771484,-1964.815308]` und hat Radius100/Höhe40. Er reagiert auf eine passende Granate im Kontaktbereich; Spieler- und Pistolenkontakte lösen ihn nicht aus. Eine Explosion außerhalb des Bereichs ersetzt diesen Kontakt nicht.

`MachineGunBunkerDestroyed` startet parallele Skripte:

- Script754 schaltet sofort `UnblockBunkerPath`, setzt den Tracker auf `TrackerTarget09` und schließt Originalziel3 ab. Nach2s folgt CA1G1716 (1,361179s), danach `BunkerProjectileEFX`, weitere2s später `MissionObjectives` und `Conversation04`. Die dazwischen gespeicherten Zeilen3011/3012/3013 sind original deaktiviert und werden nicht abgespielt.
- Script325 wartet1s und startet dann die geskriptete Bunkerzerstörung: Explosion, Warrior-Spawnereignis, Stoppen der Projektil-FX-Fabrik,50.000Schaden an `SmallBunker` mit49.000HP, Zerstören von `SmallBunkerHatch` und Abschalten von `BunkerCollision`. Seine nachfolgenden Trooper-/Warrior-/Droid-Inszenierungen werden vom neuen begrenzten Host noch nicht ausgeführt.
- Script696 liefert den ursprünglichen Wurfhinweis und Erinnerungen. Die fünf zugehörigen Originalsamples sind extrahiert; die Erinnerungsschleife ist noch nicht an den Spielkern angeschlossen.

`gunner.rs` trennt Zielabschluss, verzögerte Geometrieeffekte und Bestätigung. Die kontinuierliche Punkt-/Zylinderprobe erhält vom Aufrufer ein bereits an blockierender Weltgeometrie abgeschnittenes Segment. Sie besitzt selbst keine Sicht auf die Welt und darf nicht als Nachweis einer möglichen Wurfroute verstanden werden. Der eigenständige Prüfer verwendet ausdrücklich synthetische Kontakte am Originalzylinder.

## Darstellung, Audio und Abnahme

`Weapons.CloneWeapons.ThermalDetonator` ist als `detonator.rcscene` vorbereitet:352Dreiecke,352texturierte Flächen,0Lücken. Originalmaterial `WeaponTextures.CloneWeapons.TDetonator` mit UV0 und deckendem Shader (`Opacity=None`, `OB_Normal`), Snapshot-Rundlauf und Vorder-/Rückansicht sind geprüft. Diese Ansichten zeigen das Asset; noch keinen geworfenen Actor in Android.

Acht PCM-Samples sind vorbereitet: fünf Advisor-Wurfhinweise, AbschlusszeileCA1G1716, originaler Bounce-Sound und Leervorratssound. Ihre Längen stammen aus den tatsächlichen WAV-Daten. Explosionsaudio über `ThermalDetEffects` und vollständige Explosionseffekte bleiben zu integrieren.

1013 Bibliothekstests bestehen, darunter14 neue Tests für Projektile und Geschützablauf. Zusätzlich prüfen `rc-detonator-check` die echten extrahierten Defaults/Kontakte und `rc-detonator-toss-check` die2052 nativen Replays. Formatierung und Clippy ohne Warnungen werden vor dem Archivieren geprüft. Die Nachweise der bisherigen0.21.0-Lieferung bleiben unverändert.

## Nächste Integration

1. `AActor::physFalling` (10490a30, vorbereitet unter `analysis/decompiled/detonator-falling.c`) für den Granatenfalladapter auswerten. Der bisherige Pawn-Fallsolver ist kein Ersatz für diesen Actor-Pfad.
2. Inventarverbrauch und den ursprünglichen AnimNotify-Release an Wurfsteuerung binden; dabei Schuss/Nachladen während `GrenadeThrow` sperren.
3. Physische Flugbahn, Welt-/Actor-Kontakte, tatsächliche sichtbare Schadensziele und Bunkerzustände integrieren. Texturierte Granate, Bounce-/Leervorrats-/Explosionsaudio sowie Hinweise anbinden.
4. Den bisherigen Abschnitt nach der Aufnahme fortsetzen, erst nach dem neuen Abschluss beenden und den vollständigen Lauf mit Touch auf GPU/CPU prüfen. Anschließend neue zusammenpassende APK und Szene liefern.

Werkzeuge: `rc-detonator-assets`, `rc-detonator-visual-check`, `scripts/Record-DetonatorToss.py`, `rc-detonator-toss-check`, `rc-detonator-check` und `scripts/Summarize-DetonatorFoundation.py`.
