# Dialogverwaltung für Karte B

Stand: 04.11.2026 · **0.87.0-map2-voices / Code 91**.

Die 29 ausgewählten Dialogaktionen von GEO_01B verwenden jetzt ihre belegte Sprachkategorie und Audioquellenidentität. Bisher verließen sie die native Schnittstelle als allgemeine Effekte mit Besitzer `-1`. Jetzt erreichen sie die vorhandene Dialogverwaltung und damit deren separate Quellenverwaltung, Musikabsenkung und optionale gespeicherte Wiedergabefortsetzung.

## Lieferung und Verwendung

- [APK 0.87 für ARM64 und x86_64](../build/map2-voices/android/artifacts/republic-commando-0.87.0-map2-voices.apk).
- [Neues Szenenpaket](../build/map2-voices/central-package/package/mission1-scenes.rcpack.zip).
- [Prüfzusammenfassung](../build/map2-voices/verification-summary.json) und [gesicherter Quellstand](../build/map2-voices/source-and-verification.json).

**APK aktualisieren und das neue Paket importieren.** Die neuen B-Bindungen gelten bei frischem Beginn oder beim Übergang aus A nach B. Bereits gespeicherte B-Szenen behalten ihre bisherigen Metadaten; sie werden nicht automatisch auf eine andere Szene umgeschrieben. Vorhandene Spielstände bleiben ladbar.

Die Korrekturen aus 0.86 für Scorch am A-Bunker und die B-Kollisionsspitzen bleiben enthalten. Die positiv am Handy getestete Musik auf A/B und die Texturen von A/am Anfang von B bleiben erhalten.

## Belegte Zuordnung

`rc-map2-voice-audit` liest die ausgewählten `ACTION_PlaySound`-Exporte aus der installierten `Maps/geo_01b.ctm` sowie ihre Klassenwerte und die ursprünglichen AudioParams. Es prüft jede Soundreferenz gegen die tatsächlich eingebundene Soundbank. Alle 29 Aktionen ergeben Kategorie **Dialog (1)** und `bDontOverrideOtherSounds=false`.

Die Identität folgt zunächst dem ursprünglichen AudioParams-ActorTag, danach einem ausdrücklichen SoundSourceTag und andernfalls dem ursprünglichen Skriptkontext. Für einen ScriptedTrigger ist das dessen Exportidentität; für die ausgewählte ScriptedSequence wird der eindeutig zugeordnete Pawn über AIScriptTag aufgelöst. So sind unter anderem die belegten Commando62-/Commando40-Anker an die B-Pawns 11 und 10 gebunden. Es wird keine Sprecheridentität aus dem Dateinamen geraten. Eine nicht eindeutig auflösbare Bindung bricht den Export ab. [Originalbefunde je Aktion](../build/map2-voices/central-package/assets/validation.json).

Die optionale `sound_policies`-Liste gehört zu den Kampagnenkarten-Metadaten. Alte Szenen ohne diese Liste behalten ihr bisheriges Verhalten. Ungültige Bankindizes, doppelte Rollen, ungültige Kategorien und fehlende positive Besitzer werden beim Laden abgewiesen. Ein bereits ausdrücklich übermittelter anderer Besitzer wird nicht überschrieben. RCAUD001/002/003 führen dieselbe aufgelöste Identität; die neueren Pakete enthalten außerdem Kategorie und Schutzflag.

Sounds, PCM, ursprüngliche Soundbankindizes, Skriptwartezeiten, Bewegungs- und Arbeitslogik, Checkpointformat, Geometrie, Kollision und Musikprogramme bleiben gleich. Das neue Paket ergänzt ausschließlich die 29 B-Policies. A, C und der Fabrikeinstieg bleiben bytegleich. Waffen und Arbeitsgeräusche bleiben Effekte.

## Prüfungen

- Je **1.583 Debug-/Release-Bibliothekstests**, Clippy, Formatierung, Java-/Buildvertrag und beide Android-ABIs bestehen. Zwei neue Testgruppen sichern die Besitzergrenzen und die Zurückweisung ungültiger Bindungen ab.
- Der gepaarte B-Eingabelauf vergleicht **17.782 vollständige Spielzustände** exakt; fünf kalte Restores und 17 vorhandene Arbeitsansichten sind bildgleich. Die Szenenbindung älterer Teststände wird dabei ausschließlich im Prüfer ausdrücklich umgesetzt, nicht im Produkt gelockert.
- Der native Vergleich prüft alle 29 tatsächlich emittierten Dialogereignisse einschließlich Besitzer, Kategorie und Schutzflag. Sequenz, Tick, Soundindex und Wiedergabeparameter bleiben gleich; alle anderen Audioereignisse sind identisch. Die PCM-Bank bleibt bytegleich.
- Echte B-Dialogpakete prüfen im OpenAL-Pfad die Zugehörigkeit zum Dialogpool, Ersatz einer alten Zeile derselben Quelle und erneute Ausgabe nach kalter Audiowiederherstellung. Der kontrollierte Überlagerungstest normalisiert nur Sequenz und Tick der zuvor tatsächlich beobachteten Pakete.
- Der vollständige native A→B→C→Fabrik-Lauf mit **50.031 Eingaben, 1.278 Audioereignissen, drei Übergängen und vier kalten Restores** besteht; alle 29 B-Dialogmetadaten stimmen mit den Originalbefunden überein.
- Android prüft die erste originale B-Zeile als aktive Dialogquelle einschließlich Pausenspeicherung und größerem Wiedergabeoffset nach Prozessneustart. Die gespeicherten Mixerzustände bestätigen während dieser Zeile die Musikabsenkung auf Faktor 0,5 und die Effektabsenkung auf 0,25. Die vier bisherigen B-Ladungs-/Konsolenfälle sowie B/C-Musik werden mit dem neuen Paket erneut geprüft. Einzelbelege und Bildvergleich stehen im Gesamtnachweis.

## Reproduktion

```powershell
./build.ps1 -Target Map2Voices `
  -CampaignPackage build/work-equipment/package/mission1-scenes.rcpack.zip `
  -GameData 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' `
  -Python python -OutputDirectory build/map2-voices-rebuild
```

Die weiteren Prüfer sind `rc-map2-voice-check`, `scripts/Test-Map2Voices.py`, `scripts/Test-MissionNativeAudio.py --soundtrack` und `scripts/Test-MusicAndroid.py --map2-voice-checkpoint …`.

## Grenzen und nächste Schritte

Das ist der Anschluss der ausgewählten B-Dialoge an die vorhandene Audioverwaltung. Originale Action-Wartezeiten, globale SoundMultiple-Auswahl und vollständige räumliche Audioparität werden nicht neu rekonstruiert. Verschiedene ursprüngliche Quellen dürfen weiterhin gleichzeitig sprechen; eine generelle Serialisierung aller Stimmen würde die Originalregeln verändern. Ein bestimmter hörbarer Überlagerungsfehler muss deshalb anhand seines konkreten Ablaufs geprüft werden.

Die freie Hör-/Pegelabnahme am Handy bleibt offen. Auch die gesamte Karte B gilt weiterhin nicht als abgenommen: Es folgen Einstieg und Fixer-Beitritt, die weiteren Kämpfe und Squadwege sowie Türen/Ziele bis C mit Speichern/Fortsetzen. Die jetzige Änderung erklärt diese übrigen Systeme nicht als fertig.
