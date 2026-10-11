# Originalabschnitt geo_01a: Bacta und Absturzstelle

Aktuelle Korrektur **0.20.1**: Boden-/Mesh-Texturen und ursprünglicher DC-15s-Schuss-Sound sind ergänzt. APK und Szene unter `analysis/mission2-av`. [Ursachen, Starten und Prüfungen](TEXTURES_AND_WEAPON_AUDIO.md). Die folgenden Angaben dokumentieren den vorausgehenden Stand.

Lieferung **0.20.0-original-crash-site / Code 22**, Verzeichnis `analysis/mission2-playable`.

## Auf Android starten

1. `republic-commando-0.20.0.apk` installieren bzw. die bestehende Diagnose-App aktualisieren.
2. **Zusätzlich** `geo_01a-crash.rcplay` auf das Gerät kopieren, etwa nach Downloads. Die Szene enthält ihre vorbereiteten Originaldaten; eine separate Steam-Installation auf dem Gerät wird nicht benötigt.
3. App öffnen → **SPIELABSCHNITT GEO_01A ÖFFNEN** → **SZENE ÖFFNEN** → diese Szene auswählen.
4. Links bewegen, rechts Kamera drehen. Der eingeblendeten Wegführung zur Bacta-Station folgen. In Reichweite auf die Station schauen und **BENUTZEN gedrückt halten**, bis 100 Lebenspunkte erreicht sind. Loslassen unterbricht die Heilung.
5. Den Advisor und die Freigabe der Barrikade abwarten. Der Wegführung zur Absturzstelle folgen. Die fünf Droiden sind zunächst geschützt; bei „weiter vorrücken“ dem Weg folgen, bis der ursprüngliche Dialog-Auslöser erreicht ist.
6. Sichtbare Droiden ausschalten. **FEUER** halten; bei Bedarf **NACHLADEN**. Nach fünf bestätigten Abschüssen erscheint „Originalziele abgeschlossen: Bacta und Absturzstelle gesichert“. **NEUSTART** setzt Heilung, Gegner und Sperren zurück.

Die bisherige Szene `analysis/mission1/geo_01a-landing.rcplay` bleibt als kürzerer Bacta-Abschnitt erhalten.

## Was dieser Abschnitt aus dem Original übernimmt

- Zusammenhängende ursprüngliche Ziele 0 „Use bacta to heal“ und 1 „Secure area around crash“.
- Bacta-Station und vorbereiteter Einstieg nach der Einleitung, sieben Navigationspunkte zum Heilen und 13 weitere Wegpunkte über ursprüngliche Tracker-/Trigger-Anker.
- Parallele ursprüngliche Bacta-/Spawn-Ereignisfolgen, Advisor-Dauer und Wartezeiten. Vier originale Advisor-Samples.
- Sperren 1122 und 915 sowie zerstörbare Barrikade 617. Ihre Kollisionskörper werden tatsächlich freigegeben; die sichtbare Barrikade verschwindet aus dem dynamischen Renderpaket. Neustart stellt beide Zustände wieder her.
- Fabriken 133, 134, 277, 441 und 836, BattleDroid mit 75 Lebenspunkten, ursprüngliche Spawn-Anker und Schutzwechsel durch die Kontakt-Auslöser 128/608.
- Fünf Todesmeldungen an den ursprünglichen Zähler 620 → `CrashSiteSecured` → Ziel 1 abgeschlossen und Ausgangssperre freigegeben.
- BattleDroid- und DC-15s-Modelle mit vorbereiteten Originalsequenzen, einschließlich des Droiden-Trefferclips `Impact`.

## Grenzen

Dies ist ein begrenzter Originalmissionsabschnitt. Gegnerbewegung, Schusskadenz, Trefferrechnung und Schaden verwenden noch den vorläufigen Kampfhost. BattleDroid-KI, DC-15s-Waffenregeln, Projektilrechnung, Schilde und Waffen-Audio sind damit nicht originalgetreu rekonstruiert. Erneutes Heilen an Bacta nach Abschluss des ersten Ziels ist noch nicht integriert. Die vorbereiteten Modelle verwenden angepasste Darstellung und Kollisionsabmessungen; die Fabrikpositionen werden auf den begehbaren Boden gesetzt.

Einleitung, Abseilen und Anfangsverwundung sind vorbereitet. Dekorative Klone, Rückzug/Explosionen und weitere Gunship-Skripte werden nicht allgemein ausgeführt. Teile der Umgebung besitzen noch fehlende Geometrie oder vereinfachte Materialdarstellung; die begehbare Kollision ist separat geprüft. Die gesamte erste Mission sowie eine allgemeine UnrealScript-VM sind weiter offen. Originaldaten bleiben lokale, aus der vorhandenen Installation erzeugte Assets.

## Prüfnachweise

Unter `analysis/mission2-playable`: `check/validation.json` für den physischen Weg, Heilung, fünf Fabriken, Schüsse, ursprünglichen Zähler, dynamische Darstellung und exakten Neustart; `abi-validation.json` für denselben Ablauf über die native Schnittstelle einschließlich kurzer Puffer und Audioverbrauch. `workspace-tests.log` enthält 988 erfolgreiche Bibliothekstests. Vorheriger Bacta-Abschnitt und 1002 bestehende Replay-Kommandos bestehen unverändert.

Android-Abnahme wird über normale Dateiauswahl und Touch-Eingaben ausgeführt. Opt-in-Telemetrie liest Positionen und Gegnerzustände zur Teststeuerung; sie verändert keine Spielfigur, Lebenspunkte, Gegner oder Ziele. GPU- und CPU-Touch-Durchläufe bestehen bis zum Abschluss beider Ziele einschließlich fünf Abschüssen, vier Advisor-Samples und Neustart. Der Grafiktest besteht mit 0,485 mittlerem Kanalfehler und 99,36 % der Kanäle innerhalb zweier Stufen; Kontextneubau erhält Bild und Position exakt. Dreimaliges Neuladen besteht die begrenzte Speicherprüfung. Diese Belege sind mit APK, Szene, Originaldateien und Quellenarchiv im `validation-manifest.json` gebunden. Die Abnahme dieser Version auf einem physischen ARM64-Gerät bleibt ausstehend.

## Nächster Meilenstein

Den ursprünglichen Granatenauftrag (Ziel 2) aus Karte und Skripten extrahieren und als nächsten zusammenhängenden Abschnitt anbinden. Parallel die vorläufigen BattleDroid-/DC-15s-Kampfregeln durch belegte Originalregeln ersetzen und die fehlende Umgebungsdarstellung ergänzen. Danach Geschützstellung, Six-Two, Sprengkommando und Turmeingang; die vollständige Mission gilt erst nach diesen Abschnitten und Geräteabnahme als abgeschlossen.
