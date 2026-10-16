# Bactastation: Animation und Heilungsfreigabe

Stand: 16.10.2026, weiterer Teil des zweiten Handytest-Korrekturblocks. **Keine neue Gerätefreigabe; der gesamte Inszenierungsblock bleibt offen.**

Fortsetzung: Der originale `UsingItem`-Controller ist inzwischen als optionaler Szenenbaustein integriert. Blick und Bewegung werden beim Benutzen zielbezogen geführt; Loslassen gibt die Steuerung zurück. Die frühere Nähe-/Blick-Abbruchbeschreibung unten gilt für den vorherigen Stand ohne Controllerbindung. Die konservative Kollisionsgrenze bleibt ausdrücklich offen. [Originalregeln, aktualisierte Eingaben und Nachweise](BACTA_USING_ITEM.md).

Die deklarierte Stufe `map-a-bacta` ersetzt die 1.896 bisher eingebrannten Stationsflächen durch eine animierte Instanz des originalen `BactaDispenserGEO` (Actor736). Der Builder reproduziert die alten Weltkoordinaten und entfernt ausschließlich exakt passende Flächen; fehlende oder doppelte Zuordnungen brechen den Bau ab. Die bisherige Bodenanpassung und Box-Kollision bleiben als Portadapter erhalten. Die gemeinsame Originalmaterial-Auflösung gilt auch für den neuen Skeletal-Import.

`ctgame.BactaDispenser.Active` startet `Load`, wartet auf `FinishAnim` und setzt erst dann `bActivated`. Dieser Ablauf steuert jetzt die Heilungsfreigabe. `Load` und `Holster` haben jeweils30 Frames bei30Hz; die vorhandenen portierten Channel-Playback-/LOD-Tick-Funktionen führen sie aus. Loslassen, Wegdrehen oder Entfernen aus dem Benutzungsbereich beendet die Aktivierung und startet `Holster`. Nach einem Abbruch lässt sich die Station erneut öffnen. Die ausgewählte Tutorialstation wird nach erfülltem Heilungsziel nicht erneut benutzt; allgemeines Wiederaufladen ist weiterhin außerhalb dieses begrenzten Hosts.

Alle sechs originalen `Engine.AnimNotify_Sound` bleiben mit ihren ursprünglichen Zeitbits erhalten. Sie lösen die drei Originalaufnahmen `mach_GEN_Bdroid_Dispense_open`, `mach_GEN_Bdroid_Dispense_close` und `fol_GEN_trapFiddle_01` aus. Die Bank wird außen erweitert und ergänzt die bestehenden Soundidentitäten; Intro-/Dialogrollen und Droidenton bleiben erhalten. Die mechanischen WAV-Dateien enthalten teilweise den gültigen18-Byte-PCM-Formatblock mit `cbSize=0`. Der Prüfer akzeptiert diese Form ohne Konvertierung oder Änderung der Samples; unbekannte Erweiterungen und ADPCM bleiben abgewiesen.

Der Animationskanal, Stationsphase, Aktivierungszeit und Notify-Verlauf gehören zum persistenten Checkpoint. Zustände mit fehlender Station, ungültigen Kanalwerten oder zukünftigen Notifies werden atomar abgewiesen. Die bestehende Snapshot-ABI und historische Szenen ohne Bacta-Erweiterung bleiben unterstützt. PCM-Abspielpositionen werden wie bisher nicht gespeichert.

## Prüfung

```powershell
python scripts/Test-BactaLifecycle.py build/bacta-flow/scene/geo_01a.rcplay local-assets/mission1/assets/map_a_replay.bin --output build/bacta-flow/lifecycle.json --capture-views
```

Der Prüfer arbeitet über die echte native Schnittstelle und regulär aufgezeichnete Spielereingaben. Er prüft vollständigen A-Abschluss, alle sechs Notifies, keine Heilung während `Load`, Pause/Loslassen/Blick-/Bereichsabbruch, erneutes Benutzen sowie Zustand/Bild und90 Folgeschritte nach dem Laden jeder Stationsphase. Beschädigte Stationszustände dürfen den laufenden Zustand nicht verändern. Optionale Bilder zeigen die tatsächliche Spielerperspektive.

Im unveränderten18.730-Tick-A-Replay beginnt `Load` bei2731, die Station wird bei2789 aktiv, erreicht100 Gesundheit bei2970 und beendet `Holster` bei3028. Der Eingabelauf benötigt keine zusätzliche Wartezeit oder veränderte Bewegung. Dieser native Nachweis ersetzt keine physische Bild-/Hörabnahme.

Arbeitsnachweise unter `build/bacta-flow`: `checks/validation.json` (je1.131 Debug-/Release-Bibliothekstests, Material-/Java-/Clippy-/Buildvertrag), vollständiger20-Stufen-Neubau mit25 A/B/C-Checkpointproben bis zur Fabrik, `lifecycle-final.json`, `package-sound.json` (30 Referenzen/26 Originalcues) und `package-legacy.json` (beide Droidentöne, Rampenmaterial und vier historische Checkpoint-Restores). Das interne ZIP hat SHA-256 `45905ffbc3ffc1511de955ed71e7d9811f13e4e54ba492d7c0688e8355f81d6a`; es benötigt den aktualisierten nativen Loader.

## Noch offen

Die Originaleffekte `CTEffects.BactaZap_Dispenser` am Knochen `eye` und `CTEffects.BactaZap_Dispenser_Body` am Spieler sind noch nicht angebunden. Damit fehlt auch der an die Effektlebensdauer gebundene Original-Dauerton `GEO_Sound.GEO_03.ele_GEO_genEnrgy_lp_01`. Zusammengesetzte Emitterdefaults müssen explizit aufgelöst werden; es wird kein generischer Heilungs-Piepton eingesetzt. Erst zusammen mit dieser Effekt-/Loop-Lebensdauer ist der Bacta-Inszenierungsabschnitt vollständig.

Die aufgezeichnete Nahansicht ist noch keine bestandene Bildabnahme: Der Spieler gleitet im vorhandenen Replay von ungefähr146 auf90 horizontale Einheiten zum Actor weiter, während dessen `Load` den Ausleger von ungefähr57 auf101 lokale X-Einheiten ausfährt. Die Rootmatrix bleibt dabei identisch; eine frei erfundene Root-Verschiebung würde diesen Befund verdecken. `rc-bacta-assets` hält Posen und Originaldefaults fest: `bLockPlayerLocation=true`, `PlayerOffset=90`, `PlayerRange=256`. Die genaue native Spieler-Ausrichtung/Positionssperre muss vor einer visuellen Freigabe verfolgt werden; der derzeitige Blick-/Abstandsadapter bildet sie noch nicht ab. Die normale Nahansicht kann dadurch von geöffneten Teilen stark verdeckt werden. Kamerazustände liegen neben den Bildern unter `player-views`.

Anschließend bleiben die originale Verwundungskette, sichtbare Zerstörung/Bewegung der einzelnen Barrieren und die normale Spielerabnahme von Scorchs Bunker, Tür und A→B-Übergang. Die letzte veröffentlichte Lieferung bleibt0.50.0; eine neue passende APK und ein Kampagnenpaket folgen nach dem gesamten Korrekturblock.
