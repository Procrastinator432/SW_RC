# GEO_01B: Ziele und Begegnungen – 0.34.0

Die zweite Karte verwendet jetzt ausgewählte Originalereignisse für die acht Missionsziele, Fixers Beitritt, fünf Interaktionen, automatische Türen und Gegnerfabriken. Dies ist ein begrenzter spielbarer Ereignis-/Kampfhost, keine vollständige Wiederherstellung der ursprünglichen Skriptmaschine oder KI. GEO_01C wird noch nicht geladen.

## Ausprobieren

Dateien unter `analysis/mission17-map2-gameplay/delivery`:

- `republic-commando-0.34.0.apk`: ARM64 und x86_64.
- `geo_01b.rcplay`: passende neue Szene. Mit der neuen APK öffnen; die alte 0.33.0-App unterstützt die erweiterten Metadaten nicht.
- Die bestehende `analysis/mission15-complete/delivery/geo_01a-first-mission.rcplay` bleibt verwendbar. Am Ende von GEO_01A die neue GEO_01B-Datei auswählen. Gesundheit, Munition und Scorch werden übernommen. Ein gespeicherter GEO_01A-Handoff wird weiterhin automatisch wiederhergestellt.
- Beim direkten Start ohne Handoff beginnt GEO_01B mit dem regulären Port-Spielerzustand und Scorch am originalen Startpunkt.

Bewegen, zielen und schießen wie bisher. Am markierten Sprengpunkt **BENUTZEN drei Sekunden halten**, mindestens 300 Unreal-Einheiten zurücktreten und **FEUER** zum Auslösen drücken. Der Zünder verbraucht keine Patrone; zu geringer Abstand verhindert die Zündung. Für Schildkonsole und Sicherheitstür **BENUTZEN sechs Sekunden halten**. Fixer muss bereits beigetreten sein. Ein kampfunfähiger Scorch aus dem Kartenwechsel lässt sich in seiner Nähe durch drei Sekunden BENUTZEN wiederbeleben; diese lokale Wiederbelebung ist ebenfalls ein begrenzter Host, die originale Liege-/Aufstehanimation fehlt in dieser Karte. Vorzeitige Benutzung überspringt keine Freigabe. Bacta-Stationen heilen beim Halten von BENUTZEN; die Blasterboxen liefern im derzeitigen Einzelwaffen-Host Reservemunition.

## Anbindung an Originaldaten

- Original MissionObjectives Actor142: Infiltrate Geonosian Spire, Rendezvous with Delta Four-Zero, Command demolition maneuver, Command Slice maneuver, Slice secure door, Destroy Geonosian Fighters, Destroy crates, Advance to balcony.
- 31 ausgewählte einmalige Ereignisketten, einschließlich der Ablaufanteile aus Actors516/655/635/438/630/152/569/199/171/829/830/210 und den Original-Trackerwechseln. Wartezeiten der wartenden Originaldialoge stammen aus den PCM-Dateien.
- Originalaktionen536/386/202/163/184: blockierte Tür, Schildkonsole, Sicherheitstür, Jäger und Kisten. Ladungs- und Türereignisse setzen Ziele und physische Freigaben.
- 42 originale PawnFactory-Deklarationen mit Prototypgesundheit, Tag, Kapazität, Intervall und gegebenenfalls gleichnamigen Original-SpawnPoints. Blockierte Punkte werden erneut versucht; die lokale sichere Platzierung sucht höchstens120 Einheiten neben Originalpunkten.
- Original Counter916 zählt die vier markierten Hallendroiden und löst FinalHallwayDroidsDead aus, ohne Tote mehrfach zu zählen.
- 27 aus dem statischen Weltpaket herausgelöste Akteure werden separat dargestellt. Sprengungen entfernen die zugehörigen Objekte; Türen verwenden ihre originalen geöffneten KeyPos-Positionen. Die geschlossen geladene Kollision wird erst nach Freigabe aufgehoben und beim Neustart zurückgesetzt.
- Originaltexturen und komprimierte Animationen für BattleDroid, GeonosianWarrior und Fixer; Fixer verwendet die Skins aus Commando40DeltaDefaults. Scorchs bisherige Originaldarstellung bleibt erhalten.
- 29 Originaldialoge, DC-15s-/Squad-Schuss und originales Sprenggeräusch. Fixer erscheint auch im nativen Beobachtungspaket als freundlicher Akteur.
- Originalteleporter567 meldet GEO_01C.ctm#playerstart erst nach den vorausgehenden Zielen als Abschlussanforderung. Das ist die Grenze dieser Lieferung.

## Nachweise und Grenzen

`check/validation.json` dokumentiert den Einstieg mit240 normalen Bewegungsinputs, die fünf Interaktionsprüfungen, alle42 sicheren Fabrikaktivierungen und den exakten Neustart. Die späteren Interaktionen werden ausdrücklich mit gesetzter Spielerposition und erhöhter Testgesundheit geprüft. Der Fabriktest setzt Originalereignisse gezielt. Das beweist die Hosts und Abschlusskette, aber keinen durchgehenden spielbaren Lauf durch alle Wege und natürlich ausgelösten Begegnungen.

Die Dateien unter `travel`, `native` und `android` enthalten gesonderte Reise-/Plattformprüfungen, soweit im Liefermanifest als bestanden aufgeführt. Die ursprünglichen Lieferungen bleiben unverändert archiviert.

Offen bleiben insbesondere: durchgehender GEO_01B-Eingabelauf mit regulären Ressourcen, vollständige Wegfindung für beide Squad-Mitglieder, originale Flug-/Perch-/Melee-Notifies, vollständige Waffen-/Squad-Schadens- und Wiederbelebungsregeln dieser Karte, alle Nebeninszenierungen, Türbewegung zwischen den Endposen und Checkpoints innerhalb von GEO_01B. Die drei/sechs Sekunden für die lokale Interaktion sind Port-Regeln. Der derzeitige Kampfhost verwendet direkte kollisionsgeprüfte Bodenannäherung, Nahkampf für Warriors, Fernkampf für Droiden und sichtliniengeprüfte Squad-Unterstützung; er ersetzt nicht die detailliertere native KI-Rekonstruktion von GEO_01A. Getrennte Waffenmodelle/-Projektile, Totenanimationen und sämtliche Gegner-Soundnotifies sind hier noch nicht vollständig angebunden. Die bereits dokumentierten Shader-/Falloff-Effekte und zwölf originalen BSP-Nullmaterialflächen sind weiterhin offen, darunter die vollständige Energie-Schilddarstellung. Die erste Gesamtmission benötigt anschließend GEO_01C.
