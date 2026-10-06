# Original-BSP-Raumprüfung, Stand 0.7

Der Android-Viewer kann an seiner Kameraposition den Inside/Outside-Zustand des originalen Weltmodells und eine Linie ohne Ausdehnung prüfen. Die Taste `BSP prüfen` funktioniert nach dem direkten Import einer `.ctm`-Karte. Sie verändert die freie Kamera nicht und verhindert keine Bewegung durch Wände. StaticMesh-Kollision, Pawn-Ausdehnung, Schwerkraft und Gameplay fehlen weiterhin.

## Gelesene Originaldaten

Der neue `rc-package::collision`-Reader folgt dem vorhandenen `UModel::Serialize`-Pseudocode nach dem Geometriepräfix:

1. Zwei u32-Zonenfelder; anhand des zweiten bis zu 64 Zonenrecords, jeweils Objektreferenz und 20 weitere Bytes.
2. Polys-Objektreferenz.
3. Compact-count-Array von Bounds (je 25 Byte).
4. Compact-count-Array von Hull-Indizes (je vier Byte).
5. Leaf-Records: zwei Compact-Indizes und acht weitere Bytes; bei Versionen vor 156 ein zusätzlicher Compact-Index.
6. Compact-count-Array von Licht-Objektreferenzen.
7. u32-`RootOutside` und u32-`Linked` als gespeicherte Bool-Werte.

Die nicht benötigten Bounds-/Hull-/Leaf-Inhalte werden momentan nur in ihrem korrekten Umfang übersprungen; sie sind noch keine implementierte Körper-Kollisionsrepräsentation. Beleuchtungs- und Renderdaten danach bleiben ausdrücklich ungelesen. Der Reader unterstützt hier SWRC-Versionen 151–159, Licensee 0–1.

Zusätzliche lokale Ghidra-Belege: `analysis/decompiled/bsp-collision-tail.c` mit `0x1045c230`, `0x1045d190`, `0x10373590`, `0x1045d340`, `0x10448360`; `bsp-leaf-layout.c` mit `0x1045c7d0`. Der bestehende Node-Serializer `0x1045e000` ordnet die gespeicherten Flags dem Feld bei Windows-Offset `0x47` zu und maskiert sie beim Laden auf `0x1f`.

## Raum- und Linienprüfung

Aus den BSP-Nodes werden Ebenen, Front-/Back-Kinder und Flags übernommen. Der außerhalb liegende Zustand beginnt mit dem gelesenen `RootOutside`. Bei einer CSG-Ebene bestimmt die gewählte Seite den Zustand; bei gesetztem Flag-Bit 0 bleibt der vorherige Zustand erhalten. Negative Kindreferenz `-1` beendet die Traversierung. Der Ebenenabstand benutzt originale Weltkoordinaten und das Vorzeichen des f32-Ergebnisses.

Für eine Linie werden Abschnitte an gekreuzten Ebenen geteilt und beide Teilbäume untersucht. Bereits beide außerhalb liegenden Endpunkte garantieren keine freie Linie: Die Linie kann zwischendurch einen Solid durchqueren. Ebenen, Kindreferenzen und Zyklen werden geprüft; die Traversierung hat zusätzlich ein Budget.

Die Boolean-Linienprüfung ist aus dem lokalen `UModel::FastLineCheck` / `FUN_105520a0` rekonstruiert. Die Punktklassifikation ist die entsprechende Klassifikation einer Position im BSP, **nicht** das vollständige `UModel::PointCheck` mit Extent, Normalen und Depenetration. Vergleich gegen die laufende Windows-Engine, genaue Rundungs-/Grenzfälle, `LineCheck`-Trefferzeit/Normalen und Prüfungen mit Ausdehnung stehen noch aus.

`rc_scene_bsp_query` prüft eine Position in Szenenradien relativ zum Mittelpunkt und eine 1.000 Welt-Einheiten lange Blicklinie. Ergebnis-Bit 0 bedeutet außerhalb/freier Raum, Bit 1 eine freie Linie. `-2` meldet fehlende BSP-Daten; `-1` einen Fehler. Die JNI-Brücke reicht dies an die Diagnoseanzeige weiter. Veraltete Ergebnisse nach Kamera- oder Szenenwechsel werden verworfen.

Die Snapshots v1–v3 behalten ihr Format und enthalten weiterhin keine BSP-Solid-Daten. Ihre sichtbare Geometrie reicht nicht zum Rekonstruieren dieser Zustände. Beim Prüfen einer solchen Datei zeigt der Viewer eine Meldung und empfiehlt den direkten CTM-Import; die Szene bleibt geladen.

## Kartenbefund

`rc-bsp-check` liest die neuen Tails und prüft die Baumstruktur aller **9.123 Modelle in 79 Karten**, ohne Fehler. Für die 666 ausdrücklich gespeicherten Startpunkte prüft es außerdem Punktzustand und sechs achsenparallele 10.000-Einheiten-Linien im jeweiligen Weltmodell. Beleg: `analysis/collision/validation.json`.

`geo_01a.Model1`: 249 Nodes, `RootOutside=False`, `Linked=False`, fünf Zonen, 125 Bounds, 1.476 Hull-Indizes, 46 Leaves und 310 Lichtreferenzen. Bis zu den beiden Bool-Werten sind 51.587 Payload-Bytes gelesen; 771.135 weitere Bytes bleiben ungelesen. `PlayerStart0` liegt im freien BSP-Raum. Die Abwärtslinie ist blockiert; die fünf anderen Achsenlinien sind im Welt-BSP frei. Das passt nicht zu allen sichtbaren Mesh-Treffern aus der vorigen Diagnose: Meshes werden separat kollidiert.

## Klassenvererbung als Grundlage für Defaults

`rc-class-check` folgt den originalen Super-Referenzen der UClass-Exports aus den System-Paketen. 1.302 Klassen haben vollständige aufgelöste Vererbungsketten, ohne Zyklen. Beispiele:

- `Engine.PlayerStart → Engine.NavigationPoint → Engine.Actor → Core.Object`.
- `Engine.StaticMeshActor → Engine.Actor → Core.Object`.
- `CTCharacters.PlayerCommando → CloneCommando → Republic → CTGame.CTPawn → Engine.Pawn → Engine.Actor → Core.Object`.

Der Katalog in `analysis/classes/validation.json` dokumentiert Payload-Orte und Eltern. Er dekodiert noch keine DefaultProperties. Die zuvor ausgelassenen 89 Startpunkte und 21 Mesh-Subclasses werden daher noch nicht automatisch ergänzt.

## Reproduzieren

```powershell
cargo run --offline -p rc-inspect --bin rc-bsp-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Maps' analysis/collision/validation.json
cargo run --offline -p rc-inspect --bin rc-class-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System' analysis/classes/validation.json
```

Emulatorprüfung bei installierter Version 0.7 und bereits gestartetem Testgerät: `scripts/Test-BspEmulator.ps1`. Die Originalkarte und die v3-Szene müssen im Download-Verzeichnis liegen, wie im vorherigen Startpunkt-Test. UI-XMLs, Screenshots, Versionsnachweis und Crash-Puffer werden in `analysis/emulator/bsp-*` gespeichert.

## Nächste Umsetzung

UClass-DefaultProperties und die StaticMesh-Kollisionsrepräsentation lesen; anschließend Pawn-Abmessungen und Bewegung mit Extent prüfen. Eine blockierte Diagnose-Linie ist noch keine korrekte Spieler-Kollision. Der physische ARM64-Gerätetest bleibt offen.

## Verifikation am 6. Oktober 2026

27 Rust-Tests und Clippy über alle Targets bestehen. Neue Tests decken beide Leaf-Formatvarianten und alle Tail-Kürzungen, ungültige Bool-Werte, CSG-/Nicht-CSG-Zustände, Baumzyklen, ungültige Ebenen und Kindreferenzen, invalides Query-Input und native Ergebnisflags ab. Ein Solid zwischen zwei freien Endpunkten wird in beiden Linienrichtungen erkannt.

ARM64-/x86-64-Build 0.7.0-bsp-probe, APK-v2-Signatur und 16-KiB-zipalign bestehen. API-35-Emulator: direkter CTM-Import und Startpunkt, freier Punkt/freie Linie, freier Punkt/blockierte Abwärtsblicklinie, Solid nach Durchfliegen des Bodens, Rückkehr zum Start, Meldung bei v3-Snapshot ohne BSP und erneuter CTM-Import geprüft. Rückkehr und Wiederladen ergeben exakt dieselben Renderpixel; Crash-Puffer leer. Beleg: `analysis/emulator/bsp-validation.json`; Vergleich reproduzierbar mit `scripts/Compare-BspFrames.py` (Python/Pillow).


BSP-Kollisionshüllen und mathematische AABB-Sweeps umgesetzt: iCollisionBound und Hull-Wörter erhalten, 0x40000000-Ebenenflip, Sentinel/64-Ebenen-Limit/Bounds geprüft, feste Blätter anhand des nativen CSG-Extent-Pfads ausgewählt. 9.123 Models in 79 Karten ergeben 26.840 feste Hüllen ohne Lesefehler oder fehlende Hüllen. PlayerCommando-Radius40/Halbhöhe84 aufgelöst; 1.512 Körperprüfungen von 252 Startpunkten in 34 Model1-Karten: 1.019 blockiert, 493 frei, keine Query-Fehler. 503 weitere Startpunkte nicht geprüft: Weltmodell-Referenz im Level muss noch rekonstruiert werden, Model1 bleibt Namensauswahl. geo_01a-Bodenkontakt mit Körper 84 Units vor Mittelpunkt. 49 Tests/Clippy bestanden. f64-Diagnose mit Kantenebenen, keine volle native Toleranz-/Backoff-/Hitdaten-Parität, keine Mesh-Körperprüfung/Spielerbewegung/Android-Integration. APK unverändert0.7, kein neuer Emulatorlauf. Details: D:\Rust Projects\RepublicCommandoAndroid\docs\HULL_SWEEPS.md.
