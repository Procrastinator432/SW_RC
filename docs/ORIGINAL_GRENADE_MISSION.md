# Originaler Abschnitt bis zur Granatenaufnahme – 0.21.0

Die Szene verbindet die Originalziele 0 (Bacta), 1 (Absturzstelle sichern) und 2 (Granaten aufnehmen) in `geo_01a`. Beide Dateien aus `analysis/mission3-playable` ersetzen die bisherige Lieferung:

- `republic-commando-0.21.0.apk` installieren (ARM64 und x86_64, VersionCode 24).
- `geo_01a-grenades.rcplay` auf das Gerät kopieren und über **Spielbare Szene öffnen** auswählen.

Die Szene ist separat erforderlich; die APK enthält keine Originalspielpakete. Die Steuerung bleibt wie bisher: linker Joystick bewegen, rechts ziehen zum Zielen, Schießen/Nachladen, an der Bacta-Station Benutzen halten. Dem Zielanzeiger folgen. Nach fünf besiegten Droiden geht es durch den freigegebenen Durchgang und um die Ecke zur Barrikade. Während der ursprünglichen Funkzeilen kurz warten; die Barrikade wird anschließend freigesprengt. Zur Granatenkiste laufen: Der Vorrat steigt von 0 auf 5 Thermaldetonatoren. Nach der Bestätigung erscheint der Abschnittsabschluss. **Neustart** setzt Ziele, Kollisionen, Gegner und Inventar zurück.

## Integration

Scripts 745/747 steuern Auftrag und Aufnahme mit Originaldefaults, getrennten Kontaktzylindern und tatsächlichen PCM-Dauern. Der Auftragskontakt allein gewährt keine Granaten; der Abschnitt endet erst mit gefülltem Inventar und abgeschlossenen Dialogzeiten. Originalziel 3 (Geschütz zerstören) wird noch nicht gestartet. Ein Granatenwurf ist noch nicht implementiert.

Script 911 löst nach `CrashSiteSecured` und 0,5 Sekunden `KnockOverWall` aus. Die vier ursprünglichen Karma-Wände 909/910/912/913 wechseln in eine berechnete umgekippte Ruhepose; Texturen und Kollisionshüllen verwenden dieselbe Transformation. Das ist ein begrenzter Ersatz für Karma, ohne vollständige Dynamik oder Fallanimation. Aus Script 40 werden die relevanten Kontakte, Wartezeiten, Funkzeilen und die Zerstörung von BarricadeWall/Prop3 (964) ausgeführt. Originale Trooper-/Droid-Inszenierung, sämtliche Effekte und eine allgemeine UnrealScript-VM sind damit nicht rekonstruiert.

Der ursprüngliche Weg verläuft unter einer niedrigen Decke. Die Bodenwahl prüft nun zuerst den Bereich am tatsächlichen Wegknoten; die bisherige höher angesetzte Probe hatte fälschlich das Dach gewählt. Der zusammenhängende Lauf benötigt keine Positionssprünge oder entfernten statischen Hindernisse. Nur die jeweiligen Skriptereignisse ändern dynamische Wandzustände.

Die Kiste besitzt 402 texturierte Dreiecke. Acht zusätzliche Originalsamples ergänzen die bisherigen vier Funkzeilen und das DC-15s-Schussgeräusch: Granatenauftrag, Anleitung, Bestätigung, Aufnahme sowie zwei Barrikaden-Funkzeilen und zwei Explosionsgeräusche. Die korrigierten BSP-/Mesh-Texturen und UV-Kanäle der Lieferung 0.20.1 bleiben erhalten. Vollständige Beleuchtung, Materialeffekte und ursprüngliche Kampf-/KI-Parität bleiben offen.

## Nachweise und Wiederholung

`analysis/mission3-playable/check`, `abi-validation.json`, `android-gpu`, `android-cpu` und `graphics` enthalten die Abnahme. Der Spielkernlauf prüft fünf Abschüsse, fünf Granaten, alle 13 Soundrollen, dynamische Kollisionen und exakten Neustart. Android läuft über normalen Dateiauswahldialog und Touch-Steuerung; Positionsdaten werden nur gelesen. GPU-/CPU-Darstellung und Grafik-Kontextwiederherstellung werden separat geprüft. Alte Bacta-/Absturzstellen-Szenen und der 1002-Eingaben-Replay bleiben Regressionen. ARM64 ist gebaut; diese Version benötigt noch den Test auf dem physischen Gerät.

Reproduzierbare Werkzeuge: `rc-grenade-route-check`, `rc-grenade-build`, `rc-grenade-scene-check`, `scripts/Validate-GrenadeSceneAbi.py`, `scripts/Test-OriginalGrenades.py` und `scripts/Summarize-GrenadeMission.py`. `integration-receipt.json` dokumentiert Originalbindungen und Wandtransformationen; `validation-manifest.json` bindet APK, Szene, Originaldateien, Nachweise und `source-snapshot.zip` über SHA-256.

Nächster Abschnitt: Originalziel 3 mit Granatenwurf, Detonation, Schaden und zerstörbarer Geschützstellung. Die historische Vorbereitung unter `analysis/mission3-pickup` bleibt unverändert nachvollziehbar.
