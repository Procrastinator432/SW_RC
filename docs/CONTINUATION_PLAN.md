# Fortsetzungsplan nach dem ersten spielbaren Abschnitt

Neuester Vorbereitungsstand: **Actor-Fallrechnung und DC-15s-Wurfrelease**. 4096 SSE-Replays bitgenau, originaler Release-Notify bei60Prozent, fünf Channel-gesteuerte Inventarverbräuche und1022 Bibliothekstests geprüft. Noch keine Game-/Touch-/Weltintegration; spielbare Lieferung bleibt0.21.0. Nächste Arbeit: PhysicsVolume/GetNetBuoyancy, tatsächlicher Flug und Ruhe-/Weltkontakte, danach Live-Wurf und Bunkerzustände. [Belege und Grenzen](DETONATOR_FLIGHT_AND_RELEASE.md).

Fortsetzung nach 0.21.0: **Granatenwurf und Geschützablauf als geprüfte Grundlage**. 2052 native Wurf-Replays bitgenau; Zünder/Abprallregeln, GrenadeProj-Kontakt, verzögerte Bunkerzerstörung und Ziel3-Ereignisse separat implementiert. Texturiertes Originalprojektil und acht PCM-Samples vorbereitet,1013 Bibliothekstests bestanden. Noch keine neue spielbare Szene/APK; Actor-Fallphysik, Animationsrelease, Wurfsteuerung und Szenenbindung folgen. [Regeln, Nachweise und nächste Integration](DETONATOR_AND_GUN_POSITION.md).

Aktuelle Lieferung: **0.21.0 / Originalabschnitt bis zur Granatenaufnahme spielbar**. Bacta, fünf Droiden an der Absturzstelle, freigegebener Anschlussweg und fünf Thermaldetonatoren bilden einen zusammenhängenden Lauf. Kiste, HUD, Originaldialoge sowie Aufnahme- und Explosionsgeräusche sind integriert. APK und passende Szene: analysis/mission3-playable. [Starten, Nachweise und Grenzen](ORIGINAL_GRENADE_MISSION.md). Granatenwurf und Originalziel3 folgen separat; Karma-Ruhepose und Kampfregeln bleiben begrenzte Adapter.

Historische Vorbereitung vor 0.21.0: **Originaler Granatenauftrag als Ereignishost und texturiertes Pickup vorbereitet**. Startbestand0, Aufnahme bis5, vier Originalsamples und getrennte Auftrags-/Inventartrigger sind geprüft. Der Anschlussweg zur Kiste besteht die Kollisionsprüfung noch nicht; Karma-Mauer und weitere Mesh-/BSP-Übergänge bleiben zu integrieren. Noch keine neue spielbare Lieferung. [Befunde und nächste Integration](GRENADE_PICKUP_FLOW.md).

Vorherige Lieferung: **0.20.1 / Texturen und DC-15s-Audio korrigiert**. BSP-Boden und bisher fehlende Mesh-Oberflächen besitzen jetzt ihre Originaltexturen mit korrektem UV-Kanal; jeder akzeptierte Spielerschuss löst das originale DC-15s-Sample aus. APK und passende Szene unter `analysis/mission2-av`. [Anleitung, Ursachen und Abnahme](TEXTURES_AND_WEAPON_AUDIO.md). Darstellungs- und Audioqualität werden vor weiteren Missionsabschnitten geprüft; volumetrische Lichteffekte und vollständige Material-/Kampfparität bleiben offen.

Vorherige Lieferung: **0.20.0 / zusammenhängender Originalabschnitt geo_01a**. Bacta-Heilung führt jetzt über zeitgesteuerte Barrikadenfreigabe zur Absturzstelle mit fünf ursprünglichen Battle-Droid-Fabriken, Schutzwechsel, Todeszähler und Abschluss der Originalziele 0 und 1. APK und passende Szene unter `analysis/mission2-playable`. [Starten, Nachweise und Grenzen](ORIGINAL_CRASH_MISSION.md). Kampfregeln und Gegnerverhalten bleiben vorläufig; die gesamte erste Mission und physische ARM64-Abnahme sind offen. Nächster Abschnitt: ursprünglicher Granatenauftrag.

Vorherige Lieferung: **0.19.0 / erster Originalabschnitt geo_01a**. Das ursprüngliche Bacta-Ziel ist vom vorbereiteten Einstieg nach der Landung bis zum Abschluss spielbar: Originalumgebung, DC-15s-Darstellung, Bacta-Station, Advisor-Hinweise, Wegführung und Benutzen-Interaktion. APK und Szene unter `analysis/mission1`. [Starten, Belege und Grenzen](ORIGINAL_LANDING_MISSION.md). Die gesamte erste Mission bleibt in Arbeit; als Nächstes folgt der ursprüngliche Auftrag „Secure area around crash“.

Historischer Grundlagenstand nach 0.19.0: Der ursprüngliche Absturzstellen-Ablauf ist als begrenzter Ereignishost vorbereitet und geprüft (fünf Battle Droids, Unverwundbarkeitsauslöser, Todeszähler). Die Verbindung mit Szene, Kollision und Kampf steht aus; die spielbare Lieferung bleibt unverändert. [Befunde, Prüfungen und nächste Integration](CRASH_SITE_FLOW.md).

Vorherige Lieferung: **0.18.0 / M3l spielbare defensive Sprünge**. Geonosianer verwenden originale JumpDest-Knoten und TakeOff-/Flug-/Landeanimationen; der begrenzte Host prüft Kollision, Unterbrechung und Rückkehr. APK und drei passende Szenen liegen unter `analysis/milestone3l`. [Anleitung, Prüfnachweise und Grenzen](DEFENSIVE_JUMP_SCENE.md). M3 als vollständiger Original-Kampfablauf bleibt offen. Nächster Abschnitt: Zielhilfe/Schussstreuung und Waffen-/Treffer-FX/Audio im ausgewählten DC-17m-Pfad.

Vorherige Lieferung: **0.17.0 / M3i Nahkampf-Budget**, APK und Szenen unter `analysis/milestone3i`. [Anleitung](BOT_GOALS_SCENE.md). M3j/M3k liefern die geprüften Planungs-/Flugbahnkomponenten des neuen Hosts.

Vorherige Lieferung: **0.16.0 / M3h Schadensregeln und Schwierigkeit**, APK und drei passende Szenen unter `analysis/milestone3h`. [Anleitung und Grenzen](GAME_RULES_SCENE.md). Nächster Abschnitt: verbleibende Bot-/Goal-Abhängigkeiten, danach Zielhilfe und FX/Audio. M3 bleibt offen.

Vorherige Lieferung: **0.15.0 / M3g Ausfallschritte**, APK und passende Szene unter `analysis/milestone3g`. [Anleitung und Grenzen](LUNGE_SCENE.md). Nächster Abschnitt: GameRules/Schwierigkeitsgrad; M3 bleibt offen.

Vorheriger Stand 2026-10-11: **0.14.0 / M3f** integriert kollisionsgeprüfte Gegnernavigation und ausgewählte Angriffsziele. [Lieferung, Prüfung und Grenzen](NAVIGATION_SCENE.md). Nächster Abschnitt: Lunge-Bewegung und Machbarkeitsprüfung; M3 weiterhin offen. Die folgenden ursprünglichen Planungsstände bleiben als Historie erhalten.

Stand: 10. Oktober 2026. Planungsgrundlage ist Android 0.8.0 mit dem lokal erzeugten `geo_01a.rcplay`. Meilenstein 0 ist mit Android 0.8.1 abgeschlossen. Meilenstein 1 ist als Android 0.9.0 funktional umgesetzt und im Emulator geprüft; seine physische ARM64-Leistungsabnahme bleibt offen.

Aktueller Integrationsstand 0.11.0: Der Nutzer bestätigt auch die animierte M2-Version auf seinem Gerät. M3 ist begonnen: Original-Munitions-/Schadensdefaults, Projektiladapter und originale Schuss-/Notifygeräusche sind im separaten `analysis/milestone3/geo_01a-combat.rcplay` integriert. [Umfang, Nachweise und verbleibende M3-Arbeit](COMBAT_SCENE.md). M3 ist noch nicht vollständig abgenommen; native Laserbolzenrechnung, Schadensmodifikatoren und Geonosian-KI folgen vor M4. Die quantitative ARM64-Dauerabnahme bleibt offen.

## Ziel der nächsten Ausbaustufe

Ziel ist die erste Originalmission, zunächst in kurzen, zusammenhängenden Abschnitten. Die ersten beiden Aufträge „Use bacta to heal“ und „Secure area around crash“ sind als zusammenhängender Abschnitt nach der Einleitung spielbar. Als Nächstes folgen Granaten, Geschützstellung, Six-Two, Sprengkommando und Eintritt in den Turm. Kampfparität und fehlende Umgebungselemente werden zusätzlich schrittweise rekonstruiert. Die eigene Kampf-Testszene bleibt eine technische Regression.

Jeder Abschnitt endet mit einer startbaren Lieferung, nachvollziehbarer Originalherkunft und konkreten Prüfbelegen. Die ausgesparten Einleitungs-/Script-/Physikfälle bleiben ausdrücklich inventarisiert; eine größere eigene Demomission wird nicht als Ersatz für das Original aufgebaut.

## Was wir aus dem bisherigen Abschnitt gelernt haben

| Erkenntnis | Konsequenz für die Fortsetzung |
|---|---|
| Paketdaten, Originalumgebung, Bewegung, Kollisionsadapter, Rust-Spielkern und Android-Host lassen sich gemeinsam betreiben. | Die bestehende Architektur weiterverwenden und gezielt erweitern. |
| Viele originale Animations-, Skinning-, Material- und Renderfunktionen sind bereits separat geprüft. Im Spiel erscheinen noch vorläufige Körper und Waffen. | Als Nächstes diese vorhandenen Ergebnisse an echte Spielinstanzen anschließen. |
| Der aktuelle Bildpfad rastert auf der CPU und überträgt pro Bild ein vollständiges Pixelarray über JNI in ein Android-Bitmap. | Laufzeiten und Kopien messen, die Grafikausgabe vom Simulationsschritt lösen und einen kleinen GPU-Pfad aufbauen. |
| Kampf, Gegnerverhalten, Exfil und vorbereiteter Bodenspawn verwenden eigene Regeln. | Für jedes ersetzte Verhalten seine Originalklasse, Defaults, Ereignisse und nativen Abhängigkeiten nachweisen. |
| 930 Rust-Tests sowie Headless-, ABI- und Android-UI-Prüfungen sichern den ersten Abschnitt ab. | Diese Prüfungen als Regression verwenden und neue Tests auf neue Verhaltensrisiken konzentrieren. |
| Emulator-Tests belegen Laden, einzelne Touch-Aktionen, Sieg/Niederlage, Pause und Fehlerbehandlung. | Gleichzeitige Mehrfingerbedienung, physisches ARM64, Dauerlast und reale Bildrate separat prüfen. |

Der Stand belegt einen funktionierenden ersten Abschnitt. Testzahlen und einzeln rekonstruierte Funktionen ergeben noch keine belastbare Prozentzahl für die vollständige Portierung.

## Meilenstein 0: Referenz sichern und Engpässe messen

**Abgeschlossen:** Referenzarchiv, 1.002 Eingabekommandos, Zustands-/Bildvergleich, PC-/Android-Messungen und Emulator-Mehrfinger-/Fokusprüfung. 934 Tests bestehen; die CPU-Szenenzeichnung ist der bestätigte Hauptengpass. Physische Geräteprüfung bleibt offen. [Ergebnisse und Nachweise](BASELINE_AND_PROFILING.md).

**Ergebnis:** Eine reproduzierbare Ausgangsversion und ein nachvollziehbares Leistungsprofil.

- APK, Szenenpaket, Quellenstand, Werkzeuge und vorhandene Hashnachweise als Version 0.8.0 festhalten. Die bestehenden lokalen Änderungen zuerst zuordnen; einen konsistenten Stand für die spätere Versionierung vorbereiten.
- Eine wiederholbare Eingabespur für Laden, Bewegung, Sprung, Gefecht, Nachladen, Sieg, Niederlage und Neustart erfassen. Szenenhash, Eingaben, Zeitzuordnung und spätere Zufallsseeds gehören zum Replay. Vorhandene direkte Headless-Steuerung bleibt ein ergänzender Test.
- Zeit für Simulation, Kollisionsabfragen, Geometrieaufbereitung, Rasterung, JNI-Übertragung und Android-Bildausgabe getrennt messen. Ladezeit, Speicher, Bildzeit-Perzentile und verworfene Simulationszeit aufzeichnen.
- Eingaben über mehrere Pointer, Fokusverlust, Dateidialog und Pause gezielt prüfen. Physischer Mehrfingertest bleibt offen, bis ein Gerät verfügbar ist; die Implementierung kann währenddessen fortgesetzt werden.

**Abnahme:** Der bestehende Funktionsablauf besteht unverändert. Ein gespeicherter Bericht benennt den dominanten Engpass und die konkrete nächste Optimierung. Messungen enthalten Gerät/ABI, Buildmodus, Auflösung und Testszenario.

## Meilenstein 1: Stabile Darstellung und Bedienung unter Last

**Funktional umgesetzt:** Unveränderliche Renderframes, OpenGL-ES-Welt/Texturen/Tiefe/Actors/HUD, CPU-Umschalter, Wiederaufbau nach Kontextverlust und begrenzte Ressourcenlebensdauer. Referenzbilder, Spielabläufe und Mehrfinger-/Fokusprüfung sind über beide Pfade abgesichert. Die physische Geräte-/Dauerlastabnahme bleibt offen. [Implementierung und Prüfnachweise](GPU_SCENE.md).

**Ergebnis:** Dieselbe Szene und derselbe Spielzustand laufen über eine erweiterbare Grafikausgabe.

- Den Simulationsschritt von der Bilderzeugung trennen. `rc-game` liefert Kamera, sichtbare Instanzen, HUD-Zustand und Spielereignisse; der Grafikadapter verarbeitet diese Daten. Android-Eingabe und Lebenszyklus bleiben Hostaufgaben.
- Unveränderliche Weltgeometrie und Texturen einmal laden. Pro Bild nur geänderte Instanz-/Kameradaten übertragen. Eigentum, Freigabe und Wiederaufbau der Grafikressourcen ausdrücklich festlegen.
- Einen GPU-Pfad zunächst für die vorhandene Welt mit Basistexturen, Tiefentest, Kamera, vorläufigen Gegnern und HUD umsetzen. Die Backend-/API-Wahl erfolgt anhand der vorhandenen Android-Anbindung, der Zielgeräte und eines kleinen gemessenen Prototyps.
- Den Software-Renderer als Diagnose- und Vergleichspfad behalten. GPU-Ausgaben über Projektion, Verdeckung, Texturbelegung und Referenzansichten vergleichen; unterschiedliche Rasterizer benötigen passende Bildtoleranzen.
- Oberflächenverlust, Pause/Fortsetzen, Neustart und Szenenwechsel prüfen. Touch-Empfindlichkeit und Bedienelemente an unterschiedliche Seitenverhältnisse anpassen.
- Kollisions- oder Speicheroptimierungen nach dem Profil aus Meilenstein 0 angehen. Die bisherige Fehlerbehandlung und Zustandsübernahme bleiben dabei erhalten.

**Abnahme:** Alle bisherigen Spielabläufe bestehen über den neuen Bildpfad. Bewegung, Schussrichtung und sichtbare Kamera stimmen überein. Ressourcen werden nach Oberflächenverlust korrekt wiederhergestellt; wiederholtes Laden erzeugt kein fortlaufendes Speicherwachstum.

**Vorläufiges Leistungsziel:** Auf einem ausdrücklich benannten physischen ARM64-Referenzgerät im Release-Build zehn Minuten bei festgehaltener interner Auflösung von mindestens 640 × 400 spielen; 95. Perzentil der Bildzeit höchstens 33,3 ms, keine dauerhaft anwachsende Simulationsverzögerung. Das ist ein Zielwert. Ein Emulatorlauf ersetzt diese Geräteabnahme nicht. Nach der ersten Messung werden Zielauflösung und Geräteklasse verbindlich festgelegt.

## Meilenstein 2: Originale Gegner und Waffen sichtbar machen

**Ergebnis:** Der bekannte Abschnitt enthält erstmals echte animierte Spielinstanzen aus den Originalassets.

- Aus den tatsächlichen geo_01a-Referenzen einen passenden Gegner und eine passende Spielerwaffe auswählen. Mesh, Material, Textur, Skelett, Animationspaket und Sequenznamen vollständig auflösen.
- Vorhandene Leser und Rechenwege für Originaltracks, Linkups, vollständige Pose, Skinning, Abschnitte und Meshtransformationen wiederverwenden. Mit einem Gegner beginnen; danach dieselbe Integration auf die drei Begegnungsinstanzen anwenden.
- Idle, Bewegung, Angriff, Treffer und Tod an den Spielzustand binden, soweit die ausgewählten Originalassets diese Abläufe bereitstellen. Fehlende Zuordnungen sichtbar dokumentieren.
- Animationen im festen Spieltakt aktualisieren. Ereignisse an Sequenzgrenzen, Loop-Übergänge, Pause und Neustart prüfen. Materialauflösung, Instanzlebensdauer und Cacheinvalidierung in die Laufzeit übernehmen.
- Ein versioniertes Szenenpaket um die benötigten dynamischen Assets erweitern. Das Laden der bestehenden `RCPLAY01`-Referenz erhalten oder einen ausdrücklichen, geprüften Migrationsweg bereitstellen. Originaldaten bleiben in lokalen Datenpaketen getrennt von der APK.

**Abnahme:** Mindestens ein vollständig aufgelöster Originalgegner ist am korrekten Weltort texturiert und animiert sichtbar; alle drei Gegnerinstanzen durchlaufen ihre Zustände ohne gemeinsame Cachefehler. Die Originalwaffe folgt Kamera und Animation. Treffervolumen, sichtbarer Körper, Verdeckung und Reset passen zusammen. Neue Nachweise unterscheiden weiterhin originale Darstellung und vorläufige Kampfregeln.

## Meilenstein 3: Einen vollständigen Original-Kampfablauf integrieren

**In Arbeit:** Erster Integrationsabschnitt in Version 0.11.0. [Kampfversion und klare Adaptergrenzen](COMBAT_SCENE.md). Originaldefaults und ausgewählte Sounds/Notifies laufen in der Szene; ursprüngliche KI, native Projektilrechnung und vollständige Schadensregeln fehlen noch.

**Ergebnis:** Eine belegte Originalwaffe und ein belegter Gegnertyp funktionieren gemeinsam im Spielkern.

- Den ausgeführten Pfad vom Eingabeereignis über Controller, Inventar/Waffe und Schuss bis zu Treffer, Schaden und Tod inventarisieren. Gespeicherte Quellen, Bytecode, Defaults und native Implementierungen gemeinsam prüfen.
- Für die ausgewählte Waffe Feuerrate, Munition, Nachladen, Treffer-/Projektilmodell, Schadensregeln und notwendige Animationsereignisse übernehmen. Der tatsächlich ausgewählte Originalpfad entscheidet über Hitscan oder Projektil.
- Für einen Gegnertyp Erkennen, Annähern, Angreifen, Trefferreaktion und Sterben integrieren. Erforderliche Wegfindung für den ausgewählten Bereich ergänzen, sobald direkte Annäherung nicht ausreicht.
- Originale Schuss-, Treffer-, Nachlade- und Gegnergeräusche auflösen und zeitlich an die Rust-Spielereignisse binden.
- Die benötigten Actor-/Script-Ereignisse über klar definierte Adapter ausführen. Eine allgemeine VM wird relevant, wenn die erfassten Abhängigkeiten zeigen, dass einzelne Adapter den Missionspfad nicht mehr sinnvoll abdecken.

**Abnahme:** Ein dokumentiertes Gefecht läuft von normaler Eingabe bis zum Tod beider möglicher Parteien vollständig durch. Die übernommenen Regeln besitzen Originalbelege und Vergleichsproben; verbleibende Ersatzregeln sind benannt. Animations- und Audioereignisse werden auch bei schwankenden Bildzeiten jeweils korrekt ausgelöst.

## Meilenstein 4: Zusammenhängende geo_01a-Mission

**Ergebnis:** Ein zusammenhängender Abschnitt mit erkennbarem Anfang, Fortschritt und Abschluss; als vorläufiges Umfangsziel etwa fünf bis zehn Minuten Spielzeit.

- Die benötigten Originalactors, Start-/Cutscene-Abhängigkeiten, Trigger, Ziele und Begegnungen des gewählten Bereichs als Abhängigkeitsgraph erfassen. Daraus einen realistischen Missionsausschnitt festlegen.
- Den vorbereiteten Bodenspawn schrittweise durch den für diesen Ausschnitt belegten Originalstart ersetzen. Notwendige Türen/Mover mit sichtbarer Bewegung und aktualisierter Kollision verbinden.
- Ziele, Dialoge und Begegnungen über die erforderlichen Originalereignisse starten. Abhängige Funktionen jeweils bis zum sichtbaren Ergebnis integrieren und prüfen.
- Einen Checkpoint mit versioniertem Spielstand ergänzen: stabile Actor-Identitäten, Mission, Spieler, Inventar, Gegnerzustände und erforderliche Animations-/Zufallszustände. Grafik-Handles und temporäre Caches beim Laden neu aufbauen.
- HUD und Rückmeldungen so erweitern, dass Ziel, Schaden, Munition, Interaktion und Abschluss ohne Diagnosekenntnisse verständlich sind.

**Abnahme:** Ein neuer Spieler kann den Abschnitt vom Start bis zum Abschluss durchlaufen. Tod und Checkpoint-Wiederherstellung, erneutes Laden sowie ein zweiter vollständiger Durchlauf funktionieren. Geräusche, Trigger und Ziele sind zeitlich konsistent. Der enthaltene Originalumfang ist ausdrücklich beschrieben.

## Meilenstein 5: Squad und weitere Kampagne

**Ergebnis:** Der Kern wird um die für Republic Commando charakteristischen Begleiter und weitere Missionsbereiche erweitert.

1. Einen Begleiter mit Folgen, Wegfindung, Kampfbeteiligung und Wiederbelebung integrieren; danach die übrigen Squad-Mitglieder.
2. Einen vollständigen Squad-Befehl mit Zielerkennung, Bewegung, Ausführung, Abbruch und Rückmeldung umsetzen. Weitere Befehle anhand der Original-Missionsabhängigkeiten ergänzen.
3. Den nächsten verbundenen Kartenbereich übernehmen und Levelwechsel, Inventar-/Squad-Übernahme sowie Checkpoints prüfen.
4. Übertragbare Import- und Laufzeitregeln auf zusätzliche Karten anwenden; verbleibende Materialeffekte, Spezialgegner, Audio- und Scriptpfade nach tatsächlichem Bedarf vervollständigen.

**Abnahme pro Erweiterung:** Ein spielbarer Missionsablauf einschließlich Speichern/Laden und Wiederholung besteht auf Desktop-Prüfpfad und Android. Erst belegte Kartenabdeckung und Systemabdeckung ermöglichen eine fundierte Gesamtfortschrittsschätzung.

## Die nächsten fünf konkreten Arbeitspakete

| Reihenfolge | Aufgabe | Sichtbares bzw. prüfbares Ergebnis |
|---|---|---|
| 1 (erledigt) | Version 0.8.0 samt Artefakten, Quellenzuordnung und Ausgangsnachweisen sichern. | Wiederherstellbare Referenz und kurze Liste der noch ungeprüften Punkte. |
| 2 (erledigt) | Eingabe-Replay und getrennte Laufzeitmessung für den bestehenden Abschnitt ergänzen. | Reproduzierbarer Funktionslauf und Engpassbericht. |
| 3 (erledigt) | Simulation und Renderdatenübergabe trennen; vorhandene Ausgabe darüber betreiben. | Unverändertes Gameplay und geprüfte gemeinsame Kamera-/Instanzdaten. |
| 4 (umgesetzt; Geräteabnahme offen) | Einen GPU-Prototyp für geo_01a mit Basistexturen und Tiefe bauen und messen. | Tatsächlich dargestellte Originalszene, Vorher-/Nachher-Messung und begründete Backend-Entscheidung. |
| 5 (umgesetzt) | Einen passenden Originalgegner vollständig auflösen und die vorhandene Pose-/Skinningstrecke im Abschnitt anschließen. | Eine animierte Originalinstanz mit geprüfter Welttransformation und Materialbelegung. |

Diese Pakete sind Arbeitsziele und können nach der jeweiligen Untersuchung in kleinere Schritte zerfallen. Die Produktionsreife des GPU-Pfads und die weiteren Actor-/Waffeninstanzen gehören zu den anschließenden Meilenstein-Abnahmen.

## Arbeits- und Prüfregeln

- Jede Rekonstruktionsaufgabe beginnt mit einer benötigten Spielfunktion und endet mit ihrer Integration oder einem konkreten dokumentierten Blocker samt nächstem Schritt.
- Für jeden Meilenstein Originaldaten, rekonstruierte Originalrechnung, eigene Adapter und vorläufige Regeln getrennt nachweisen. Ein funktionierender Ersatzmechanismus bleibt als solcher erkennbar.
- Nach Änderungen gezielte Verhaltenstests, dann die erforderlichen Workspace-/ABI-Prüfungen durchführen. Bestehende Szenenabläufe als Regression verwenden. Pixelgleichheit nur innerhalb eines dafür geeigneten Renderpfads verlangen.
- Android am Ende eines zusammenhängenden Integrationsmeilensteins prüfen, entsprechend der bisherigen Nutzerpräferenz. Der spätere persönliche Test des Nutzers ist zusätzlicher Befund; die Entwicklung kann bis dahin weitergehen.
- Funktionale Fehler, Bedienprobleme und bestätigte Engpässe aus diesem Test vor weiteren Inhalten priorisieren.
- Fortschritt an erfüllten Abnahmekriterien, spielbaren Funktionen, Originalparität und Geräteprüfung berichten. Zeit-/Restaufwandschätzungen nach den ersten Profil- und Integrationsaufgaben neu bewerten.

## Lokale Grundlagen

- [Spielbarer Abschnitt und vorhandene Nachweise](PLAYABLE_SCENE.md)
- [Bisheriger Portierungsweg](PORTING.md)
- [Originale Animationstracks](ORIGINAL_ANIMATION_TRACKS.md)
- [Original-Mesh-/Animationszuordnung](ORIGINAL_SKELETAL_LINKUPS.md)
- [Geladene Animation bis zur vollständigen Pose](SKELETAL_LOADED_FULL_TICK.md)
- [Gewichtetes Skinning](SKELETAL_WEIGHTED_SKIN.md)
- [Skelettabschnitte und Renderübergabe](SKELETAL_DRAW.md)
- [Zusammengeführter Vertexmesh-Pfad](VERTMESH_RENDER.md)
- [Renderertransformationen](RENDERER_TRANSFORM.md) und [berechnete Inverse](RENDERER_INVERSE.md)
- Vorhandene Referenz: `analysis/playable/validation-manifest.json`, `analysis/playable/android/validation.json` sowie `analysis/playable/abi-validation.json`.


## M3-Fortsetzung 2026-10-10: Geonosian-Angriffsentscheidung

Vorbereitete native Manöverwahl portiert und in 9.408 begrenzten Originalinstruktions-Replays geprüft; alle 898 Pakettests bestehen. ForcePike statt angenommenem Fernangriff, bewaffnete/unbewaffnete Karteninstanzen und originale MeleeHit-Zeiten aufgelöst. Noch keine Gameplay-Integration oder neue APK. Nächster zusammenhängender Schritt: GOAL_MeleeAttack-Ausführung, vier Angriffsclips und MeleeHit an den geklärten Schadenspfad anschließen; anschließend Android prüfen. [Befunde und Integrationsgrenzen](GEONOSIAN_ATTACK.md).


M3-Fortsetzung 2026-10-10: Die native DamageVariance-Argumentbelegung ist anhand der Instruktionen geklärt. Varianz und vorbereitete Pawn-Schadensrechnung sind portiert; 4.038 Originalfälle und 958 Workspace-Bibliothekstests bestehen. Originaldaten zeigen 100 CloneCommando-Schilde, 62,5 ForcePike-Schaden vor weiteren Spielregeln und unterschiedliche DC-17-Schadenstypfaktoren. Noch keine Anbindung an die spielbare Szene oder neue APK. Nächste Integration: f32-Gesundheit/Schilde, Varianz/GameRules-Grenze und Melee-Goal/Clip/Notify zusammenführen. [Schadenspfad und Nachweisgrenzen](DAMAGE_PATH.md).


## M3d: spielbare Nahkampfintegration

Die neue Szene und APK 0.12.0 verbinden Original-Angriffsclips/Hit-Notifies mit fractional Gesundheit, Schildregeneration und ausgewählten DamageVariance-Listen. Bisherige vorbereitete Bausteine sind teilweise integriert; Navigation, native Goal-Ausführung, Lunge-Bewegung und GameRules bleiben offen. Aktuelle Nachweise, Lieferdateien und Abgrenzungen: [MELEE_SCENE.md](MELEE_SCENE.md).


## M3e, 2026-10-11: Projektil- und Schussursprung-Integration

Android 0.13.0 und geo_01a-ballistics.rcplay verbinden vorbereitete native Vektorbewegung, originalen FPFireOffset, direkte Zielpunktkorrektur und die Näheprüfung vor dem Spawn mit der Nahkampfszene. Mathematik-/Kollisionsgrenzen und verbleibender AimError sind ausdrücklich dokumentiert. Aktueller Stand und nächste Schritte: [BALLISTICS_SCENE.md](BALLISTICS_SCENE.md).


2026-10-11: M3f integriert Gegnernavigation und ausgewählten Goal-Host. Neue Lieferung Android 0.14.0/Code16 plus geo_01a-navigation.rcplay unter analysis/milestone3f. 37 originale, bodengeprüfte PathNodes und 68 gerichtete PathList/ReachSpec-Gehverbindungen; eigener begrenzter Dijkstra-Host, Neuplanung/Warten, vorbereitete native Manöverwahl und MeleeGoal-Reihenfolge. Originalkarten-Umweg PathNode3 -> PathNode191 mit Notify-Treffer, Zielwechsel, Tod, Warten, Reset, Bildratenvergleich und drei Abschüssen geprüft. Native FollowLinks/GoalQueue, Lunge/Sprung, Angriffsanzahlfeld und GameRules bleiben offen. Details: D:/Rust Projects/RepublicCommandoAndroid/docs/NAVIGATION_SCENE.md. Nächster Abschnitt: Lunge-Bewegung.


2026-10-11: M3g integriert die originalen Move-Track-Kurven von Lunge/Lunge2, vollständige statische Kurvenprüfung, Bewegungs-Sweeps, MeleeHit und Abbruch/Absetzen. Neue APK0.15.0/Code17 plus geo_01a-lunge.rcplay unter analysis/milestone3g. Originale Root-Translation mit bestehender Szenenskalierung; kein vollständiger nativer PHYS_RootMotion-/GoalQueue-Port. Anleitung und Grenzen: docs/LUNGE_SCENE.md. Nächster Abschnitt: GameRules/Schwierigkeitsgrad und aktive Schadensmodifikation.


2026-10-11: M3k übernimmt die vorbereitete originale CalcTrajectoryVelocity-Rechnung mit expliziter RSQRTSS-Seed-Grenze. 4.240 native Instruktionsfälle, 983 Bibliothekstests und bestehende Szenen-/Replay-Regressionen bestehen. Gespeicherte Sequenztranslation ist als eigene Laufzeitdaten-Abhängigkeit identifiziert. Keine neue APK/Szene; 0.17.0 bleibt spielbar. Als Nächstes Sequenztranslation/ChangePhysics/Start-/Landekette und ausführbaren Goal-Host integrieren, danach Android prüfen. [Nachweise und Grenzen](JUMP_TRAJECTORY.md).
