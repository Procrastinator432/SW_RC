# HardwareShader-Uebergabe nach dem Originalaufruf

Drei Aufgaben verbinden die bisher einzeln geprueften Konstanten und Texturplaetze mit dem nativen HardwareShader-Aufruf: gemeinsamer Konstanten-Scratchpuffer mit getrennten Uploadhistorien, begrenzter Texturplatz-Durchlauf samt abschliessenden Zustandsworten und gemeinsame Snapshot-/Mesh-Gegenpruefung.

## 1. PS zuerst, gemeinsamer Scratchpuffer, getrennte Uploads

`D3DDrv!1000dbe0` ruft den Konstantendispatcher `1001c130` bei `1000dcbb` zuerst mit `shader+80c` / Zaehler `+940` (PS), danach bei `1000dcff` mit `shader+8c` / Zaehler `+93c` (VS). Beide verwenden dieselbe globale Adresse `10083e48`. Die einmalige Initialisierung reserviert 96 Ebenen zu je 16 Bytes (`1000dc77..8b`). Die Direct3D-8-Geraetemethoden an VTable-Offsets `16c` / `13c` erhalten jeweils Startregister 0, denselben Scratchzeiger und die jeweilige positive Anzahl.

`rc-package::hardware_constants::DeviceConstants` bildet diese Reihenfolge ab. `Counts` gehoert zum Material und beginnt mit PS=-3 / VS=-2; Zaehler 0 ueberspringt den jeweiligen Dispatch und Upload. Der Scratchpuffer bleibt materialuebergreifend erhalten. Nur der hochgeladene Praefix veraendert die getrennten Pixel-/Vertex-Geraeteregister. Unbenutzte Scratchregister und Geraeteregister ausserhalb dieses Praefixes haben deshalb unterschiedliche Historien. `Bank.count` in den Geraetebaenken bezeichnet den letzten Upload, nicht den aktuellen Materialzaehler.

Die Anfangswoerter aller drei Speicher werden ausdruecklich vom Host geliefert. Fuer diese Diagnose: null ausser Scratch- und Vertex-c17=.25. Native Konstruktorwerte bzw. vorherige echte GPU-Register werden nicht behauptet. Ein gemeinsam verwendetes DeviceConstants-Objekt bildet den globalen Scratchpuffer ab; Flicker bleibt separat global geteilt. Uploads modellieren erfolgreiche Geraeteschreibvorgaenge, ohne Direct3D aufzurufen. Fehler des sicheren Konstantenmodells erhalten bisherige Scratchwrites, Zaehler, Flicker und bereits erfolgte Uploads; der fehlerhafte Dispatch wird nicht hochgeladen. Das ist eine sichere Behandlung ausgeschlossener Hosteingaben, keine Behauptung nativer Fehlerbehandlung bei ungueltigen Speicherzugriffen.

Die bisherigen `rc-shader-snapshot-check` / `rc-snapshot-draw-check` bleiben reproduzierbare getrennte Bank-Fixtures mit VS-vor-PS. Sie belegen weiterhin ihre Einzelrechnungen, bilden jedoch nicht die nun rekonstruierte native Gesamtuebergabe ab. Fuer diese gilt ab jetzt `rc-hardware-draw-check`.

## 2. Texturplaetze und Zustandswoerter

`rc-package::hardware_stages` rekonstruiert `1000dd24..1000de98`. Acht Materialreferenzen beginnen bei `shader+8ac`; die Hostkapazitaet begrenzt die Schleife. Der erste Nullplatz oder eine nicht aufloesbare Referenz beendet sie sofort. Spaetere belegte Plaetze werden nicht nach vorn gepackt. Ressourcenaufloesung und deren Zustandsaenderungen bleiben ein Host-Callback; bestehende spaetere Buehnen werden nicht geloescht.

Nach erfolgreicher Aufloesung setzt die Routine den Texturindex in Wort 4 mit Maske `01ffff80`. Bei Skala +0 oder -0 wird Wort 2 mit `fffe2fff` / `2000` angepasst, ohne die Transformationswoerter anzufassen. Sonst gelten `ffff7fff` / `17000`, die Diagonale wird auf die rohe Skala gesetzt, die Nebendiagonale auf null und die beiden Versatzwoerter in umgekehrter Eingabereihenfolge geschrieben. Die Assembly prueft `UCOMISS`-Flags: NaN folgt dem Nichtnullpfad; sein Payload bleibt erhalten.

Die Proben liefern Transformations- und vorherige Zustandswoerter ausdruecklich. Filterung, Adressierung, Texture-Modifier, Ressourcenlebenszyklen und die spaetere Anwendung auf die GPU sind nicht implementiert. Im Meshbeispiel sind die fehlenden Plaetze Null, Skalen/Versaetze null und Resolver-Zustandswoerter null als Fixtures. Drei explizit gespeicherte Originalreferenzen werden gebunden: GreyHoloFade, GreyHoloFade, NoiseDigital. Wrapper-Overrides und Klassenvererbung werden nicht erfunden. Die CPU-Probe verwendet weiterhin ihren separaten Nearest/Repeat-Sampler.

## 3. Gemeinsame Originalmaterial-/Snapshot-/Mesh-Pruefung

`rc-hardware-draw-check` verarbeitet alle 256 bisherigen synthetischen x86-Szenensnapshots mit Originalprogramm und Originalkonstanten. 128 Szenen sind vollstaendig auswertbar; 128 erzeugen die bereits bekannten sicheren Fehler fuer Luecken in der nativen Lichttabelle. Auch bei diesen bleibt der zuvor erfolgte PS-Upload sichtbar; der VS-Geraetezustand stammt vom letzten erfolgreichen Upload. Jede Probe zeichnet Vorher-/Nachher-Scratch, Materialzaehler, Geraeteregister, Flicker, Inversionsanfragen und RNG-Verbrauch auf.

Die beiden erfolgreichen Snapshots 251/255 zeichnen je 3500 rekonstruierte CloneCommando-LOD0-Dreiecke ueber `HologramPass` mit den hochgeladenen Geraeteregistern. Es gibt keine nachtraeglichen Registerueberschreibungen. Mesh-Normalisierung/Faktor 1.5, portable RSQRT-Saat, c17=.25, 128x128 Ziel, halbe Pixelzentren, Alpha >0, LessEqual ohne Tiefenschreiben und SourceAlpha/Additive bleiben Diagnosevorgaben. Die beiden Bilder sind diagnostische Mesh-Ausschnitte.

`Record-HardwareDraw.py` prueft den neuen Original-Assembly-Aufruf, den PE-Import `d3d8.dll!Direct3DCreate8`, Originalmaterialreferenzen und Konstanten. Als numerisches Scratch-Orakel verwendet es den SHA256-geprueften vorherigen unabhaengigen Originalbefehls-Bericht: dieses Material ueberschreibt c0..3 zu Beginn jedes VS-Dispatches, wodurch die vorherigen PS-c0/c1 die VS-Rechnung nicht beeinflussen. Die separaten Uploadhistorien werden danach unabhaengig rekonstruiert. Die Vertex- und Rasterberechnung wird erneut separat ausgefuehrt.

Ergebnisse: **256 Uebergaben / 409600 Vorher-/Nachher-Woerter**, **768 Texturplatz-Proben / 172032 Zustandswoerter** mit allen 256 Belegungsmasken, Kapazitaeten 0..8, Resolver-Abbruechen und gebundenen Anzahlen 0..8; **21000 Vertex-Ausgaben**, **32768 Bildpixel**, Schreibvorgaenge 14736/14723. Debug-/Release-Berichte bytegleich. Sieben neue Tests, **596 Workspace-Tests**, Clippy mit `-D warnings`, Format-/Diffpruefung bestanden.

Artefakte: `analysis/decompiled/hardware-shader-{setup.c,setup.asm,setup-callers.txt,stage.c}`, `shader-dispatch-callers.txt`, `analysis/reports/hardware-draw{,-release,-validation}.json`, `hardware-draw-tests.log`, `hardware-mesh-0/1.png`. Die fruehere D3D9-Bezeichnung war ungenau: der Originaltreiber importiert Direct3D 8. Aeltere D3D9-Dokumentationslinks beschreiben nur Rastervergleiche.

```powershell
cargo run -p rc-inspect --bin rc-hardware-draw-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/shader-snapshots.input.json analysis/reports/skeletal-draw.bin analysis/reports/hardware-draw.json
cargo run --release -p rc-inspect --bin rc-hardware-draw-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/shader-snapshots.input.json analysis/reports/skeletal-draw.bin analysis/reports/hardware-draw-release.json
cargo test --workspace *> analysis/reports/hardware-draw-tests.log
python scripts/Record-HardwareDraw.py
```

Offen: originale Render-/Filter-/Adresszustaende, volle Textur-/Wrapperanbindung, Live-Szenenerzeugung, Ressourcen- und Geraeteverwaltung sowie die spielbare Engine. Allgemeine Diffuse-Abdeckung unveraendert 271/289. Android-Pruefung zum Schluss.

Weiterfuehrung 2026-10-09: [Material-/Samplerzustandsuebergabe](MATERIAL_STATES.md) rekonstruiert nun Capability-/Rueckfallpfade, rohe Pass-/Ressourcen-/Adress-/Filterwrites und deren Snapshot-Erfassung. Die spaetere GPU-Anwendung und die bisherigen CPU-Sampler-Fixtures bleiben offen bzw. explizit.

Weiterfuehrung/Korrektur 2026-10-09: [Direct3D-Zustandsuebergabe](D3D_STATE_HANDOFF.md) rekonstruiert nun Renderpraefix, nichtmatrizielle Buehnenuebersetzung und geordnete Cache-Aufrufplaene. Das bisherige rohe Label `filter` fuer Geraetewort +466c betrifft hier Adressierungswerte (IDs 13/14/25); echte Min-/Mag-/Mip-Filter bleiben offen. Rohwertberichte unveraendert, keine Grafik-API aufgerufen.
