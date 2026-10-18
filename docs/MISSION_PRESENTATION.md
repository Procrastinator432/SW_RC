# Bacta- und Missionsinszenierung: aktueller Arbeitsstand

Fortsetzung dieses Standes: [Stationskollision, freie Barrierenphysik und Android-Abnahme 0.51.0](MISSION_PHYSICS_ANDROID.md). Die nachfolgenden Abschlussbelege beschreiben den vorherigen Zwischenstand.

Stand: 18.10.2026. Die sichtbaren Bacta-Emitter und ausgewählte originale Explosionen/Raucheffekte sind an die laufende Mission angeschlossen. Die bisher ausgelassene Verwundungs-Teleportaktion und die Bewegung der Ausgangstür sind ergänzt. Dies ist eine technische Fortsetzung der vier beauftragten Punkte; die genaue Stationskollision und die vollständige native Karma-Physik bleiben ausdrücklich offen. Eine neue Geräte-/Hörfreigabe wird daraus nicht abgeleitet.

## Die vier Punkte

| Punkt | Umsetzung und verbleibende Grenze |
| --- | --- |
| Bacta-Darstellung | Alle 14 ursprünglichen SpriteEmitter-Templates, ihre Materialtexturen, Bereiche, Größen-/Farbkurven, Opazität, Ausrichtung und Atlasanimation importiert. Eye-Anker aus dem animierten Originalknochen; vorhandener Energieloop bleibt angeschlossen. |
| Körper/FadeOut und Position | EndState entfernt Eye sofort und beendet das Nachspawnen der Body-Emitter; lebende Körperpartikel laufen aus. RNG, Partikel, Verzögerungen und Bewegungsbereichszeit werden gespeichert. Stationsausrichtung und konservative Kollisionsgrenze geprüft; die letzten ungefähr 10 Welt-Einheiten zum originalen Sollstandpunkt bleiben durch den Box-/AABB-Adapter blockiert. |
| Intro → Verletzung → Bacta → Barrieren | Zehn originale Factory-/Prop-Effekte mit Spawn-/Ambientquellen angebunden. Originalaktion 2395 setzt Factory 823 vor dem Auslösen zur Spielerposition und übernimmt die Pawn-Ausrichtung. Prop 617 besitzt keinen Ersatzmesh/-material und wird mit seinem ursprünglichen Übergangseffekt entfernt. Vier Karma-Barrieren verwenden weiterhin den vorhandenen Restpose-Adapter; ihr vollständiges physikalisches Umfallen ist nicht rekonstruiert. |
| Scorch, Tür und Kartenwechsel | Normaler Eingabelauf mit Spieler-Kameraansichten bis zur geöffneten Tür und tatsächlichem A→B-Wechsel. Tür 34/35 verwendet DelayTime, MoveTimes[0], KeyPos[1] und die originale Glide-Kurve; Kollisionshüllen und Bild werden gemeinsam verschoben. Speicherproben prüfen auch die halb geöffnete Tür. Android-Bild-/Hörabnahme folgt als eigener Schritt. |

## Originalbelege und Laufzeit

Die Bacta-Klassen stammen aus `cteffects.u`; die geerbten Felder von `ParticleEmitter` und `SpriteEmitter` aus `engine.u` werden im ausgewählten Importpfad feldweise mit den Instanzwerten zusammengeführt. Die generische Defaultauflösung bleibt unverändert streng. Der Loader weist andere Emitterklassen und nicht unterstützte Orts-/Atlasvarianten ab.

Gelesene Originalfunktionen: `UParticleEmitter::SpawnParticle`10478140, `SpawnParticles`10476ad0 und `UpdateParticles`10476d70; `USpriteEmitter::FillVertexBuffer`1051bf30; D3D-ParticleMaterial10006020; `AMover::physMovingBrush`104edfb0. Die rekonstruierte `ACTION_TeleportToPoint.InitActionFor`-Editorquelle enthält sowohl `SetLocation(Dest.Location, bNoCheck)` als auch `SetRotation(Dest.Rotation)`. Die originale Kartenaktion2395 nennt `PlayerExplosionFactory` und `PlayerCommando` als Ziele. Der bisherige feste Kartenort der Factory war deshalb falsch: Er lag nach dem Abseilen außerhalb der Spieleransicht. Der gespeicherte Effekttransform folgt dem Spieler nach dem Auslösen nicht weiter.

Der ausgewählte Partikelhost verwendet 60 feste Ticks pro Sekunde und einen gespeicherten lokalen CRT-Zufallsstrom. Er berücksichtigt verzögerte und anteilige Geburt, Lebensdauer/Respawn, Startgröße/-farbe/-spin, Beschleunigung, bewegte Spawnbox, Geschwindigkeitsgrenzen und anschließende zufällige Geschwindigkeitsdämpfung. Relative Partikel bewegen sich mit dem Anker; unabhängige behalten ihren Geburtsort. Die Body-Positionen stammen aus dem originalen CloneCommando-Skelett mit ausgewähltem ActionBreathe-Poseadapter. Das ist keine vollständige Rekonstruktion der Spieleranimation oder des globalen Engine-Zufallsstroms.

Der Materialpfad unterscheidet die nativen Blendfaktoren:

| Ursprünglicher DrawStyle | Portmodus | RGB |
| --- | --- | --- |
|1 AlphaBlend|Alpha|S × Alpha + D × (1 − Alpha)|
|3 Translucent|Additive|S + D|
|5 Darken|Darken|D × (1 − S)|
|6 Brighten|Screen|S + D × (1 − S)|

D3D-Faktor4 bedeutet **INVSRCCOLOR**. Die falsche Auslegung als SRCCOLOR würde dunkle, ausblendende Partikel zu schwarzen Rechtecken machen. CPU und GLES verwenden deshalb dieselbe inverse Quellfarbe. ZTest und AlphaTest/AlphaRef sind emitterbezogen; Effekte schreiben keine Tiefe. Atlaszellen werden in originaler V-schneller Reihenfolge gewählt und bei BlendBetweenSubdivisions vor dem Compositing interpoliert.

Die RCFRA003-Blendwort-Erweiterung verwendet den Modus im unteren Byte, Bit8 für ausgeschalteten Tiefentest, Bit9 für Alpha-Test, Bits16–23 als AlphaRef und Bit10 für Atlasblending. Bits24–27/28–31 speichern U-/V-Rastergröße minus eins. In Atlasbatches enthält der vierte Farbfloat das Mischgewicht; diese Form ist auf RGB-Blendmodi mit Farb-alpha1 begrenzt. Alte001/002-Frames und003 ohne diese Flags bleiben lesbar.

Die Türkurve ist bei Glide `3*t*t − 2*t*t*t`, sonst linear. Der Öffnungston startet wie in Mover.Open erst nach DelayTime; der gespeicherte Türzeitgeber verhindert ein erneutes Abspielen nach Restore. Die vorhandene Türfreigabe bleibt zeitlich der Missionsfluss; der neue Host ist auf die zwei translationsbewegten Ausgangstüren begrenzt. Allgemeine Moverrotationen, Encroachment/Pushing und vollständige Karma-Simulation sind nicht enthalten.

## Bau und Prüfungen

`map-a-presentation` ist eine deklarierte zusätzliche Szenenstufe. Sie bezieht sämtliche Originaldaten über GAME_DATA und den übergebenen Szeneninput. Sie fügt keine impliziten historischen analysis-Abhängigkeiten hinzu. Materialpixel werden verlustfrei als little-endian-u32/Base64 gespeichert; alte JSON-Wortarrays bleiben lesbar. Auflösung, RGB und Alpha bleiben dabei unverändert. Das native 112-MiB-Payloadlimit wurde nicht angehoben.

Der zentrale Missionsbau führt nach den bestehenden 25 Checkpointproben zusätzlich `rc-presentation-check` aus. Dieser benutzt ausschließlich aufgezeichnete reguläre Eingaben und die Laufzeitkamera. Er speichert Ansichten und native GPU-Pakete bei Verwundung, Bacta, Sprengungen und Türbewegung. Zustand, Frame und 30 weitere Eingaben werden nach Restore an fünf ausgewählten Punkten verglichen. Der A-Lauf umfasst18.829 Eingaben; der folgende Kartenstart entsteht über `travel_to` und läuft weitere 60 Ticks.

Reproduzierbarer Gesamtbefehl:

```powershell
.\build.ps1 -Target All -SceneInputs local-assets/mission1/scene-inputs.json `
  -GameData 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' `
  -OutputDirectory build/mission-corrections/complete
```

Zusätzliche native Stations-/Soundprüfung:

```powershell
python scripts/Test-BactaLifecycle.py build/mission-corrections/complete/scenes/map-a-presentation/geo_01a.rcplay `
  build/mission-corrections/complete/scenes/map-a-bacta-replay/map_a_replay.bin `
  --output build/mission-corrections/lifecycle-final.json --capture-views
python scripts/Test-OpeningSoundBindings.py build/mission-corrections/complete/artifacts/mission1-scenes.rcpack.zip `
  --output build/mission-corrections/sounds-final.json --ticks 1800
.\scripts/Test-GpuFrame.ps1 -FramesDirectory build/mission-corrections/complete/scenes/player-presentation `
  -OutputDirectory build/mission-corrections/gpu-frame-java
```

Der Android-Build benötigt die lokale Verbindung zum Gradle-Daemon. Unter einer Sandbox können dafür eine freigegebene lokale Ausführung und ein beschreibbarer TEMP-/TMP-Ordner nötig sein; die Abhängigkeiten bleiben offline.

Die konkreten Abschlussberichte liegen unter `build/mission-corrections`; `complete/validation.json` ist der zentrale Gesamtbericht. Die letzte veröffentlichte 0.50.0 bleibt eingefroren. Arbeits-APK und Szenenpaket bilden noch keine neue Release-/Gerätefreigabe.

## Verbleibende Abnahme

Die exakte Zylinder-/Pawn-Kollision an der Station, vollständige native Karma-Barrierenbewegung, HandSignal-Waffenwechsel und vollständige Spieleranimation sind noch offen. Die neuen Bilder sind CPU-Referenzansichten; echte GLES-Darstellung, Touch/Nahansicht und die Hörprüfung am kleinen Androidgerät müssen mit dem zusammengehörigen APK/Paket durchgeführt werden. Vorher ist der komplette Korrekturblock nicht als gerätefertig abgeschlossen zu bewerten.

Für den späteren Handytest müssen das zusammengehörige Arbeits-APK und Szenenpaket neu installiert/importiert und eine neue Mission gestartet werden. Bestehende Spielstände behalten ihre archivierten Szenen und zeigen dadurch nicht automatisch die neuen Effekte.

## Geprüfter Abschluss dieses Arbeitsstands

Der zentrale Offline-Gesamtbau unter `build/mission-corrections/complete` besteht: je **1.150 Debug-/Release-Bibliothekstests**, Materialimporttests, Doc-Tests, Clippy aller Targets, Java-/Buildvertrag, ARM64/x86_64 und APK, 22 Szenenstufen sowie 25 aktuelle Checkpointproben A/B/C→Fabrik. 34 gewöhnliche Spieleransichten und ihre nativen Framepakete sind erzeugt; der tatsächliche Android-Parser liest alle 34 sowie das echte GPU-Weltpaket unter 96 MiB JVM-Heap. Das Weltpaket umfasst 43.475.920 Bytes und bleibt im bestehenden 64-MiB-JNI-Limit. Fünf zusätzliche Zustands-/Bild-/Fortlaufproben umfassen Verwundung, aktive Bacta, Body-Tail, Türstart vor Ablauf der Verzögerung und bewegte Tür. Der Öffnungston wird genau einmal nach DelayTime ausgelöst.

Die native Stationsprüfung besteht mit fünf Animationsphasen, sieben gespeicherten Partikelzeitpunkten, Abbruch/Wiederbenutzung, Eingabesperre, sechs Originalnotifies und atomarer Abweisung beschädigter Partikel-/Factoryzustände. Bei Tick 2970 verschwinden Eye-Partikel sofort, 54 Body-Partikel bleiben; bei Tick 3100 bleiben vier, bei 3400/3500 keine. Die Soundbank bleibt mit **7.610.358 Bytes** im bestehenden JNI-Limit. Am endgültigen ZIP sind 41 Cue-Referenzen/29 eindeutige Originalcues, beide Droidentöne, Rampenalpha und vier historische Snapshot-Restores geprüft.

Zusammenfassung: `build/mission-corrections/verification-summary.json`; Ursprungshashes: `original/source-inputs.json`; Stationsprüfung: `lifecycle-final.json`; Sound-/Paketprüfung: `sounds-final.json` und `package-final.json`. Spieleransichten einschließlich Übersicht: `complete/scenes/player-presentation/review-sheet.png`.

Arbeits-ZIP SHA-256: `9195e5370617580dbbb80f67a9450d712e41e018ff43446a65a881642f524293`.
A-Szene SHA-256: `4dabe5942e685542f9a009a879283a408de0cc18187d10a91852134fbfbe9e64`.
Das ist ein interner geprüfter Stand; die oben genannten Kollisions-/Karma- und Geräteprüfungen bleiben offen.
