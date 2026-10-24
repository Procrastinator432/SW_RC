# Originale Musikentscheidungen und Episodensperre

**Historischer Stand 0.73.0.** Der folgende Bericht bleibt als Nachweis erhalten. Der aktuelle Missionsanschluss und die neue Lieferung sind in [MUSIC_GROUPS.md](MUSIC_GROUPS.md) beschrieben.

Stand: 27.10.2026 · **0.73.0-music-decisions**. Die Missionsausgabe ignoriert nun überlappende und leere Episodenanforderungen entsprechend dem belegten Originalgate. Zusätzlich sind Kampfprüfung und SoundMultiple-Auswahl als eigenständige Originalkerne rekonstruiert und mit ausgeführten Originalinstruktionen verglichen. Deren vollständiger Missionsanschluss ist weiterhin offen.

[APK für beide ABIs](../build/music-decisions/android/artifacts/republic-commando-0.73.0-music-decisions.apk) · [Prüfbericht](../build/music-decisions/verification-summary.json) · [Quellzuordnung](../build/music-decisions/source-and-verification.json)

Das [Musik-Szenenpaket aus 0.72.0](../build/mission-soundtrack/assets/package/mission1-scenes.rcpack.zip) bleibt unverändert verwendbar. Wer es bereits importiert hat, muss für diese Version nur die APK aktualisieren. Ältere, vor dem Musikpaket gespeicherte Szenen werden weiterhin nicht automatisch migriert.

## Umgesetzt und abgegrenzt

| Bereich | Originalbefund und Umsetzung | Anschluss an die Mission |
|---|---|---|
| Episodenannahme | `PlayMusicEpisode` 103d0ee0 und Aufnahmegate 103ab9b0 ignorieren Nullanforderungen und neue Episoden während einer laufenden Episode. Der skalare Kern bildet außerdem das Löschen des aktiven Battle-Cues und des ausstehenden Overlays ab; die physische Flushoperation ist ein Hostauftrag. | **Das Aufnahmegate ist angeschlossen.** Der ausgewählte Missionsadapter beobachtet das Episodeende anhand der Assetdauer auf der Simulationsuhr. Eine vorher verworfene Episode wird nicht später nachgeholt; Raumzustände werden währenddessen weiter vorgemerkt. Vollständige originale Overlay-/Battle-Cue-Verwaltung und echte Geräte-Endrückmeldungen bleiben getrennt. |
| Kampfprüfung | 103ac860 verwendet die originalen Kriterien **Attack**, **Proximity**, **AttackAndProximity**. Das Angriffskriterium erhält ausdrücklich das beobachtete Controllerbit `+29c & 8000`; es wird nicht aus Sichtbarkeit, Nähe oder Lebenspunkten erfunden. Nähe prüft die quadrierte Entfernung streng kleiner als den quadrierten Radius, in originaler Binary32-Reihenfolge. Bei laufendem bzw. ausdrücklich erzwungenem Kontext ist die Schwelle eins; der äußere Radius hängt dagegen nur vom tatsächlich laufenden Kampf ab. | **Eigenständiger Kern.** Aktuelle Missions-Gegner liefern dieses originale Controllerbit noch nicht. Der vorhandene Radiusadapter wird deshalb nicht als vollständige Original-BattleEngine bezeichnet. |
| BattleVolumes | Der Kern unterscheidet Spieler-/Gegnerzugehörigkeit zur Zone und die Aussage, ob ihre Gegnerfabriken erschöpft sind. Ein laufender Kampf in der Zone kann bei noch aktiven Fabriken auch ohne nahe Gegner bestehen bleiben. Die an den Baddie-Collector gerichteten Aufrufe bleiben in Reihenfolge erhalten. | Volumenzugehörigkeit, Klassen-/Teambeziehungen, Totstatus und Fabrikstatus sind explizite Beobachtungen. Der Kern implementiert weder die originalen Fabriken noch den deduplizierenden Pawn-Collector. Der gesamte einsekündige Battle-Update-/Nachlaufzeitplan ist damit ebenfalls nicht portiert. |
| SoundMultiple | 103d1f80: geordnete Auswahl mit Umbruch, einmaliger FirstSound, Kampagnen-/Battlefilter, Nullmitglieder, gecachte bzw. neu berechnete Gesamtgewichte und die ursprüngliche Rückfallwahl auf das erste Mitglied. Auswahl und Cachefelder werden einschließlich Rohbits erhalten. Vor dem strengen Vergleich wird das Zufallsprodukt wie im Original nach Binary32 gerundet. | **Eigenständiger Kern.** Die Original-Zufallswerte werden im Vergleich kontrolliert vorgegeben. Globale RNG-Zeitlinie, Gruppenbaumimport, rekursive Auswahl und ursprüngliches RefreshGroup müssen noch angeschlossen werden. Der Szenenbuilder verwendet weiterhin seine dokumentierte erste Gruppenvariante. |
| Anfangsmusik | Zusätzliche Auditwahl für andere Karten; `geo_01briefing.ctm` wurde mit 94 ausgewählten Kartenrecords untersucht. Darunter fand sich weder eine direkte PlayMusic-/SetImuseState-Aktion noch eine LevelInfo-Musikzuweisung. | Das beweist keine Aussage über alle nativen Initialisierungspfade oder die Musikübernahme beim Kartenwechsel. Die Anfangsmusik von A bleibt ungeklärt. |

Die neue Kernimplementierung liegt in [music_decisions.rs](../crates/rc-package/src/music_decisions.rs), der ausgewählte Missionsanschluss in [music.rs](../crates/rc-game/src/music.rs). Eingaben und gespeicherte Gruppen erhalten zusätzliche begrenzende Validierung. Die Originalvergleiche gelten für die beschriebenen gültigen Zustände, nicht für beliebige beschädigte native Speicherobjekte.

## Nachweise

**3.514 ausgeführte Originalaufrufe**, anschließend erneut gegen die eingefrorene Fixture geprüft:

- 1.280 Kampfprüfungen, darunter 512 Fälle an Float-/Radiusgrenzen; Ergebnis und Aufrufliste des Collectors stimmen überein.
- 2.106 Gruppenwahlen über 160 Zustandsfolgen, einschließlich Gewichtsgrenzen, FirstSound, Reihenfolge, Filterwechseln und Nullmitgliedern. Auswahl, alle abgebildeten Cachefelder und die Notwendigkeit eines Zufallsaufrufs stimmen überein. Die Rustfolge wird nach jedem Schritt serialisiert und kalt wiederhergestellt.
- 128 öffentliche Episodenaufrufe über alle Verfügbarkeits-/Null-/Belegungsvarianten; Zustandsfelder und Anforderung der physischen Flushoperation stimmen überein.

Die Tests führen die lokal vorhandene, gehashte Original-Engine mit Unicorn aus. Teambeziehungen, Tod, Volumen-/Fabrikantworten, Collector, Zufallswerte und der physische Episode-Flush sind kontrollierte Callbackgrenzen. Kein Spielablauf oder Lautsprecherklang wird dadurch behauptet. Private Dekompilate bleiben in den ignorierten Analyse-/Buildordnern; die versionierte Fixture enthält Eingaben und Ergebnisse, keine Originalinstruktionen.

Zusätzliche Nachweise mit dem endgültigen Port:

- Je **1.509 Debug-/Release-Bibliothekstests**, zehn neue Rust-Testgruppen; **80 Java-Audiogruppen**, Material-/Dokumentationstests, Clippy und Buildvertrag bestanden.
- Vier kontrollierte Programme mit tatsächlichen C-Musikressourcen über RCPLAY15 und die Spiel-C-ABI: Überlappung, Anforderung exakt am Clipende, Nullanforderung und vorgemerkter Raumwechsel. Musikabfragen bleiben unveränderlich. Diese bearbeiteten Programme sind ausdrücklich Diagnosefälle, keine angeblichen Originalmissionsereignisse.
- Tatsächlicher A/B/C→Fabrik-Lauf mit **49.795 Eingaben**, **1.230 WAV-Ereignissen**, **39.836.000 Stereo-Frames**, drei Übergängen und vier kalten Restores. B/C behalten ihre zwölf/dreizehn beobachteten Musikzustandswechsel.
- Musikspeicherung über getrennte native und JVM-Prozesse, Pegel-/Ducking-/Pool-/Dialog-/Pausenregressionen und beide bisherigen Audio-Checkpoint-Prozessfälle erneut bestanden.
- Ein echter Musik-/Gameplaycheckpoint aus der **0.72.0-Lieferung** wurde über den produktiven Java-Sidecarleser und die neue native DLL wiederhergestellt; Gameplaybytes bleiben exakt, Musikpositionen laufen weiter. Die alten Nachweise wurden dabei nur gelesen und kopiert. Zusätzlich 36 historische Gameplayrestores und neun Einstiegskamerabilder exakt bestanden.
- APK für arm64-v8a und x86_64 gebaut; ELF-/DEX-/Lizenz-/16-KiB-/Extraktionsvertrag geprüft. **Keine Android-Laufzeit- oder Hörabnahme dieser Version.** Paket und frühere Lieferungen bleiben unverändert; kein Commit, Tag oder Push.

## Reproduzieren

```powershell
$env:PYTHONPATH=(Join-Path $PWD 'tools/analysis-python')
python scripts/Test-MusicDecisionsOriginal.py --game-data <GameData> `
  --verify-fixture fixtures/music-decisions-native.json `
  --output build/music-decisions/original-replay.json
./build.ps1 -Target Check -OutputDirectory build/music-decisions/check
./build.ps1 -Target Android -OutputDirectory build/music-decisions/android -Abis arm64-v8a,x86_64
```

Der frische Originalvergleich benötigt die gepinnten Pakete aus `config/analysis-requirements.txt`; Rusttests funktionieren mit der Fixture offline und ohne Originalinstallation. Für das zusätzliche Briefingaudit unterstützt `rc-music-audit GAME_DATA OUT.json Maps/geo_01briefing.ctm` eine explizite Kartenauswahl. Ohne zusätzliche Argumente bleibt die bisherige A/B/C-Auswahl erhalten.

## Nächster Anschluss

Originale Gruppenbeschreibungen einschließlich Kontext, Kampagnenbereich, Gewicht, FirstSound und Reihenfolgestil importieren und die Gruppenzustände/RNG-Aufrufe an den gespeicherten Missionszustand binden. Danach das echte Controller-Angriffsbit, BattleVolumes und Fabrikstatus bereitstellen und den ursprünglichen Battle-Zeitplan anschließen. Anfangsmusik und allgemeine Original-Prioritäts-/Duckerflags sowie Geräte-/Hörabnahme bleiben offen. **Dieser Meilenstein schließt diese Punkte nicht pauschal ab.**
