# DC-15s: fünf Schritte zur originalen Waffenanimation

## Umgesetzter Umfang

1. **Atmen, Gehen und Rennen:** `Weapon.PlayIdleAnim` verwendet die tatsächliche dreidimensionale Geschwindigkeit, nicht die Positionsdifferenz. Unter 0,04 Geschwindigkeit² gilt Atmen, ab 0,04 Gehen und ab 51.076 Rennen. Auch vertikale Bewegung zählt wie im Original.
2. **Ruheanimation:** `PlayIdleTime=20` stammt aus `properties.DC15sDefaults`, Property-Payload 186. Bei vollständigem Stillstand beginnt nach dieser Frist der originale Clip `Idle`; anschließend kehrt die Waffe zum Atmen zurück. Bewegung und Waffenaktionen unterbrechen beziehungsweise erneuern die Frist. Der Port misst die Frist auf seiner vorhandenen festen 60-Hz-Simulation; Pause hält sie an.
3. **Originale Schussbindung und Clipende:** `FireAnim=Fire` wird über den ursprünglichen Klassendefault von `engine.Weapon` aufgelöst. Jeder akzeptierte neue Schuss startet diesen Clip neu. Das Ende des separaten Schusskanals gibt die ruhende/bewegte Darstellung wieder frei, ohne den unabhängigen Schussabstand zu löschen. Die vorhandene Halbautomatik bleibt erhalten.
4. **Zwei Animationskanäle:** Kanal 0 erhält seine eigene Phase für Atmen/Gehen/Rennen/Ruhe; Kanal 1 spielt den Schuss. Beide werden über den bestehenden vorbereiteten Original-Kanalhost und die originalen komprimierten Tracks ausgewertet. Der Schusskanal überschreibt im ausgewählten Pfad mit vollem Gewicht; es wird keine erfundene additive Mischung ergänzt. Granatenwurf, Nachladen und gesperrte Waffen unterbrechen ihn. Auch frühe Rückkehrpfade bei Intro/Abseilen/Kampfunfähigkeit bereinigen alte Schussposen.
5. **Persistenz und Integration:** Ruhefrist, Ruheflag, Schussclip/-frame und Kanalflags gehören zum Checkpoint. Fehlerhafte Modulpräsenz, Fristen, Clipnummern, Frames und Flags werden vor der Übernahme geprüft. Gegnerfabriken erhalten beide laufenden Waffenkanäle beim Neuanlegen des Gegnerzustands. Ein regulärer Kartenwechsel beginnt die Darstellung am neuen Karteneinstieg; Energie und Feuertastenstatus behalten ihren bestehenden Übergabevertrag.

Alte Szenen ohne optionales Animationsprofil verwenden weiterhin ihre frühere Auswahl. Vorhandene Clipnummern, Texturen und Meshtransformationen werden nicht ersetzt. Der Szenenbuilder ergänzt fehlende Clips aus genau dem Animationssatz des vorhandenen Waffenmeshs und prüft die Bone-Zuordnung. Originalwerte und Clipbelege stehen je Karte in `dc15-energy.json` und `weapon-clips.json` unter dem lokalen Buildverzeichnis.

| Originalclip | Frames | Originalrate |
|---|---:|---:|
| ActionBreathe | 80 | 30 FPS |
| Walk | 25 | 30 FPS |
| Run | 23 | 30 FPS |
| Idle | 53 | 30 FPS |
| Fire | 10 | 30 FPS |

## Prüfungen und Grenzen

`rc-weapon-animation-check` verwendet die real importierten Clips in ausdrücklich kontrollierten Geschwindigkeits-/Schuss-/Zeitfixtures. Er prüft die 20-Sekunden-Grenze, Rückkehr aus Idle, Gehen/Rennen, unabhängige Basisphase beim Feuern, Schussende/-neustart, Sperren, Neustart, fünf bild-/zustandsexakte Checkpoints, 60 Schritte exakten Fortlauf sowie sechs neu gehashte beschädigte Checkpoints mit atomarer Abweisung. Wiederholtes Rendern verändert keinen Zustand. Der volle Schusskanal liefert dieselbe Geometrie wie derselbe originale Fireclip allein. Eine eigene Bibliotheksregression deckt die Geschwindigkeitsschwellen einschließlich vertikaler Bewegung ab.

Diese kontrollierten Darstellungsfixtures ersetzen keinen Missionslauf. Die zentrale Szenenpipeline führt den Prüfer zusätzlich zum unabhängigen normalen A/B/C→Fabrik-Lauf und den Spieleransichten aus. Zwölf ältere Spielstände aus 0.52/0.53/0.54 werden mit der neuen produktiven DLL exakt wiederhergestellt; die drei zugehörigen A-Einstiegsframes sind bytegleich mit den alten Dateien.

Der zentrale Gesamtbau besteht mit je **1.179 Bibliothekstests in Debug und Release**, Clippy, Java-/Parserprüfungen, Android ARM64/x86_64, 30 Szenenstufen, dem regulären A/B/C→Fabrik-Lauf mit 25 Checkpointproben, 39 Spieleransichten und dem neuen Originalclip-Prüfer.

Das normale APK besteht im Android-x86_64-Emulator Paketimport, Wiederaufnahme nach Prozessverlust, kleine Touch-Steuerung, Halbautomatik und alle drei automatischen Kartenwechsel. Der Zusatztest stellt die Waffenkanäle nach kaltem Fortsetzen exakt wieder her und erreicht die 20-Sekunden-Ruhefrist durch gewöhnliches zeitliches Weiterlaufen der App. Diese APK-Prüfung verwendet für die Kartenwechsel echte Replay-Endspielstände und ist kein frei gespielter vollständiger Touch-Durchlauf.

Zusammengehörige Prüflieferung **0.55.0-dc15-animation**:

- [APK](../build/releases/0.55.0/republic-commando-0.55.0-dc15-animation.apk)
- [Szenenpaket](../build/releases/0.55.0/mission1-scenes.rcpack.zip)
- [Prüfbericht](../build/releases/0.55.0/verification-summary.json) und [Quell-/Dateizuordnung](../build/releases/0.55.0/source-and-delivery.json)

APK und Szenenpaket gemeinsam aktualisieren, Paket importieren und eine neue Mission starten. Frühere Szene-/Spielstandpaare behalten ihren alten Darstellungsumfang. Die eingefrorenen früheren Lieferungen werden nicht ersetzt.

Nachweise liegen unter `build/dc15-animation`: `complete/validation.json`, `complete/scenes/runtime-validation`, `complete/scenes/player-presentation`, `complete/scenes/weapon-animation`, `campaign-android/validation.json` und `legacy-052/053/054.json`. Reproduzieren über `build.ps1 -Target All` mit den deklarierten Szeneneingaben; die zusätzliche APK-Prüfung verwendet `scripts/Test-CampaignAndroid.py --expect-semi-auto --expect-weapon-animation` zusammen mit den neuen Artefakt- und Checkpointpfaden.

Nicht enthalten sind vollständiges `Weapon.NormalFire`/`ClientThink`/`Idle.BeginState`, erzwungene Schüsse, Zoom, Tween-Übergänge, zusätzliche Pawnanimationen, sämtliche Notify-/Foley-Skripte, Zielhilfe, Streuung, Rückstoß und ballistische Originaltreue. Der ausgewählte Clipende-/Steuerungshost ersetzt keine vollständige Unreal-VM oder originale native AnimEnd-Dispatchkette. Freier physischer Handy-Gesamtlauf und subjektive Bild-/Hör-/Touch-Abnahme bleiben offen.
