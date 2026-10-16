# Fortsetzungsplan nach dem ersten spielbaren Abschnitt

2026-10-16: **Zweiter Handytest – Korrekturblock in Arbeit.** Soundbank-Verschiebung in GEO_01A korrigiert, alle24 Intro-/Abseil-Cuereferenzen an Originalquelle/PCM gebunden, gemeinsame Materialentscheidung und vier zusätzliche Stufen für vorhandene A/B/C/Fabrik-Szenen umgesetzt. Doppelte Intro-Root-Translation beim Zeichnen entfernt. Der gesamte Inszenierungsblock und die Geräteabnahme sind offen; keine neue Releasefreigabe. [Umfang, Belege und konkret offene Schritte](SECOND_PHONE_TEST.md).


**Vorrang seit dem Handytest (16.10.2026): Mission 1 ist eine Alpha mit offenen Blockern.** Die historischen Abschlussangaben belegen vorbereitete Replays, keine unabhängige Geräteabnahme. 0.50.0 bearbeitet sichere Rückfallpunkte/Todesschleifen, die unsichtbare Rampe, gekoppeltes Droiden-Schussfeedback und verzögertes Audio. Danach folgen der frei gespielte Geschützabschnitt, die vollständige Sicht-/Hörprüfung aller Gegnertypen und des Intros und anschließend das Texturqualitätsprofil. Neue Missionsinhalte und Touch-Feinschliff sind nachrangig. [Konkrete Änderungen und offene Abnahme](PHONE_TEST_BLOCKERS.md).

2026-10-15: **0.49.0 / Kampagnenbedienung erreicht.** Einmaliger streamender Vier-Karten-Paketimport mit SHA-256/native Kartenkennung und atomarer Veröffentlichung, Start-/Fortsetzen-Menü, automatische A→B→C→Fabrik-Wahl ohne Dateidialog, kompakte48dp-Spielzeile/Pausenmenü und persistente50–200%-Blickempfindlichkeit. Original-Neue-Mission-Activity lädt nach Android-Hintergrundkill den exakten Checkpoint statt erneut beiTick0 zu beginnen. Je1.124Debug-/Release-Bibliothekstests, Clippy/Java, echter Paketimport unter96MiBHeap,16Touch-Layoutvarianten undAPK-Bedientest bei640×360dp mitBewegung/Schuss/Sprung, beidenBlickachsen, echtemProzessverlust undallen drei automatischenÜbergängen bestehen. Gerätegefühlauf physischemARM64 undmanuellerTouch-Gesamtlauf bleibenoffen. [Bedienung und Nachweise](CAMPAIGN_MENU.md).

2026-10-15: **0.48.0 / Build und Quellstand konsolidiert.** Bisherige Missions-/Checkpointquellen separat als ea9d212 gesichert. Debug-Stackoverflow im Bundle-Decoder durch getrennte Formatfunktionen behoben; Regression mit explizit2MiB. Zentraler Einstieg `build.ps1`: Debug/Release jeweils1.123Bibliothekstests, Doc-Tests, Clippy allerTargets, native Builds, synthetische Java-Prüfungen, Android beiderABIs und deklarierte B/C/Fabrik-Szenenpipeline. 14Builder nutzen benannte SHA-256-gebundene Eingaben statt historischer Mission-Verzeichnisse; Archiv bleibt unangetastet, lokale IDE-Daten bleiben erhalten. Quellprüfung zusätzlich im frischen Checkout ohne Analyse-/Assetbestand; lokaler Abschluss-Tag v0.48.0. [Build-Anleitung und Grenzen](BUILD.md).

2026-10-15: **0.47.0 / Checkpoint-Wiederherstellung gehärtet.** Vorheriger Checkpoint wird nach erfolgreichem Laden sofort wieder zum Hauptspielstand; abgewiesene Szenen ändern die Wiederherstellungsreferenz nicht. Neues Speicherfile wird vor Backup-Rotation vollständig geschrieben. Java- und echte Android-Dateischreibfehler, Wiederholungsversuch, drei kalte Fortsetzen-Zyklen und drei echte C-Missionszwischenstände mit anschließendem Prozessabbruch geprüft. Native Bibliotheken und Snapshot-ABI bleiben gegenüber0.46.0 bytegleich; vorhandene Szenen/Spielstände bleiben verwendbar. Physisches ARM64, tatsächliche Datenträgererschöpfung und Stromausfall bleiben offen. [Anleitung und Nachweise](CHECKPOINT_RECOVERY.md).

2026-10-15: **0.46.0 / persistente Checkpoints und Fortsetzen erreicht.** Vollständige aktuelle/Eintritts-Zustände, Szenen-/Formatbindung und atomare native Wiederherstellung; private geprüfte Szenenkopie, zwei atomare Speicher-Generationen, Autosave alle600Ticks/bei Zieländerung/Hintergrundwechsel, manuelle Speichern-/Laden-Tasten und Hauptmenü-Fortsetzen. Wiederherstellung startet pausiert; kaputte Checkpoints/Übergaben bieten vorherigen Checkpoint, Karteneinstieg bzw. ausdrücklich eigenständigen Start, Szenenwahl oder Abbruch. 25A/B/C-Zustands-/Grafik-/Fortlauf-/Neustartproben, vollständiger A/B/C→Fabrik-Lauf, Windows-ABI, getrennte Android-Native-Prozesse, echter APK-force-stop/Continue mit exaktem Zustand und beide sichtbaren Fehlerdialog-Wege, Java-Speicherkorruption/-Unterbrechung und1.120Bibliothekstests bestehen. [Bedienung und Grenzen](CHECKPOINTS.md).

**Stand nach0.46.0:** Fortschritt innerhalb der Karten ist persistent; vorhandene0.45.0-Szenen und Übergaben bleiben verwendbar. Ein abrupter Prozessabbruch kann Fortschritt seit dem letzten erfolgreichen Autosave verlieren. Laufende PCM-Wiedergabeposition wird nicht gespeichert; Checkpointformatrevisionen sind explizit gebunden. Physische ARM64-/manuelle Touch-Gesamtabnahme und bekannte Originalparität bleiben offen. Nächste Qualitätsarbeit kann Geräteabnahme/Erholung unter Speicherdruck oder Missionsdarstellung verfolgen; der anschließende Fabrik-Missionsablauf bleibt getrennt.

2026-10-15: **0.45.0 / Mission 1 bis zum tatsächlichen Kartenwechsel abgeschlossen.** Zusammenhängender A/B/C-Spielablauf (18.730/16.414/11.800 normale Eingaben), alle sieben C-Ziele, echter Reiseaufruf nach GEO_03A. Original-PlayerStart1/Actor8, 118.736 texturierte Dreiecke/184 statische Kollisionsquellen, Schema3 übernimmt65HP/14/65/vierGranaten und Scorch/Fixer/Sev. 360 Fabrikeingaben prüfen Bewegung/Sprung/zweiSchüsse/Nachladen/Ducken; gespeicherte Wiederaufnahme, zwölf fehlerhafte Carries, falsche Reiseziele und exakter Zustand/Grafik-Neustart geprüft. Windows-/Android-Gesamtlauf, APK/GPU/Touch/Audio, Java-Kampagnenverknüpfungen/96MiB-Dateileser und1.120Rusttests bestehen. [Bedienung und Abschlussgrenzen](FIRST_MISSION_COMPLETE.md).

**Stand nach0.45.0:** Mission1 ist funktional durchspielbar einschließlich C→GEO_03A. Nächste Arbeit: physische ARM64-/manuelle Touch-Gesamtabnahme und Originalparität/Qualität der ersten Mission; alternativ den folgenden Fabrikablauf integrieren. GEO_03A ist derzeit nur der geladene Eintrittsbereich mit Squad und Steuerung, keine ausgeführte zweite Mission. 4.715 transparente Effektflächen und verborgener DefaultProp-Lichtträger740 sind beim statischen Fabrikimport dokumentiert ausgelassen; vollständige native Effekt-/Mover-/KI-/Squad-/Markerparität bleibt offen.

2026-10-15: **0.44.0 / Fabrikaufzug und A/B/C-Missionsabschluss erreicht.** Originale Tür465/466 mit bewegter Geometrie/Hülle, Trigger468, freigegebener Trigger227 und Ausgang39 nach GEO_03A. Ziel6 durch tatsächlichen Spielereintritt bei erfüllten Zielen0–5, erfolgreichem Sun-Fac-Ausgang und offener Tür; Schema3 erhält Spieler sowie Scorch/Fixer/Sev. 11.800 normale C-Eingaben nach18.730A+16.414B erreichen alle sieben Ziele mit65HP/14/65,22Kills,einemRestgegner,192Audioereignissen. Exakte Wiederholung/Neustart/Grafik, Tür-/Squad-Taktung20/60/144FPS, Windows-/Android-Gesamtlauf samt Niederlagenregression, APK/GPU/Touch/Audio,96MiB-Dateileser und1.120Bibliothekstests geprüft. [Bedienung und Grenzen](MAP3_ELEVATOR.md).

**Nächster Meilenstein nach0.44.0:** GEO_03A als tatsächliche ladbare Zielszene importieren, Schema3 am ursprünglichen PlayerStart mit allen drei Squad-Mitgliedern anwenden und atomaren Kartenwechsel/gespeicherte Wiederaufnahme prüfen. Aktuell endet die erste Mission am Originalausgang mit gespeicherter Reiseanforderung; die Zielkarte wird noch nicht geladen. Original-KI-/Squad-/Marker-/Material-/Effektparität sowie physische ARM64-/manuelle Touch-Gesamtabnahme bleiben offen.

2026-10-15: **0.43.0 / Sun Facs Jäger, Beschuss und Absturz erreicht.** Sun Facs Aktionen 1023–1029, ausgewählte Erinnerungs-/Startketten 415/416, die vier Original-Jägerclips aus 319 und KillMe-Notify 130 sind angebunden. Sev erhält DC17mSniper, FireSniper und Originalton; sieben Schüsse im Fünf-Sekunden-Fenster mit begrenzter 0.8-s-Taktung. Erfolgsentscheidung über die tatsächliche Position, Wrack 382, ausgewählte Abschlussaktionen 287 bis zum Aufzugstracker; explizite Ziel-0-Zuordnung bei SunFacTerminated. Ohne Zuweisung folgen vier Erinnerungen, Entkommen und Missionsfehlschlag. 10.841 erfolgreiche / 10.526 fehlgeschlagene normale C-Eingaben nach echtem A/B-Abschluss, Erfolg mit 80 HP/9/84, 20 Abschüssen, drei Restgegnern und 166 Audioereignissen. Exakte Replays/Neustart, 20/60/144 FPS, vollständige Windows-/Android-Replays beider Ausgänge, APK/GPU/Touch/Audio, 96-MiB-Dateileser und 1.119 Bibliothekstests bestehen. [Bedienung und Grenzen](MAP3_FINALE.md).

**Aktuell nächster Meilenstein nach 0.43.0:** Aufzugstür, Zugang, ursprüngliche Auslöser und Ausgang 39 nach GEO_03A verbinden. Ziel 6 erst durch tatsächliche Aufzugsbenutzung abschließen und danach den vollständigen A/B/C-Missionsabschluss samt Zustandsübergabe abnehmen. Original-Flight-/Partikel-/Musik-/Grievous-, Ballistik-/Marker-/Squad-/KI-/Kletterparität bleiben offen. Ein Warrior erscheint begrenzt unterstützt über Spawnpunkt 488. Stunned nutzt native Jitterrechnung mit lokalem gespeichertem CRT-Strom; globaler Original-RNG und vollständige Jitter-Pose-Parität fehlen. Verlustfreie Base64-Audiometadaten unterstützen weiterhin alte Zahlenlisten; alte APKs benötigen für neue Szenen das Update. Physischer ARM64-/manueller Touch-Gesamtlauf bleibt ausstehend.


2026-10-14: **0.42.0 / AA-Raum, Schild und Sevs Scharfschützenposition erreicht**. AchtOriginalfabriken151/152/153/474/475/476/481/486, SunFac-Aktionen1016–1022 mitEmoteSeeEnemy45/30+DeathTaunt65/30, achtNotifies, unterstützterLauf zuPunkt33/Originalausrichtung, originaleSchildhülle412/Prop3 erstbeiActivateShieldaktiv/120Ticksdanachsichtbar. Original-HsFalloff-FallbackTextur stattbehaupteterShaderparität. ZweiOriginaldialoge, ausgewählte145/482Aktionen, physischeSevBereitschaft34/Zuweisung485 überKiste484.7.768C-Eingaben nachechtemA/B-Abschluss,80HP/9/84,20Kills,zweiRestgegner,150Audio/28Samples. Neustart/Replays/20/60/144FPS,Schildkollision/-verzögerung,Windows/Android/APK/GPU/Touch/Audio,96MiBDateileser,1.116Rusttests bestanden. [Bedienung und Grenzen](MAP3_BALCONY.md).

**Historischer nächster Meilenstein nach0.42.0:** SunFac-Aktionen1023–1029, SnipeReminder/StartSunFacRunning, eigentlicheSev-Schusssequenz undOriginaljägerablauf bisTakeOff/Absturz mitZielen5/6 umsetzen. DanachGesamtziel0,Aufzug/Ausgang39 nachGEO_03A undvollständigeA/B/C-Abnahme. SevsWaffe/Pose bleibtStandard, nativeMarker-Abbruch-/InkapazitätsregelnundKI-/Karma-Paritätfehlen. SunFacTeleport32 wirdfür115Körperhöhe~20EinheitenaufunterstütztenBodenprojiziert; nativeTeleport-/Kapselparitätnichtbehauptet. SchildFalloff/Pan/Tint/BlendundNotify-Audio/FXoffen. FrühereBacta-/Granaten-/Türschaden-/Markergrenzenbestehen;physischerARM64-/manuellerTouch-Gesamtlaufoffen.


2026-10-14: **0.41.0 / Bacta, Wachraum und Originalziel4 erreicht**. Originalstationen310/261, Squad-Marker122/Controller125 mit Positionen183/184/185, drei Clone-Türbrechclips und5/3/0Notifies integriert. Boden-/Rampen-/Stufenkontakt, Türflügel120/121 mit Original-KeyPos/0.5s, Originalskript140 und drei Originalsamples.7.081normale C-Eingaben, beideStationen, zwölfAbschüsse,95HP/12/103, zweiRestgegner,120Audioereignisse. Exakter Neustart/Wiederholung,20/60/144FPS, Heilungsgrenzen/-Eingabesperre, vollständiger Windows-/Android-A/B/C-Replay, APK/GPU/Touch/Audio und96MiB-Dateileser bestehen;1.116Bibliothekstests. [Bedienung und Grenzen](MAP3_BREACH.md).

**Historischer nächster Meilenstein nach0.41.0:** Sun Facs Folgekette nach SunFacRoomDoorOpened, AA-Raum-Fabriken, Scharfschützenmanöver und Ziele5/6 umsetzen. Danach Originalziel0 und Aufzug/Übergang nachGEO_03A zum vollständigen Missionsabschluss verbinden. BactaRate4 wird derzeit als4s/Vollheilung interpretiert; vollständige Aktivierung/FX/Audio, Granaten-/Ladungsobjekte, Türschaden/-FX/-Ersatzmesh und SpeechCues bleiben offen. Emittierte AA-Raum-Ereignisse führen den Folgeabschnitt noch nicht aus. Native KI-/Squad-/ReachSpec-/Karma-Parität und physischerARM64-/manuellerTouch-Gesamtlauf bleiben getrennte Grenzen.



2026-10-14: **0.37.0 / GEO_01C-Einstieg und Sev-Beitritt erreicht**. Originale153Frame-EntranceRope1760-/195Frame-SevIntro-/Warrior-/Seilclips, native Kanal-/Notify-/Root-Matrix-Funktionen und ausgewählte Aktionsketten103/98/99/101/264/292/293 integriert. Sevs Übergang zur Bodenphysik, Skins/Waffen,14Originalsamples/30Audioereignisse, Untertitel und Originalziel1/Checkpoint-Ereignis.1.800C-Eingaben nach echterB-Übergabe;20/60/144FPSzustandsgleich, gesperrte Eingaben ohne Zusatzbewegung/Verbrauch, exakter Zustands-/Grafikneustart. Vollständiger A/B→C-Einstieg unterWindows/Android undAPK/GPU/Touch/Audio/Pause bestehen;1.114Bibliothekstests. [Bedienung und Grenzen](MAP3_OPENING.md).

**Historischer nächster Abschnitt nach0.37.0:** Die fünf originalen SpawnPlanningRoom-Gegner, schädigbares Strategieraumglas/WindowDestroyed und Sun Facs Flucht samt Superkampfdroiden-Auftritt einbinden. Anschließend Türbrech-/Scharfschützenmanöver und Aufzug bisGEO_03A. Originalziel0 bleibt offen; Einstieg ist keine vollständige erste Mission. Überflug/Partikel/Pike/Ragdolls, vollständige RootMotion-/Kapsel-/Kampfparität und physischerARM64-Gesamtlauf bleiben sichtbar getrennte Grenzen.


2026-10-14: **0.36.0 / Kartenimport und B→C-Zustandsübergabe erreicht**. GEO_01C:81.748sichtbareDreiecke/74Kollisionsquellen, PlayerStart0, vollständige erfolgreiche Oberflächenbindungen im gewählten statischen Export. DrawType2-DefaultProp-Hilfsmeshes von Seilen/Endsymbol ausgeschlossen; fehlende kleine Original-BSP-Zelle mathematisch aus13Trennebenen ergänzt, keine Kollision ausgeschaltet. Schema2 übernimmt10HP,20/141Munition, vierGranaten und Scorch/Fixer100HP/100Schilde nach echtem18.730A+16.414B-Lauf.360C-Eingaben mit Bewegung/Sprung/Schießen/Nachladen/Ducken, atomare Fehlerfälle, gespeicherte Wiederaufnahme, exakter Neustart sowie Windows-/Android-/APK-Prüfungen bestehen.1.114Rust-Tests. [Dateien und Grenzen](MAP3_INTEGRATION.md).

**Historischer nächster Meilenstein nach0.36.0:** Den originalen Sun-Fac-Ablauf in GEO_01C integrieren: Einstieg/Abseilen und Sun-Fac-Inszenierung, Zero-Seven, Strategieraumglas, Superkampfdroide, Türbrech-/Scharfschützenmanöver und Fabrikaufzug.997Actors/Aktionen,47Aktionsketten,22Fabriken und sieben Ziele sind inventarisiert; Ausgang39 führt nachGEO_03A. Danach vollständigen A/B/C-Missionsabschluss und Android-Abnahme prüfen. Die aktuelle dritte Karte ist Erkundung; keine abgeschlossene erste Mission oder volle Original-KI/Karma-/Animationsparität behauptet.


2026-10-13: **0.35.0 / durchgehender GEO_01B-Lauf erreicht**. 16.414 normale Controller-Eingaben ab echtem GEO_01A-Übergang schließen alle acht Ziele und fünf Interaktionen ab: 10 HP, 20/141 Munition, 35 Abschüsse, Ausgangsanforderung GEO_01C. Physisches Scorch-/Fixer-Folgen über Originalknoten, Rampen-/Bodenkontakte und drei verzögert freigegebene Wrack-Hüllen korrigiert. 1.104 Bibliothekstests. [Lieferung, Nachweise und Grenzen](MAP2_CONTINUOUS_PLAYTHROUGH.md).

**Historischer nächster Meilenstein nach 0.35.0:** GEO_01C inventarisieren, Texturen/Geometrie/Kollision übernehmen, den tatsächlichen GEO_01B-Übergang samt Spieler-/Squad-Zustand einbinden und anschließend die originalen Ziele bis zum Ende der ersten Mission integrieren. Danach zusammenhängenden A/B/C-Lauf und Android-Abnahme durchführen. Volle Original-KI/Karma-/Animationsparität und physische ARM64-Abnahme bleiben getrennt sichtbar.

2026-10-13: **GEO_01B-Spielablauf 0.34.0 / Code38 integriert**. Acht Originalziele,31 ausgewählte einmalige Ereignisketten,29 Originaldialoge, Fixer-Template, fünf lokale Spreng-/Slice-Hosts,27 dynamisch freizugebende Akteure und42 originale Fabriken mit Kapazität/Intervall. Originalcounter916 verarbeitet vier Hallendroiden einmalig. Der Einstieg, alle Interaktionen/Fabriken, Abschlussanforderung GEO_01C und exakter Neustart sind gezielt geprüft. Spätere Prüfungen verwenden ausdrücklich Positions-/Gesundheitsfixtures; kein zusammenhängender GEO_01B-Lauf behauptet. [Dateien, Bedienung und Umfang](MAP2_GAMEPLAY.md). Nächster Meilenstein: durchgehenden GEO_01B-Weg mit regulären Ressourcen abnehmen und dabei Squad-Wege/Kampfinszenierung/Materialeffekte vervollständigen; danach GEO_01C samt B→C-Übergabe integrieren.

2026-10-13: **GEO_01B-Kartenintegration0.33.0 /Code37 erreicht**. Eigenständige32.810.668Byte-Szene, Original-PlayerStart,249.736Dreiecke/348Kollisionsquellen, Actor-Klassendefaults und ursprüngliche Tür-/Prop-Posen. Atomarer Wechsel nach echtem18.730Input-GEO_01A-Lauf; geprüfte Spieler-/Inventar-/Scorch-Übernahme und gespeicherte APK-Wiederaufnahme. Windows-/Android-ABI, GPU/Touch/Originalwaffenaudio/Pause,1.097Rust-Tests/Clippy/Format und Java48MiB-Dateileser bestehen. Lieferung `analysis/mission16-map2/delivery`; [Anleitung und Grenzen](MAP2_INTEGRATION.md). Nächster Meilenstein: GEO_01B-Skripte, Ziele, Türen/Terminals und erste originale Begegnungen. Die zweite Karte ist bisher Erkundung, die Mission über GEO_01A/B/C nicht vollständig.

2026-10-13: **Erste spielbare Karte bis zum Übergang /0.32.0 erreicht**. Durchgängige Ziele0–6 ab Landung bis Exit9, Live-Intro-Actors452/530 mit sechs zusätzlichen Originalsounds, korrigierte Platzierung und explizite Behandlung des fehlgeschlagenen dekorativen Wegauftrags. Einmalige geprüfte Übergabe an GEO_01B.ctm#playerstart, Eingabestopp und atomare Android-Zustandsaufnahme.18.730 normale Eingaben/20HP/acht Gesamtabschüsse; Squad-Kampflauf und Windows-/Android-ABI, APK/GPU/Touch/Audio/Pause bestanden.1.097 Rust-Bibliothekstests, Clippy/Format und Java-Pausenregression. Lieferung `analysis/mission15-complete/delivery`; [Anleitung, Nachweise und Paritätsgrenzen](FIRST_MISSION.md). Nächster funktionaler Meilenstein: GEO_01B-Szenenimport, PlayerStart und Zustandsübernahme. Native dekorative Weg-/GoalQueue-/WorldGeometry-Parität sowie weitere LZ-/Material-/Shaderdetails bleiben Qualitätsarbeit, keine abgeschlossene1:1-Rekonstruktion.

2026-10-13: **Intro-Warrior/Trooper in Arbeit**. Originale Actor-/Clip-/Stimmenbindungen und paralleler Script13/197-Ablauf vorbereitet; echte Warrior-AnimEnd-/Notify-Callbacks geprüft. Native Laufzeit-FindSpot-/CheckSlice-Steuerfolge portiert, acht gezielte Tests; Originalziel895 wird im vorhandenen Diagnosehost um etwa9,895Einheiten angehoben und danach freigegeben. Native Abfragefilter/FarMoveActor und WaitForLanding→Goto-Ausführung sind noch offene Hostgrenzen. Keine neue APK: spielbar bleibt0.31.0. [Belege, Ergebnisse und nächste Integration](INTRO_ACTORS.md).

2026-10-13: **Live-Abseil-Meilenstein0.31.0 / Code35 erreicht**. Originaler EntranceRope600-Kanalstart/LOD-Tick/Notifies/AnimEnd, relative Root-Matrizen, Spieleransicht im Gunship, begrenztes Schauen, Seil7/Originalzerstörung und vier neue Foley-Samples.1.925Auftakt-Ticks,18.730normale Missionsinputs bis Turmausgang;16.757Inputs im Scorch-Kampflauf. Lieferung `analysis/mission13-rappel/delivery`; [Bedienung, Originalbindung und Portgrenzen](RAPPEL_RECONSTRUCTION.md). Nächste Inszenierung: Intro-Warrior530/Trooper452 und weitere LZ-Lebenszyklen; Material-Fußschritte/volle Kollisions-/Kameraparität bleiben offen. Anschließend GEO_01B-Laden/Zustandsübernahme und Gesamtmission-Abnahme.

2026-10-13: **Native RootMotion-Bausteine rekonstruiert**. Relative Kanalmatrizen und physRootMotion-Reihenfolge in Rust;141Originalframes bitgleich zum Original-SSE und in der Originalwelt körpergeprüft. Bisheriger Missionslauf unverändert. Nächste Arbeit: PlayAnim-Einblendung/Kanallebensdauer, PlayerStand/Bodenübergang und Live-Kamera/Foley. [Belege und Grenzen](RAPPEL_RECONSTRUCTION.md); spielbare Lieferung weiterhin0.30.0.

2026-10-13: **Abseil-Originalassets vorbereitet**. EntranceRope600 vollständig abgetastet; Seil, Intro-Trooper452 und Warrior530 samt Notifies/Posen aufgelöst. Native Pawn-RootMotion-/Kamerabindung bleibt nächste konkrete Arbeit. [Befunde und Integrationsgrenzen](RAPPEL_RECONSTRUCTION.md); vorbereitete Daten unter `analysis/mission13-rappel`, spielbare Lieferung weiterhin0.30.0.

2026-10-13: **Missionsauftakt0.30.0 integriert**. Ausgewählte Originalscripts12/523/497 mit Landungs-/Sun-Fac-Briefing, zeitgesteuertem175-Schaden, Bacta-Funkhinweisen und Ziel-Freigabe; Gunship580 nutzt originale Intro-/Loop-Posen vor dem bestehenden Artillerieauftritt.1.925Auftakt-Ticks +16.640bestehende Eingaben bis zum Turmeingang. Lieferung `analysis/mission12-opening`; [Originalbindung und konkrete Grenzen](MISSION_STAGING.md). Als Nächstes originale Abseilkamera/EntranceRope600 und Intro-Warrior, danach LZ-Trooper/Überflug/Rückzug. Vollständige Einleitung und Kartenwechsel bleiben offen.

2026-10-13: **Wiederbelebung0.29.1 vervollständigt**. Originale f32-Gesundheitsrampe während GetUp, saubere Idle-Rückkehr und einmaliger HealStop-Abschluss. Identische Gesundheitsverläufe im Windows-/Android-Rust-Fixture, Missions-/Squad-Regressionsläufe und neue APK unter `analysis/mission11-revival`. [Originalregeln und Grenzen](REVIVAL_PARITY.md). Nächste Arbeit bleibt die Inventarisierung der fehlenden geo_01a-Inszenierung und der Kartenwechsel mit Zustandsübernahme; allgemeine Original-KI-/Squad-Parität ist weiterhin offen.

2026-10-12: **Gegner-/Scorch-Spielablauf0.29.0 integriert**. Originalgroße Lunge-/JumpDest-Bewegung, gegenseitige Zielwahl, DC17m mit Projektilen/Originalsound, Folgen/Halten/Fokus und gegenseitige Wiederbelebung. Eingabebasierter Missionslauf sowie Scorch-Fokusfeuerlauf und isolierte Fehlerfälle geprüft. Lieferung: `analysis/mission10-squad`; [Bedienung, Belege und konkrete Paritätsgrenzen](ENEMIES_AND_SQUAD.md). Nächster Meilenstein: verbleibende geo_01a-Inszenierung inventarisieren und vollständige erste Mission über Kartenwechsel/Zustandsübernahme vorbereiten; danach Gesamtmission-/Touch-/ARM64-Abnahme. Allgemeine Original-KI-/Squad-Parität bleibt offen.

2026-10-12: **Warrior-KI 0.28.0 integriert**. Wahrnehmung, letzte sichtbare Position, körpergeprüfte Wegsuche über78 Originalknoten/208 gerichtete Verbindungen, originales Nahkampf-Goal/BurstCount und MeleeHit-Notifies ersetzen den bisherigen direkten5-Schaden-Angriff. Verfolgung, verzögerte Niederlage, Ausweichen und Schussunterbrechung durch normale Eingaben geprüft;1.067 Bibliothekstests. APK und RCPLAY11-Szene: `analysis/mission9-ai`. [Bedienung, Originalregeln und Grenzen](WARRIOR_AI_MISSION.md). Missionsgebundene Flug-/Sprung-/Lunge-Integration und allgemeine Squad-KI bleiben nächste Arbeit.

2026-10-12: **Artillerieabschnitt 0.27.0 erreicht**. Originale Gunship-/Spider-Droid-Animationen, vier schädigende Artilleriegeschosse und drei Warrior-Fabrikgegner sind im zusammenhängenden Abschnitt0–6 integriert. 16.648 Eingaben bis Sieg mit20HP/acht Abschüssen; Beschuss-Niederlage separat geprüft, 1.064 Bibliothekstests. APK und RCPLAY10-Szene: `analysis/mission8-artillery`. [Bedienung, Belege und offene Originalparität](ARTILLERY_MISSION.md). Vollständige erste Mission, allgemeine KI-/Squad-Parität und Laden von GEO_01B bleiben offen.

2026-10-12: **Turmzugang 0.26.0 erreicht**. Die ausgewählten Originalziele 0–6 sind zusammenhängend spielbar: Conversation05, Artilleriefunk, zeitgesteuerte Barrikadenfreigabe, originale Türposen/-geräusche und Ausgangsauslöser mit Reiseanforderung GEO_01B. 14.746 normale Eingaben bis Sieg, 20 HP/fünf Abschüsse, exakter Neustart; 1.060 Bibliothekstests. APK und Szene: `analysis/mission7-spire`. [Bedienung, Nachweise und Grenzen](SPIRE_MISSION.md). Der Kartenwechsel lädt GEO_01B noch nicht. Nächster Meilenstein: fehlende Artillerie-/Fabrik-Kämpfe und Inszenierung von geo_01a vor der Gesamtmission-Abnahme integrieren.

2026-10-12: **Sprengauftrag 0.25.0 erreicht**. Der ausgewählte Auftrag „Command demolition maneuver“ ist live integriert: normaler Lauf über Originalziele 0–5, Scorch/Setzclips, Zünder/Hände/Notify-Audio, BunkerBlown2 mit tatsächlichem Durchgang und exaktem Neustart. 13.397 normale Eingaben, 20 HP/fünf Abschüsse/Sieg; Windows- und Android-x86_64-ABI sowie APK/GPU/Touch-Benutzen/Schießen/Ducken bestehen. 1.055 Bibliothekstests. Der Android-Dateileser vermeidet die Speicherverdopplung großer Szenen. Lieferung `analysis/mission6-playable`; [Anleitung und offene Originalfälle](DEMOLITION_MISSION.md).

Historischer nächster Meilenstein nach 0.25.0: `Conversation05` und Originalziel 6 „Infiltrate Geonosian spire“ mit Anschlussweg durch den geöffneten Bunker rekonstruieren und integrieren. Vor der Gesamtmission-Abnahme zusätzlich Warrior-/Fabrikinszenierung, Tutorial 3552–3555, Hologramm-/große Explosionsmaterialien und begrenzte Squad-/Kampfregeln abgleichen. Vollständiger Touch-Durchlauf und physische ARM64-Abnahme der neuen Version sind weiterhin offen.

2026-10-12: **Meilenstein Geschützstellung erreicht / 0.23.0**. Zusammenhängend spielbare Originalziele0–3: Bacta, Absturzstelle, Granaten und Geschützstellung mit texturiertem Bunkerumbau, Explosionsauftritt/-audio, Originalbestätigung und Abschnittssieg. 1036 Bibliothekstests; Rust-/native- und Android-x86_64-Eingabelauf bis Sieg. APK-Dateidialog, GPU, Touch-Schießen, Originalwaffensound und Pause aufAndroid geprüft; vollständiger Touch-Lauf und physische ARM64-Abnahme dieser Version offen. APK und Szene: analysis/mission4-complete. [Starten, Nachweise, Grenzen und nächster Meilenstein](GUN_POSITION_MISSION.md). Die gesamte erste Originalmission bleibt in Arbeit.

2026-10-12: **0.22.0 / Live-Wurf integriert**. Vom Bacta-Einstieg durchgängiger Game- und native ABI-Lauf bis GranadeProj-Kontakt621. Notify35/Ende57, echte Waffensperre, fünf Verbräuche, Foley/Bounce/Leer, Welt-Tiefe, Pause und Neustart geprüft; 1033 Bibliothekstests. RCPLAY05 samt ARM64/x86_64-APK unter analysis/mission4-live-throw. Nächster Abschnitt: sichtbarer Bunker-/Hatch-/Kollisionsumbau, thermaler Explosionsauftritt/Schadensauswahl, Scriptbestätigung/Abschluss; danach Android. [Nachweise und Grenzen](LIVE_GRENADE_THROW.md).

2026-10-12: **Weltflug und Arena-Texturen geprüft**. Reale kontinuierliche Pawn-Route ab Pickup610, Originalreleaseoffset und kollisionsbegrenzter Flug mit Abprall anProp4 erreichen Trigger621. Dry-Buoyancy/Volumen und begrenzter Ruheadapter implementiert; 1031 Bibliothekstests. Neue kompatible Szenenkorrektur `analysis/mission4-world-flight/geo_01a-grenades-arena.rcplay` beseitigt isolierte Lichttexturen auf13.743Flächen; vollständiger Abschnitt0/1/2 samt nativer ABI besteht. Nächste Arbeit: Game-/JNI-/Touch-Wurf, Originalaudio und Bunkerzustände mit optionaler Missionsfortsetzung, dann Android. [Belege und konkrete Grenzen](DETONATOR_WORLD_FLIGHT.md).

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


## Originalziel4: Scorch-Begegnung, 2026-10-12

2026-10-12: **Originalziel4 – Scorch-Begegnung vorbereitet**. 40 Originalaktionen, acht PCM-Aufnahmen und begrenzter Ereignis-/Dialoghost; sieben neue Verhaltenstests, insgesamt1043 Bibliothekstests. Originaldialoge bei20/60/144Hz geprüft. Anschlussweg erreicht Trigger802, besteht den Gehgraphen zu957 noch nicht; Mesh-/Bodenprüfung an BSPTrim/Slab folgt. Noch keine neue APK/Szene; spielbar bleibt0.23.0. [Befunde, Blocker und nächste Integration](RENDEZVOUS_FLOW.md).

## Scorch-Begegnung erreicht, 2026-10-12

2026-10-12: **Scorch-Begegnung spielbar / 0.24.0**. Originalziele0–4 führen jetzt durch den niedrigen Trümmerdurchgang bis zum Treffen mit Six-Two. Ducken mit Originalmaßen, gelbe Scorch-Textur, Originalanimationen, zwei kollisionsgeprüfte Begleiterwege,13 zusätzliche Samples und Squad-HUD sind integriert. Zusammenhängender Rust-/nativer und Android-x86_64-Eingabelauf bis Ziel4; APK/GPU, Touch-Schießen/Ducken und Pause geprüft. APK und RCPLAY07-Szene: analysis/mission5-playable. [Starten, Nachweise und Grenzen](SCORCH_RENDEZVOUS.md). Als Nächstes folgt der Squad-Sprengbefehl.

Nächster Meilenstein: Originalziel5 „Command demolition maneuver“. Squad-Befehl auf Marker466/Charge465, Scorchs Weg und Original-SetCharge-Animationen, Bewaffnen/Explosion und Prop12-Durchgang integrieren; Original-SpawnWarriorAttack/SpawnDemoDroids mit ihren zugehörigen Kampf-/Freigabebedingungen prüfen. Danach den gesamten Eingabelauf0–5 und Android abnehmen. Allgemeines Squad-Folgen/Combat und noch protokollierte Schlachtfeld-/Intro-Fabriken bleiben ausdrücklich offen.

2026-10-12: **Ziel5 – Sprengablauf vorbereitet**.65 Originalactor-/Action-Einträge,24 ausgewählte Aktionen und neun PCM-Sounds aufgelöst. Begrenzter Marker-/Charge-Host mit zehn Tests, sechs Originaldatenläufen bei20/60/144Hz,15 Textur-/Poseproben. Nach akzeptiertem10448-Eingabe-Präfix erreicht ein separater physischer Begleitercontroller Marker466; BunkerBlown2 mit23 Originalhüllen vorbereitet. Zünderauswahl innerhalb192Einheiten ist vom späteren Auslösen getrennt, damit Rückzug möglich bleibt. Noch keine neue APK oder integrierte Szene; spielbar bleibt0.24.0. Nächster Schritt: Live-Squad-Befehl, Setz-/Zünder-Controller, dynamischer Prop12-Tausch und Kampf-/Freigabebedingungen. [Befunde, Grenzen und Reproduktion](DEMOLITION_FLOW.md).


2026-10-14: **0.38.0 – GEO_01C-Planungsraum**. Fünf Originalfabriken, begrenzter Kampf/Squad-Unterstützung, Glas263 mit150HP und WindowDestroyed/Originalziel2 integriert. Durchgehender Eingabelauf bis fünf besiegte Gegner; ausB übernommene10HP bleiben erhalten. Sun Facs Bewegung, Tür139/Flügel134–135, ActivateSBDIntro und SBD-Auftritt bilden den nächsten Abschnitt. [Anleitung, Nachweise und Grenzen](MAP3_PLANNING.md).


2026-10-14: **0.39.0 – Sun Facs Flucht durch die Wachtür**. Originalskript193 bis zur Wartegrenze vorSunFacRoomDoorOpened:86körpergeprüfte Laufpunkte,450Geschwindigkeit, Türflügel134/135 mit ursprünglichen KeyPos/MoveTimes, Verriegelung,180Ticks warten, originaler Teleport nach32 und Nähetrigger144. 3558normale C-Eingaben, exakter Neustart/Replay und20/60/144FPS geprüft. Nächster Abschnitt: Skript147, Figuren321–323/IntroSBD-Clips, acht Wachraumfabriken, SBD-Kampf/SBDDestroyed/Ziel3. [Anleitung und Grenzen](MAP3_ESCAPE.md).


2026-10-14: **0.40.0 – SBD-Auftritt und Ziel3**. Skripte147/358, Figuren321–323 mit90/210/150Originalframes,15Notifies, acht Wachraumfabriken und Intro-SBD400HP integriert. BENUTZEN markiert sichtbare Gegner für priorisierte Squad-Unterstützung.4653normaleC-Eingaben bis SBDDestroyed/Ziel3 undBreachDoorSunFac-Tracker;5HP,8/122Munition,6Abschüsse,8weitereWachraumgegner verbleiben. NächsterAbschnitt: Original-Bactastationen310/261, durchgehenderWachraumweg undTürbrechmanöver/Ziel4; danachScharfschützenabschnitt/SunFacEnde/Aufzug. [Nachweise und Grenzen](MAP3_GUARD.md).
