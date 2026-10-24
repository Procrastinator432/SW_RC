# Mission 1: Musik, Sprachverständlichkeit und Fortsetzen

Stand: 27.10.2026 · Lieferung **0.72.0-mission-soundtrack**. Dieser Stand erweitert die spielbare Mission; eine vollständig originalgetreue iMUSE-/BattleEngine und eine aktuelle Android-Hörabnahme sind damit noch nicht abgeschlossen.

## Lieferung

- [APK für arm64-v8a und x86_64](../build/mission-soundtrack/android/artifacts/republic-commando-0.72.0-mission-soundtrack.apk)
- [Neues Szenenpaket mit Originalmusik](../build/mission-soundtrack/assets/package/mission1-scenes.rcpack.zip)
- [Zusammengefasste Nachweise](../build/mission-soundtrack/verification-summary.json)
- [Quellzuordnung](../build/mission-soundtrack/source-and-verification.json)

APK installieren, das neue Paket einmal über „Szenenpaket importieren“ einlesen und unter „Audioausgabe“ den nativen Mixer wählen. SoundPool bleibt der WAV-Vergleichspfad und spielt diese Musik nicht. Gesamt, Geräusche, Sprache, Umgebung und Musik sind getrennt regelbar; vorhandene vier Regler werden mit Musik = 100 % übernommen.

**Für den vollständigen Test „Mission starten“ verwenden.** Ein alter Spielstand bleibt an seine bisherige zwischengespeicherte Szene gebunden. Ein erneuter Import und „Karte neu starten“ ersetzen diese Szene nicht. Alte B-/C-Spielstände können daher bis zum nächsten Kartenwechsel weiterhin ohne die neue Musik laufen. Es erfolgt keine riskante automatische Spielstandmigration.

## Umgesetzt

1. **Originale Zuordnungen und Musikprogramme importieren.** Ein neuer Audit liest die installierten Karten A/B/C und GEO-/GEN-/YYY-Musikpakete samt aufgelösten Klassenvorgaben. Streamnamen stammen aus den originalen nativen SoundStreamed-Daten, keine Dateinamensvermutung. B enthält sieben Musikprogramme und sechs Ogg-Ressourcen, C fünf Programme und sieben Ressourcen. Ereigniswartezeiten, explizite Zustands-/Episodewechsel, Battle-Schalter, Radien und Abschlussanforderungen werden als begrenzte Musikprogramme übernommen. Jede Gruppe und Auswahl steht im Importbericht.
2. **Inkrementelle Vorbiswiedergabe in der Mission.** Der vorhandene Rustdecoder liefert begrenzte Blöcke, ein Stereo-Renderer setzt sie mit linearer Abtastratenwandlung in dieselben 48-kHz-Ausgabeblöcke wie die WAV-Ausgabe. Es wird nicht die gesamte Musik in PCM umgewandelt oder in SoundPool geladen. Originales Looping, originale Fadeparameter und der vorhandene portable Kurvenkern werden genutzt. Pro Musikkanal bleiben höchstens eine aktuelle und eine ausblendende Stimme erhalten; unveränderte Zustandsmeldungen starten keine Stimme neu. Stummschalten hält die Zeit nicht an.
3. **Verständlichkeit bei Sprache.** Eine explizite Missionsregel senkt während aktiver Dialogquellen Geräusche/Umgebung auf 0,25 und Musik auf 0,5 ab und führt die Pegel weich zurück. Die Faktoren entsprechen den bekannten Konfigurationswerten; die Auslösung durch aktive Dialoge ist eine Portregel, kein Nachweis der originalen Ducker-/Ausnahmeflags. Neue Einzeleffekte verdrängen keine Dialoge. Neue Sprache ersetzt zuerst Einzeleffekte und danach ältere ungeschützte Sprache. Die bestehende belegte NoOverride-Regel für denselben Besitzer bleibt erhalten. Allgemeine Originalprioritäten werden nicht aus den drei Ereigniskategorien erfunden.
4. **Musik über Pause und Prozesswechsel erhalten.** Ein kleiner gespeicherter Musikzustand enthält Ereigniszeiten im Gameplaycheckpoint. Der optionale Audio-Sidecar erhält gewünschte Kanäle, aktuelle Musikpositionen, ausblendende Stimmen, Kurven und Ducking. Neue Decoder werden aus der gebundenen Bank aufgebaut; aktuelle Lautstärkeeinstellungen bleiben wirksam. Wie bisher ist das die Rendergrenze, nicht die zuletzt vom Lautsprecher gehörte Probe. Fehlende/beschädigte optionale Audiodaten verhindern den Gameplayrestore nicht. Eine fehlgeschlagene native Snapshot-Abfrage sperrt jetzt auch spätere Verbrauchsquittierungen.
5. **Gemeinsam ausliefern und prüfen.** RCPLAY15 umschließt eine unveränderte Kampagnenszene mit begrenztem Musikprogramm und komprimierter Bank. RCSND002 und RCLOOP05 ergänzen die unveränderten WAV-/Listenerpakete. Alte Szene-, Bank-, Pose- und Lautstärkepakete bleiben lesbar. Neue Musikabfragen (Game-Audio 9/10) verändern weder Gameplay noch WAV-Ereigniszeiger. Native C-ABI, JNI, Mailbox, Produzent, AudioTrack und Sidecars verwenden denselben Anschluss.

Die Musikmischung verwendet 0,7 als ursprünglichen Musikfaktor neben dem bisherigen Ausgangsgrundpegel und dem neuen Benutzerregler. Die musikalische Ausgabe besitzt maximal vier eigene Softwarestimmen neben dem bisherigen 16-stimmigen WAV-Pool. Ihre Summe wird auf S16 begrenzt; damit wird keine vollständig übersteuerungsfreie Originalmischung behauptet.

## Prüfung

- Je **1.499 Debug-/Release-Bibliothekstests**, Materialtests, Dokumentationstests, Clippy und Buildvertrag bestehen. 15 neue Rust-/ABI-Tests und sechs neue Java-Gruppen; insgesamt **80 Java-Audiogruppen**.
- Vollständiger tatsächlicher A/B/C→Fabrik-Durchlauf: **49.795 normale Eingaben**, **1.230 WAV-Ereignisse**, **39.836.000 Stereo-Frames**, drei Übergänge und vier kalte Gameplayrestores. B meldet zwölf, C dreizehn Wechsel der gewünschten Musikwiedergabe. Die Musiksnapshots beider Karten bleiben beim Neuaufbau exakt erhalten. Die Zählung umfasst auch Stille-/Loop-Rückkehrmeldungen, nicht 25 unterschiedliche Kompositionen.
- Echter B-Musikzustand nach 240 normalen Eingaben, getrennte native Capture-/Restoreprozesse und getrennte JVM-Speicher-/Ladeprozesse mit den produktiven CheckpointStore-/AudioCheckpointStore-Klassen. Musikpositionen und Ducking kommen zurück, der folgende Rendersekundenblock enthält PCM, der Gameplaycheckpoint bleibt unverändert.
- Isolierte Musik mit Originalbank und Originalbefehlen: 50-%-Musikregler halbiert die gemessene Amplitude; Musik-/Gesamt-Mute ergeben nur OpenAL-Dither, während die Musikposition weiterläuft. Aktuelle Regler wirken auch nach Restore.
- Physische OpenAL-Mischprüfung mit kontrolliertem leisen Dialogtrigger und Original-WAV: Geräusch-RMS 1.839,82 → 460,33; Musik-RMS 252,98 → 126,49. Der Trigger und der leere Listener-/Loopzustand dieser Isolationsfälle sind kontrollierte Testeingaben, keine erfundene Mission.
- Pooldruck, Dialogbesitz, Lautstärkeregression, Dialogrestore, Pausenübergabe und beide bisherigen Audio-Checkpoint-Prozessfälle erneut gegen dieselbe DLL geprüft. 36 ältere Gameplayzustände und neun Einstiegskamerabilder bleiben exakt wiederherstellbar.
- Beide Android-ABIs gebaut; ELF-Exporte, Bibliothekszuordnung, 16-KiB-Segmentausrichtung, native Extraktion, DEX-Helfer und Lizenzen strukturell geprüft. **Keine Android-Laufzeit-, Geräte- oder Hörabnahme dieser Version.**

## Reproduzieren

```powershell
./build.ps1 -Target Check -OutputDirectory build/mission-soundtrack/check
./build.ps1 -Target Soundtrack -OutputDirectory build/mission-soundtrack/assets `
  -CampaignPackage <vorhandenes-geprüftes-Missionspaket.zip> `
  -GameData <Originalinstallation/GameData> -Python <python>
./build.ps1 -Target Android -OutputDirectory build/mission-soundtrack/android -Abis arm64-v8a,x86_64
```

Der Soundtrack-Builder benötigt nur Python-Standardbibliothek, den frisch gebauten Audit und eine eigene Originalinstallation. Es gibt keine implizite Abhängigkeit von historischen `analysis/mission…`-Ordnern. Die zusätzlich verwendeten Originalmusikdaten bleiben in den lokalen Build-/Szenenpaketen. Prüfbefehle und Hashes stehen in `build/mission-soundtrack/`; der vollständige Quellstand einschließlich früherer lokaler Änderungen ist dort als Hashliste, Patch und Kopien unversionierter Quellen eingefroren. Kein Commit, Tag oder Push in diesem Abschnitt.

## Was zur vollständigen Originalumsetzung noch fehlt

- **Originale BattleEngine-/iMUSE-Entscheidungen:** Die Programme und ihre Verweise stammen aus den Originaldaten. Welche BattleCriteria- und Sicht-/Bedrohungsregeln einen Kampf aktivieren, die ursprüngliche Zufalls-/Gewichtungsauswahl in SoundMultiple sowie vollständige Flourish-/Episode-Arbitrierung sind nicht allgemein rekonstruiert. Der Port benutzt lebende Gegner in den originalen inneren/äußeren Radien mit Hysterese und deterministisch die erste Gruppenvariante. Episoden unterbrechen den Zustandskanal; danach kehrt dieser an seiner fortgelaufenen Missionszeit zurück. Das ist eine überprüfte Spielregel, keine Behauptung ursprünglicher iMUSE-Gleichwertigkeit.
- **Kartenübergreifender Anfangsmusikzustand:** A schaltet im lokalen Originalprogramm bei `levelstart` die BattleEngine aus und hat dort keine direkte Musikzuordnung. Seine bisherigen Szenenbytes bleiben deshalb unverändert. Damit ist nicht bewiesen, welche Musik aus dem vorangehenden, noch nicht portierten Briefing übernommen wurde. Eine beliebige Introdatei wird nicht als Originalverhalten eingesetzt. Der Fabrikeinstieg bleibt die Grenze von Mission 1.
- **Allgemeine Original-Misch-/Prioritätsintegration:** Die bereits rekonstruierten Kerne bleiben getrennte Nachweise; originale Quellenslots, Ducker-/Ausnahmeflags und laufende Prioritätsmetadaten fehlen weiterhin im allgemeinen Missionsereignisvertrag. Die neue Sprachschutz-/Duckingregel schließt die praktische Lücke, ersetzt aber keinen solchen Originalnachweis.
- **Handy und Hören:** Musik-/Dialogeinsätze, Übergänge, Anfangszustand, Richtungswirkung, Übersteuerung (A erreicht im Host weiterhin den S16-Spitzenwert), Laufzeitkosten, Audiofokus, Hintergrund/Prozessverlust und kleines Reglerdisplay im freien Spiel abnehmen. Vorher ist der Auftrag „Audio vollständig originalgetreu und am Gerät abgenommen“ nicht als erledigt zu markieren.

**Fortsetzung 27.10.2026:** [0.73.0: Originale Musikentscheidungen und Episodensperre](MUSIC_DECISIONS.md) ergänzt drei originalgeprüfte Kerne und den ausgewählten Episodenanschluss. Die 0.72.0-Lieferung und ihr Paket bleiben unverändert; vollständige Gruppen-/Battle-Integration bleibt offen.
