# GEO_01C: Abseilen und Sev-Begegnung / 0.37.0

Der dritte Abschnitt beginnt jetzt mit dem ausgewählten Originaleinstieg statt freier Bewegung auf der oberen Startplattform. PlayerCommando, Scorch und Fixer steigen mit `EntranceRope1760` ab; Sev und die beiden Intro-Warriors führen ihre Originalanimationen aus. Nach dem Dialog tritt Sev bei, Originalziel1 „Rendezvous with Zero-Seven“ wird abgeschlossen und die normale Steuerung wird freigegeben. Sun Fac steht mit dem aus seiner Originalklasse aufgelösten Geonosian-Mesh im Strategieraum. Seine Flucht und Eliminierung sind noch nicht spielbar.

## Ausprobieren

- APK: `analysis/mission20-map3-opening/delivery/republic-commando-0.37.0.apk` (ARM64/x86_64, Code41).
- Passende Szene: `analysis/mission20-map3-opening/delivery/geo_01c.rcplay`.
- GEO_01A und GEO_01B verwenden weiterhin die unveränderten Szenen der Lieferungen0.32.0 und0.35.0. Am B-Ausgang **Weiter: GEO_01C** wählen und die neue Szene öffnen.
- Ein gültiger gespeicherter B→C-Übergang wird beim direkten Öffnen wiederhergestellt. Im Referenzlauf bleiben10HP,20/141Munition, vierGranaten und Scorch/Fixer100HP/100Schilde erhalten. Ohne gespeicherten Übergang gilt der reguläre eigenständige Port-Start.
- Während des Einstiegs sind Bewegung, Schüsse, Granaten und Sprünge gesperrt; Schauen bleibt innerhalb der originalen Briefing-Grenzen möglich. Danach sind Bewegung, Springen, Ducken, Schießen, Nachladen und Pause verfügbar. Neustart spielt den Einstieg erneut ab.
- Das Originalereignis `CheckpointLevelStart` ist gebunden. Ein zusätzlicher Speicherstand innerhalb von GEO_01C wird dadurch noch nicht geschrieben; persistiert wird weiterhin der Kartenübergang aus B.

## Originalbindungen

Ausgewählte Aktionsketten103/98/99/101/264/292/293 führen Warteereignisse, Timer, Animationen, Dialoge, Beitritt, Zielfortschritt und Briefing-Ende aus. Die beiden Seil-Receiver100/102 werden durch `CloneRope62` beziehungsweise `CloneRope40` aktiviert. Ihre aus den Originaldaten dekodierte `AnimTriggered`-Struktur nennt ausdrücklich `CloneEntrance1760`, nicht schleifend.

`EntranceRope1760` stammt aus `Clone.CloneCommandoSPSet`:153Frames bei30FPS mit fünf Originalnotifies. `SevIntro` stammt aus `Clone.cGEO01CloneSet`:195Frames/30FPS mit acht Notifies; hinzu kommt ein Notify von `GestureSignalOK`. Die beiden Warrior-Sequenzen aus `Geonosian.cGEO01WarriorSet` besitzen zwei beziehungsweise drei Notifies. Native Kanalinitialisierung, LOD-Tick, Notify-Ausführung, AnimEnd und relative Root-Matrizen verwenden die bereits rekonstruierten Rust-Funktionen. Sie werden nicht durch einen zeitlich interpolierten Kameraweg ersetzt.

Die drei Abseil-Körper verwenden Originalpositionen und -rotationen. Ihre authored Bewegungen werden gegen die vorhandene Welt mit Körperausdehnung geprüft. Die stehende Port-Kapsel begrenzt die letzten Animationsbewegungen am Boden: im Referenzlauf42begrenzte Spielerschritte sowie je43bei Scorch und Fixer. Die vollständige native RootMotion-/Kapsel-/Floor-Übergangsparität wird nicht behauptet. Sev wechselt nach seiner Introanimation zur Bodenphysik und landet unterstützt, statt über dem Boden zu verbleiben. Beim Zeichnen werden die originalen Kollisionshöhen84/115 und die Modellgrenzen berücksichtigt.

Scorch, Fixer und Sev verwenden ihre gebundenen Originalskins und Waffenbefestigungen. Die beiden echten Seil-Meshes ersetzen die zuvor ausgelassenen DefaultProp-Hilfsmeshes. Sonnen-/Sun-Fac-Shader, Beleuchtung, Blends und Partikeleffekte sind weiterhin nur teilweise unterstützt. Die Intro-Warriors sind dekorative Akteure; ihre Körper verschwinden im begrenzten Host nach AnimEnd. Ihre ursprünglichen Ragdolls, Blut-/Staubeffekte, Pike-Props und die vollständigen Waffen-/Notify-Seiteneffekte fehlen noch.

Die Soundbank enthält14Originalsamples: Spieler- und Squad-Schuss, Abseil-Foley, Sevs Treffergeräusche und die Einstiegsdialoge. Der Referenzlauf erzeugt30Audioereignisse einschließlich der zwei späteren Spielerschüsse. Dialogwartezeiten stammen aus den tatsächlichen WAV-Daten. Originale Dialogtexte erscheinen als Untertitel. Die ausgewählten Skripttimer werden im festen60Hz-Host ausgeführt; es wird keine vollständige native Skriptmaschine oder räumliche Audioparität behauptet.

## Nachweise

`playthrough/validation.json` spielt1.800normale C-Eingaben ab einem tatsächlich erzeugten Schema2-B-Abschluss. Es setzt keine laufenden Positionen, Ressourcen oder Ereignisse. Geprüft sind Abstieg, Animationsenden, Notifies, Sevs Beitritt/Bodenkontakt, Originalziel1 und `TargetSunFac`/`CheckpointLevelStart`. Anschließend werden Bewegung, ein Sprung, zwei Schüsse, Nachladen und Ducken geprüft; Ergebnis10HP,20/139Munition. Zustand und Grafikframe sind nach Neustart identisch.

Bei20/60/144Eingabeframes proSekunde ist der gesamte Simulationszustand nach25Sekunden identisch. Eine separate Prüfung hält während des gesperrten Einstiegs Bewegung, Blick, Schießen, Nachladen, Springen, Ducken und Granatenwurf gedrückt: keine zusätzliche Bewegung neben der Originalanimation, kein Munitionsverbrauch und kein nachträglicher unbeabsichtigter Schuss.

`native/validation.json` und `android/native-validation.json` spielen zuvor die tatsächlichen18.730A- und16.414B-Eingaben durch und wechseln dann in die neue C-Szene. Die C-Einstiegs- und Steuerfolge wird über die Produktionsschnittstellen abgespielt; Übergabe, Audio, Sevs Beitritt, Neustart und Grafik-Caches werden geprüft. Die APK-Prüfung in `android/validation.json` öffnet C mit dem tatsächlich unterAndroid erzeugten gespeicherten B→C-Übergang, wartet bis zu Sevs Beitritt und prüft anschließend GPU, Touch, Waffenton und Pause. Sie ist kein vollständiger manueller Touch-Durchlauf durch A/B/C. Physische ARM64-Abnahme bleibt offen.

Die Lieferung enthält Quellen, Originalbindungen, gerenderte Zwischenstände und SHA-256-Nachweise.1.114bestehende Rust-Bibliothekstests, Clippy/Format und der Java-Szenenleser unter96MiBHeap sichern die Erweiterung. Frühere Lieferungen bleiben unverändert.

## Nächster Abschnitt

Die erste Gesamtmission bleibt unvollständig. `SpawnPlanningRoom` und `GeoFighterFlyByRappel` werden als Originalereignisse ausgelöst; die fünf zugehörigen Kampfgegner und der Jägerüberflug werden in dieser Lieferung noch nicht erzeugt. Als Nächstes folgen Planungsraum-Kampf, schädigbares Strategieraumglas und `WindowDestroyed`, Sun Facs Flucht und der Superkampfdroiden-Auftritt. Danach werden Türbrech-/Scharfschützenmanöver und der Aufzug bis zum ursprünglichen Ausgang nachGEO_03A angebunden. Ziel0 „Eliminate Sun Fac“ wird hier nicht als erledigt markiert.
