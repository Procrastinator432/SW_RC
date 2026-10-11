# Erster Originalabschnitt: geo_01a – Bacta-Tutorial

Version **0.19.0-original-landing / Code21**, 2026-10-11.

Diese Lieferung spielt den ersten ursprünglichen Missionsauftrag **„Use bacta to heal“** in geo_01a. Sie beginnt nach der Gunship-/Abseil-Einleitung und der geskripteten Verwundung. Der Spieler geht von der Landezone zur originalen Bacta-Station, hält **BENUTZEN** gedrückt und beendet den Abschnitt bei vollständig wiederhergestellter Gesundheit. Das ist der erste begrenzte Abschnitt der Originalmission; die gesamte erste Mission ist noch nicht fertig.

## Starten

1. `analysis/mission1/republic-commando-0.19.0.apk` installieren. ARM64 und x86_64 sind enthalten.
2. Die dazugehörige Datei `analysis/mission1/geo_01a-landing.rcplay` auf das Gerät kopieren.
3. **Spielabschnitt geo_01a öffnen → Szene öffnen** wählen und diese Datei laden.
4. Mit dem linken Joystick laufen und rechts die Blickrichtung drehen. Die obere Anzeige führt über sieben Wegpunkte zur Bacta-Station; grüne Markierungen helfen bei der Orientierung.
5. An der Station in Richtung des Geräts schauen und **BENUTZEN** halten. Loslassen unterbricht die Heilung. Bei 100 Gesundheit erscheint der Abschluss. **Neustart** beginnt wieder an der Landezone.

Am Desktop entspricht **E** der Benutzen-Taste. Pause/Fokusverlust stoppt Bewegung und Heilung. Die bisherige Kampf-Testszene liegt weiterhin separat unter `analysis/milestone3l`.

## Belegter Originalpfad

- `geo_01a.ctm`, Export10 `PlayerStart0`: Start und StartInventory `CTInventory.DC15s`.
- Export16 `MissionObjectives0`: sieben Originalziele, beginnend mit Bacta. Alle Zieltexte und 92 ScriptedSequence-/ScriptedTrigger-Aktionslisten sind in `geo_01a-original.json` vollständig extrahiert.
- Script523: Abseilen, `CheckpointLevelStart`, Advisor-Auftrag, anschließend `StartBactaTraining`.
- Script497, Aktion2397: 175 Schaden ohne Schwierigkeitsfaktor, danach Hinweis auf Bacta und Sichtbarmachen des ersten Missionsziels.
- Export736 `BactaDispenserLZ`, Originalmodell `BactaDispensers.BactaDispenserGEO`; Export737 ist der zugehörige Standplatz.
- `BactaDispenser.BeginActivating`: pro Takt `deltaTime * MaxHealth / BactaRate`, begrenzt auf die fehlende Gesundheit. Originaldefault **BactaRate4**, Health100; Einstieg mit vorbereiteten 25 Gesundheit nach dem Verwundungsabschnitt.
- Script739 wartet auf **BactaUsed** und führt Aktion2901 `ACTION_ObjectiveUpdate` aus. Der Originaldefault schließt Ziel0 ab. Die Lieferung setzt denselben Zielmodus auf „abgeschlossen“ und endet an dieser klaren Abschnittsgrenze.
- Drei originale Advisor-Zeilen aus `GEO_Voice`: `CA1G1749`, `CA1G1747`, `CA1G1791`. Der begrenzte Host spielt sie als Hinweis und Abschlussmeldung ab; er führt keine allgemeine Dialog-VM aus.

## Umsetzung und Grenzen

Der neue RCPLAY04-Wrapper enthält Missionsdaten und eine visuelle RCPLAY02-Basis. Ältere Szenen behalten ihre bisherigen Regeln. Die drei erfundenen Testgegner werden in diesem Abschnitt entfernt: Der Intro-Geonosianer läuft im Original geskriptet ab und ist hier zusammen mit der Einleitung ausgespart. Der erste freie Kampfauftrag folgt erst nach dem Bacta-Ziel.

Der Start verwendet die Original-PlayerStart-XY-Koordinaten, auf den vorhandenen Boden gesetzt. Originale Navigationspunkte liefern die Wegpunkte; Verbindungen und Führung sind eigene, mit der Spieler-Körperphysik geprüfte Hilfen. Das Bacta-Modell wird in Referenzpose mit Basistexturen dargestellt. Bodenanpassung und eine Box für die ursprüngliche Zylinderkollision sind begrenzte Adapter; Geräteanimation, Bacta-Blitze und die originale Aktivierungszustandsmaschine fehlen noch. Reichweite, Blickprüfung und Touch-Zuordnung sind ebenfalls Adapter.

Die DC-15s ist original dargestellt. Ihre vollständigen Waffenregeln sind in diesem friedlichen Tutorialabschnitt noch nicht rekonstruiert; die bisherigen vorläufigen Feuer-/Nachladeregeln bleiben bestehen. Gunship-Kinematik, Abseilanimation, geskriptete Verwundung, Intro-Geonosianer, Schilde, allgemeine Script-VM und originale Savegames sind nicht Teil dieser Abnahme. Neustart stellt den vorbereiteten Abschnittseinstieg wieder her.

Der Wegtest deckte einen Fehler im bestehenden Bewegungshost auf: Eine begehbare Schräge löschte die horizontale Bergaufgeschwindigkeit. Begehbare Bodenflächen werden jetzt beim horizontalen Geschwindigkeitsabgleich von Wänden unterschieden. Eine eigene Schrägenregression und der tatsächliche Weg zur Station sichern die Korrektur ab.

## Abnahme

986 Bibliothekstests, Clippy, Format und Whitespace-Prüfung bestehen. Die bisherige Kampf-/Sprungszene und das 1002-Kommandos-Replay bleiben geprüft.

Nachweise werden in `analysis/mission1/validation-manifest.json` mit Szene, APK, Originaldateien und Quellarchiv gebunden. Der gemeinsame Rust-Test läuft den gesamten Weg durch Körperkollision, prüft gedrücktes Benutzen, Unterbrechung durch Loslassen, Ablehnung aus der Entfernung, Zielabschluss, exakten Neustart und Bewegung bei 120Hz. ABI-Prüfungen sichern Puffergrenzen, ungültige Handles/Flags und einmaligen Verbrauch der drei Audioereignisse ab.

Android prüft den Weg und die Benutzung mit normalen Touch-Eingaben im API35-x86_64-Emulator, jeweils über GPU und CPU. Die Automation liest Positionsmessungen zur Orientierung; sie setzt weder Position noch Gesundheit oder Ziele. Grafikvergleich, Kontextneubau und wiederholtes Laden werden separat geprüft. Die Abnahme auf einem physischen ARM64-Gerät steht für diese Lieferung noch aus.

## Fortsetzung der Originalmission

Als Nächstes folgt der ursprüngliche Auftrag **„Secure area around crash“**. Dafür müssen `BactaUsed`-Folgeaktionen, Barrikadenfreigabe, Weg zur Absturzstelle, originale Gegnerfabriken und `CrashSiteSecured` zusammengeführt werden. Danach folgen Granatenaufnahme/Geschützstellung, das Treffen mit Six-Two, Sprengkommando und Eintritt in den Geonosianischen Turm. Diese Ziele sind bereits aus der Karte inventarisiert, noch nicht spielbar umgesetzt.
