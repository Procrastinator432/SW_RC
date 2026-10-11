# M3i: Originales Nahkampf-Budget

Die Lieferung 0.17.0 ersetzt die bisher fest eingestellten zwei Angriffe pro Nahkampf-Goal durch den ausgewählten Originaldefault `GeonosianWarriorBot.BurstCount = 1`. Das native Zeitbudget beträgt `float(BurstCount) * 2.0`, hier zwei Sekunden. M3 bleibt in Arbeit.

## Ausprobieren

APK: `analysis/milestone3i/republic-commando-0.17.0.apk`. Dazu eine neue Szenendatei verwenden:

- `analysis/milestone3i/geo_01a-normal.rcplay`
- `analysis/milestone3i/geo_01a-easy.rcplay`
- `analysis/milestone3i/geo_01a-hard.rcplay`

APK installieren, die gewünschte Datei auf das Gerät kopieren und über „Spielabschnitt geo_01a öffnen“ / „Szene öffnen“ auswählen. Die bisherigen Lieferungen bleiben erhalten. Die erste originale ForcePike-Schadenshöhe bleibt auf Normal 125; Schwer kann weiterhin mit dem ersten Treffer töten.

## Originalbelege und Umsetzung

`properties.GeonosianWarriorBotDefaults` liefert `BurstCount = 1`; die Zuordnung ist in `bot-goals-receipt.json` festgehalten. `UStateGeonosianWarriorAttack::UpdateSelf` liest bei `1003a2ba` das Bot-Feld `+0x40c`, konvertiert es mit CVTSI2SS und multipliziert mit der Konstante 2.0 bei `1004d91c`.

Die Feldzuordnung ist eine Rekonstruktion: `ACTBot::SetShotCounter` (`1000aac0`) schreibt `+0x408`; `ShotsLeft` (`100190b0`) liest dieselbe Adresse. Im exportierten `CTBot.uc` folgt das skalare Int-Feld `BurstCount` unmittelbar auf `ShotCounter`; beide Paketdefinitionen sind geprüft. Dies ist keine direkte native Property-Offset-Registrierung. Die Disassemblierung und der Ghidra-Export liegen neben der Lieferung.

`UGOAL_MeleeAttack` (`10007d90`, `100339a0`, `10033a70`) verwendet vorzeichenbehaftete Zähler und Float-Zeitgrenzen. Der Port prüft Erreichbarkeit strikt nach der Ein-Sekunden-Grenze, startet einen zulässigen Angriff vor der Ablaufprüfung und lässt dessen Animation enden, bevor das verbrauchte Goal abgeschlossen wird. Neue Szenen verwenden diesen Pfad; Szenen ohne Bot-Profil behalten den bisherigen Ablauf und ihre Replay-Kompatibilität.

## Prüfung und Grenzen

`validation-manifest.json` bindet APK, Szenen, Quellenarchiv und Prüfnachweise an SHA-256. Der Szenenprüfer kontrolliert alle drei Profile, Neustart und ein synthetisches Drei-Angriffe-Profil mit sechs Sekunden Budget. Bibliothekstests prüfen unter anderem strikte Zeit-/Facing-Grenzen, die Reihenfolge der Entscheidungen, Float-Rundung und negative native Zähler. Die Szenenprofile begrenzen die akzeptierten positiven Zähler auf 1 bis 32. Insgesamt bestehen 976 Bibliothekstests, Clippy sowie die alten Szenen und der Replay mit 1.002 Kommandos.

Ohne Gegenwehr tritt die Niederlage jetzt bei Tick 360 / 159 / 38 (Leicht / Normal / Schwer) ein; vorher waren es 397 / 274 / 38. Der veränderte Rhythmus entsteht durch die erneute Manöverwahl nach dem einzelnen Angriff. Die erste Trefferhöhe bei Tick 38 bleibt 31,25 / 125 / 218,75.

Die finale APK enthält ARM64 und x86_64; Signatur und 16-KiB-Ausrichtung sind geprüft. Alle drei Szenen bestehen die ABI-Prüfung. Im API-35-x86_64-Emulator bestehen der vollständige Normal-Gefechtsablauf über GPU und CPU, Kontextwiederherstellung mit identischen Pixeln und Posen, Backendwechsel und drei Wiederladungen. Alle drei Schwierigkeitsstufen wurden über den normalen Dateidialog bis zur Niederlage und zum Neustart geprüft; ihre Niederlagen-Ticks entsprechen exakt dem Rust-Spielkern.

Die Level-Zeit wird aus dem festen Simulationsschritt nach f32 konvertiert. Distanz, Ausrichtung, Erreichbarkeit, Animation und Goal-Scheduling bleiben vorbereitete Host-Eingaben; die native SSE-Distanzrechnung und RSQRT-Näherung werden nicht als bitgleich beansprucht. Allgemeine GoalQueue, defensive Sprünge, die Zufallsziehung nach dem Nahkampf und die originale Animationsauswahl fehlen weiterhin. Physische ARM64-Dauerabnahme bleibt offen.

Nächster Abschnitt: die verbleibenden Goal-/Zufallsabhängigkeiten am Originalpfad prüfen und die defensive Sprungauswahl als zusammenhängenden Ablauf rekonstruieren, bevor Zielhilfe und FX/Audio folgen.
