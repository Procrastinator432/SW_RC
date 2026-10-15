# Artillerie und Warrior-Angriff – 0.27.0

Die ausgewählten Originalziele 0–6 auf geo_01a enthalten jetzt den Artillerieabschnitt mit Gunship, Spider-Droid und drei Geonosian Warriors. Die gesamte erste Originalmission ist damit noch nicht vollständig rekonstruiert.

## Ausprobieren

APK: `analysis/mission8-artillery/republic-commando-0.27.0.apk`.
Szene: `analysis/mission8-artillery/geo_01a-artillery.rcplay` (RCPLAY10, 92.212.438 Bytes).
Beide Dateien auf das Android-Gerät kopieren, APK installieren und die Szene über den Dateidialog öffnen. Diese Szene benötigt die neue App-Version.

Vom Bacta-Einstieg den bisherigen Zielen folgen: Absturzstelle, Granaten, Geschütz, Scorch und Sprengauftrag. Nach Öffnung des Bunkers der Wegführung zum Turm folgen. Bei der Beschusswarnung zurücktreten und den Einschlagsbereich meiden. Die Warriors lassen sich mit der Waffe bekämpfen. Der Spider-Droid wird im ursprünglichen Gunship-Ablauf zerstört; er ist kein vom Spieler zu tötender Boss. Anschließend den Turmeingang erreichen. Neustart setzt Gegner, Beschuss, Kollision und Aufträge zurück.

## Rekonstruktion

- Gunship580 und SpiderDroid31 verwenden originale texturierte Skelettmeshes, ausgewählte cGEO01-Animationssätze, Root-Bewegung und Notify-Zeitpunkte. Vier Gunship-Sounds und sechs Effektaufrufe einschließlich zweier Strahlen sind integriert.
- Vier Projektile aus Originalquellen1013–1016 verwenden TranMissile-Mesh, Originaltextur und Ribbon-Textur. Fabrikkapazität4, Intervall0,30000001192s, Geschwindigkeit8000, Lebensdauer15s und Explosionswerte400/512 sind gebunden. Der begrenzte Host detoniert bei einem Weltkontakt und kann Spieler und Warriors verletzen.
- Tutorial3552–3555 führt über die originale Warnung CA1G1711 zum Warrior-Angriff. Fabrik533 erzeugt insgesamt drei Gegner, höchstens einen lebenden zugleich, an1018/1028/1029. Originalmodelle, Bewegungsanimationen und Geschwindigkeit450 sind integriert; jeder Tod meldet AttackWarriorDead.
- Die Szenenbank enthält64 Samples. Alle früheren Audio-Rollen bleiben erhalten.

## Abnahme

Der normale Eingabelauf erreicht nach16.648 Simulationsschritten den ursprünglichen Ausgang mit20HP und acht Abschüssen. Er umfasst vier Einschläge, drei Warrior-Todesmeldungen, einmalige Spider-Zerstörung, originale Ziele0–6 und exakten Neustart. Ein zweiter normaler Eingabelauf bleibt im Beschussbereich und endet nach13.899 Schritten mit0HP; der Einschlag verursacht139 Schadenspunkte am Spieler.

Rust-Spielkern, Windows-ABI und Android-x86_64-ABI prüfen Sieg und Beschuss-Niederlage. Die bisherige RCPLAY09-Szene bleibt mit14.746 Eingaben erfolgreich. Android-Dateidialog, GPU, Audio, Touch-Schießen/Benutzen/Ducken und Pause werden separat geprüft. Der Java-Szenenleser besteht bekannte und unbekannte Dateilänge mit112MiB Heap, Fehlerfällen und temporärer Dateibereinigung. 1.064 Bibliothekstests, Clippy, Format und Quelldiff sind im Liefermanifest gebunden.

`validation-manifest.json` bindet APK, Szene, Originalpakete, Quellen-Snapshot und Prüfergebnisse per SHA-256. Screenshots und Zustände liegen im Unterordner `playthrough`; die Animationsgrenzen sind separat dokumentiert.

## Offene Originalparität

Der Warrior-Host nutzt bodengebundene direkte Verfolgung, Sichtprüfung und vorläufigen Nahkampf mit5 Schaden/1,4s; vollständige CTBot-Flug-, Patrouillen-, Squad- und Schadensregeln fehlen. Zufallsintervalle verwenden feste Mittelpunkte, Spawnpunkte eine feste Reihenfolge. Projektile nutzen einen begrenzten geraden Flug und Weltkontaktadapter; allgemeine ProjectileFactory-/Bounce-/Fuse-Parität bleibt offen.

Strahlen und Ribbons verwenden originale Texturen und Notify-Anheftungen, aber begrenzte Geometrie. Weitere Explosionsaufrufe verwenden den vorhandenen Sprite-Adapter. Vollständige Partikel-, Licht-, Shader- und Gunship-Einleitungsinszenierung fehlen. REP_Gunship_GroundDust, SpawnDemoDroids und SpawnExitArea haben keinen gefundenen Empfänger in der untersuchten Karte. Die vorherigen Türposen wechseln weiterhin ohne kontinuierliche Mover-Interpolation.

Die Reiseanforderung GEO_01B.ctm#playerstart bleibt erhalten; GEO_01B wird noch nicht geladen. Vollständiger Touch-Durchlauf und physische ARM64-Abnahme dieser Version bleiben offen.

## Nächster Meilenstein

Zuerst die verbliebenen geo_01a-Unterschiede anhand der Originalereignisse inventarisieren und den Warrior-/Squad-Kampf sowie die ausgesparte Einleitung priorisieren. Danach Kartenwechsel mit Zustandsübernahme und GEO_01B-Einstieg integrieren. Erst nach einem zusammenhängenden Originalablauf einschließlich dieser Übergänge die gesamte erste Mission abnehmen.
