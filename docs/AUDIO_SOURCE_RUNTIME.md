# Audioquellen: Initialisierung, Verwaltung und gespeicherter Fortlauf

**Quellfortsetzung 24.10.2026:** [Transient-Kanäle, Aktivierungsvorbereitung und Stimmenzählung](AUDIO_ACTIVATION.md), fünf weitere intern umgesetzte Pakete und acht neue Testgruppen. Die AL-Upload-/Playgrenze, globale Update-Sortierung und der allgemeine App-/Treiberanschluss bleiben ausdrücklich offen.

**Weitere Quellfortsetzung 24.10.2026:** [Pause-/Stopbefehle und persistente Warteschlange](AUDIO_COMMANDS.md), fünf intern umgesetzte Punkte mit 1.088 Originalbefehlsaufrufen. Dort ist auch die korrigierte Zuordnung `IsPaused +1c` / `IsQueued +20` dokumentiert. Allgemeiner App-/Treiberanschluss bleibt offen.

Stand: 24.10.2026. Die nächsten fünf Punkte nach den [allgemeinen Audio-Controllern](AUDIO_CONTROLLERS.md) sind als intern prüfbare Quellenbausteine umgesetzt. Die neuen Module liegen in `crates/rc-package/src`; sie sind unabhängig von einer Mission und vom Android-Treiber verwendbar. Die spielbare Referenz bleibt 0.60.0. Es gibt für diesen Block keine neue APK, Mission oder Hörabnahme.

## Die fünf umgesetzten Punkte

| Punkt | Implementierung | Interner Nachweis |
|---|---|---|
| 1 · Quelleninitialisierung | `audio_source::Source` und `Init`: Quellen-/Actor-/Soundhandles, Flags, Slot, Priorität, Rate und Lautstärken, Originalkurve/Fade-in, Radien und Zeitinitialisierung. Konstruktor auf vorher genulltem Speicher mit umlaufendem 32-Bit-Zähler. | Originalkonstruktor und ganzer Init-Pfad mit kontrollierter Engineauflösung; fünf Zählergrenzen und 384 Initialisierungen. |
| 2 · Actor-Anbindung und Aktualisierungsreihenfolge | `audio_source_update`: Belegungs-/Pause-/Engine-Gates, Abtrennung ungültiger Actors, Actorposition, Relativgeschwindigkeit, Nullvektoren für Flag `0x100`, Zählerbedingung und Reihenfolge Volume→Radii→Priority. | 512 ganze originale UpdateBase-Aufrufe mit kontrollierten Actor-/Pause-/Unterfunktionscallbacks, einschließlich Clear während Volume und anschließender Slotneubewertung. |
| 3 · Quellenlebensdauer | `Source::clear`, `pause` und Basis-`stop`: genau begrenzte Feldänderungen; Identität, Positionen, Kurven und Radien bleiben gemäß Original erhalten. | 480 Pause-/Clear-/Stop-Schritte gegen die Originalfunktionen, mit kaltem Fortlauf. |
| 4 · Aktive und wartende Quellen finden | `audio_source_lookup`: `Matches`, aktiver Index, wartende Suche, aktive Suche mit Queue-Fallback und exakte Unique-ID-Suche. Automatische Actorwahl hat einen eigenen expliziten Eingang. | 256 Poolszenarien mit jeweils vier Suchaufrufen sowie 1.893 Matches- und 1.893 ID-Proben. |
| 5 · Zusammengesetzte, persistente Quellenverwaltung | `audio_source_runtime::Runtime`: vorhandene Kurven/Fade/Ducking/Radien/Priorität zusammen ausführen, Poolauswahl zum Installieren eines eigenen Quellensnapshots verwenden und versionierte Kernzustände atomar laden. Mehrere Quellen werden als ein Rechenschritt übernommen. | 768 Originalschritte mit echten Unterfunktionen, gespeicherter Fortlauf nach jedem Schritt, 512 vorhandene Original-Poolfälle sowie Fehlerabweisung nach bereits berechneter erster Quelle. |

Die neun neuen Rust-Testgruppen prüfen die Ergebnisse aller eingefrorenen Fälle. Jeweils **1.224 Bibliothekstests in Debug und Release** bestehen. Der vollständige zentrale Quellcheck besteht: Format, Debug/Release, Materialimport-/Dokumentationstests, Clippy, Native-/Inspector-Build, Java und Buildvertrag. Der frische Originalvergleich bestätigt die versionierte Fixture vollständig. Belege liegen unter `build/audio-source-runtime/`; `verification-summary.json` fasst den endgültigen Stand zusammen.

## Originalverträge

| Funktion | Adresse |
|---|---|
| Konstruktor / Init | `103d24a0` / `103d2a40` |
| Matches / Clear / Pause / Basis-Stop | `10358e80` / `10358eb0` / `10358f00` / `10358f10` |
| GenerateSourceId | `10359630` |
| GetSourceIndex / GetQSource / GetSource / FindSourceById | `103d2f90` / `10363480` / `103634d0` / `10363540` |
| UpdateBase | `103d5340` |

`Source` ist ein portabler Feldvertrag, kein natives Speicherabbild. Pointerartige Werte sind undurchsichtige, vom Aufrufer bereitgestellte `u32`-Handles; sie werden niemals dereferenziert. `unique_id` ist die beim Konstruktor erhöhte Kennung. Sie ist von der gepackten `identity` mit Slot-/Schutzbits getrennt. Der Konstruktorzähler wird vom Aufrufer übergeben; der neue Runtime-Snapshot speichert vorhandene Quellen, aber keinen globalen Erzeugungszähler. Dessen Verwaltung und Wiederherstellung gehören zur späteren Engineanbindung. `GenerateSourceId` gibt die vorhandene Unique-ID zurück und erzeugt keine neue.

Der native Konstruktor initialisiert nicht jedes Feld. `Source::new` bildet deshalb ausdrücklich den Konstruktor auf genulltem Speicher ab. Init auf einer wiederverwendeten Quelle erhält unter anderem Unique-ID, Pauseanforderung und Geschwindigkeit. Die unaufgelöste Zelle `+98` wird als `reserved98` unverändert mitgeführt. Pitch liegt bei `+8c`, Basislautstärke bei `+90`, die anfängliche und später berechnete Ausgabelautstärke bei `+94`.

Bei Slot 3 prüft Init die Begrenzung `audio+dc`, weist im begrenzten Zweig aber `audio+d8` zu. Die Umsetzung erhält diesen Befund. Radius 0 wird anschließend durch 10 ersetzt; die ursprüngliche Radiusänderungszeit ist −1. Fade-in wird auf der originalen Nullkurve aufgebaut; bei Dauer 0 ist der Modus sofort inaktiv.

Clear setzt Belegung, Sound-/Actorhandles, Flags, gepackte Identität, Priorität, Cursor und Pauseanforderung zurück sowie den gehaltenen Fadefaktor auf 1. Es setzt weder die Unique-ID noch den Kurven-/Radius-/Positionszustand zurück. Pause speichert die Anfrage einschließlich beliebiger i32-Werte. Die tatsächlichen virtuellen `IsPaused`-/`IsQueued`-Ergebnisse bleiben getrennte Treibereingaben; sie werden nicht aus dieser Zelle erraten. Der implementierte Basis-Stop verwendet das Standard-Clear; Treiberüberschreibungen gehören weiterhin zum Host.

UpdateBase kann zweimal nach dem Pausezustand fragen. Die beiden Antworten sind separate Eingaben. Der Zähler `audio+22c` wird nur im erreichten Hauptzweig bei gelöschtem Flag 4 erhöht; `BaseOutcome::counter_incremented` meldet genau diese Bedingung. Die Slotprüfung für den Prioritätsaufruf erfolgt **nach** Volume und Radii. Ein Stop/Clear während Volume kann deshalb dazu führen, dass eine zuvor als Ambient behandelte Quelle anschließend noch die Prioritätsfunktion durchläuft.

Aktive und wartende Suchlisten werden in Originalreihenfolge durchsucht, auch wenn ein Eintrag nicht als belegt markiert ist. Ein automatischer Actor wird nur innerhalb der aktiven Indexsuche aufgelöst: Params `+54`, Bit 0 und Sound `+30`, Bit 1 müssen gesetzt sein. Der spätere Queue-Fallback verwendet wieder die ursprünglichen Suchargumente. Unique-ID-Suche vergleicht alle 32 Bits und bevorzugt aktive Einträge. Sie verwendet nicht die maskierte gepackte Identität der Poolverdrängung.

## Zusammenspiel und Speichervertrag

`Runtime::step` erhält Stimmenzahl, Zeitschritt und pro Quelle einen `Frame`. Es aktualisiert zuerst den Duckingcontroller und dann jede Quelle in Listenreihenfolge. Der Volume-Adapter verarbeitet echte Fade-/Duckingwerte, bildet einen Basis-Stop auf Clear ab und schreibt die Ausgabelautstärke an das entsprechende portable Feld. Danach folgen Radiusfortlauf und laufende Priorität.

Der frische Originalvergleich führt in seinen 768 Zusammenschritten die **ganzen originalen Funktionen** UpdateBase, UpdateVolume, GetFadeMultiplier, GetDuckMultiplier, UpdateRadii und UpdatePriority aus; bei Stop läuft der originale Basis-Stop mit Clear. Originaler Duckingfortlauf wird zuvor ebenfalls ausgeführt. Die neuen Fallfolgen decken klare und nichtpositionale Hörbarkeit ab. Geometrische Verdeckung stammt weiterhin aus dem vorherigen separat geprüften Geometrie-/Radiusvertrag und ist hier kein neuer kompletter Trace-Nachweis.

Die folgenden Grenzen sind kontrolliert: Engine-/Outer-Auflösung, Actorvalidität und Warnungslogger, Listener-/Actorwerte, Levelzustand, die klare Hörbarkeitsantwort und Queue-/Pauseabfragen des Treibers. Native Profilingzähler laufen mit künstlichem Speicher; daraus wird kein Leistungsnachweis abgeleitet. Isolierte UpdateBase-Fälle ersetzen seine drei Rechenunterfunktionen durch protokollierte Callbacks; die Zusammenschritte ersetzen sie ausdrücklich nicht.

`Runtime::admit` benutzt die bereits originalgeprüfte Poolauswahl und installiert einen eigenen Snapshot in den ausgewählten Slot. Die eingehende Auswahlpriorität muss der Aufrufer zuvor bestimmen: Init setzt außerhalb Slot 3 zunächst −1; `admit` ersetzt diesen Platzhalter nicht automatisch durch die separate Aufnahmepriorität. Das ist eine Verwaltung eigener Rustdaten; es führt keine externen Treiber-Stop-/Attach-Aufrufe aus und behauptet keinen vollständigen Originalallocator.

`checkpoint`/`restore`/`load` verwenden ein eigenes JSON-Format `{schema:1,state:…}`. Floatzustände und Uhren bleiben in Integerwörtern erhalten. Nach jedem der 768 Originalschritte wird über diese Schnittstelle kalt gespeichert und geladen. Unbekannte Versionen/Felder, beschädigte Zahlen und mehr als 4.096 Quellen werden abgewiesen; Eingabedateien sind auf 16 MiB begrenzt. `load` übernimmt erst nach vollständiger Validierung.

Der reine `Runtime::step` übernimmt Quellen und Ducking gemeinsam erst bei Erfolg. Die Tests erzwingen einen Fehler in einer späteren Quelle und belegen, dass frühere Quellen und Ducking unverändert bleiben. Zustandsänderungen einer benutzerdefinierten `CurveMath`-Politik sind außerhalb dieser Transaktion. Der niedrigere `audio_source_update::update`-Pfad mit externen `UpdateHost`-Callbacks besitzt dagegen bewusst keine Rückabwicklung bereits ausgeführter externer Aktionen.

## Reproduktion

Originaldateien: dieselben gehashten `engine.dll` und `msvcr71.dll` wie im [Controllerbericht](AUDIO_CONTROLLERS.md). `fixtures/audio-sources-native.json` enthält synthetische Eingaben, Zustände und Ergebnisse; keine Originalinstruktionen und keine Audio-/Kartendaten. Der Prüfer verwendet den vorhandenen Mapper aus `Test-AudioControllersOriginal.py`.

Linux:

```bash
source scripts/env-linux.sh
cargo test --locked --offline -p rc-package --lib audio_source_tests
python scripts/Test-AudioSourcesOriginal.py --game-data "$RC_GAME_DATA" \
  --verify-fixture fixtures/audio-sources-native.json \
  --output build/audio-source-runtime/original-replay.json
./build.sh -Target Check -OutputDirectory build/audio-source-runtime/check
```

Windows mit eingerichteten Werkzeugen unter PowerShell 7:

```powershell
cargo test --locked --offline -p rc-package --lib audio_source_tests
python scripts/Test-AudioSourcesOriginal.py --game-data $env:RC_GAME_DATA `
  --verify-fixture fixtures/audio-sources-native.json `
  --output build/audio-source-runtime/original-replay.json
./build.ps1 -Target Check -OutputDirectory build/audio-source-runtime/check
```

Die Rusttests brauchen nur die versionierten Fixtures; frischer Originalvergleich benötigt die Originalinstallation und die Pythonpakete aus `config/analysis-requirements.txt`. Die Pfade bleiben über Umgebungsvariablen beziehungsweise Argumente konfigurierbar.

## Grenzen und noch offene Abnahme

Die Präzisionsgrenzen des vorherigen Blocks gelten weiter: portable nichtlineare Mathematik wurde auf diesem Rechner geprüft; die laufende Priorität erhält den aufgezeichneten originalen RSQRTSS-Seed. Eine andere sqrt-Politik ist kein Nachweis derselben x86-Näherung.

Die portablen Eingaben sind validiert und begrenzt. Quellen-/Relativvektoren müssen innerhalb ±1e6 liegen; Zeiten und Zielwerte folgen dem Controllervertrag. Die native aktive Indexsuche dereferenziert bei Actor=null und Sound=null einen Nullpointer; dieser Aufruf wird portabel abgewiesen. Die wartende Wildcardsuche darf beide Handles 0 verwenden. Diese Grenzen bilden keine Behauptung ab, dass die Originalengine beschädigte Eingaben sicher behandelt.

Die fünf internen Pakete sind abgeschlossen. Es fehlen weiterhin der tatsächliche allgemeine Spielquellen-/Treiberanschluss, originale globale Stimmenverwaltung, Queueaktivierung/Streaming und deren App-Checkpointintegration. Das neue Kernsnapshotformat verändert keine vorhandenen App-Spielstände. Danach sind neue Missions-, Checkpoint-, Android- und Hörprüfungen nötig. Zusätzlich bleibt die zuvor ausdrücklich zurückgestellte [Windows-Abnahme der Geometrieanbindung](AUDIO_GEOMETRY_RUNTIME.md#windows-abnahme-bewusst-zurückgestellt) offen.
