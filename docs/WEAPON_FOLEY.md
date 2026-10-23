# DC-15s: originale Lauf- und Ruhegeräusche

Dieser Bericht dokumentiert den eingefrorenen Stand 0.56.0. Die anschließende originale Variantenwahl ist in [SoundMultiple / 0.57.0](SOUND_SELECTION.md) umgesetzt.

## Fünf Arbeitspakete

1. **Originalzuordnung:** `rc-weapon-notify-assets` und der gemeinsame Importer lesen sämtliche sechs Notifies der installierten DC-15s-HUD-Clips aus `Animations/hudarms.ukx`. Objektidentität, Klasse, Property-Payload und unveränderte Zeitbits werden in `weapon-notifies.json` festgehalten. Unbekannte neue Lauf-/Ruhe-Notifyklassen brechen den Import ab.
2. **Soundimport:** Zwei Lauf-Notifies (`AnimNotify_Sound574/587`, Exporte 32/33) binden `GEN_Sound.Foley_Movement.cc_armor_walk`; der Ruhe-Notify (`AnimNotify_Sound214`, Export 37) bindet `fol_GEN_dc15_idle_01`. Unveränderte PCM-Daten, Quellreferenzen und Fingerprints sind im Beleg enthalten.
3. **Laufzeit und Soundbank:** Die drei Ereignisse werden beim tatsächlichen Überschreiten der Originalzeit auf dem Basiskanal ausgelöst, auch wenn der unabhängige Schusskanal aktiv ist. Der Laufclip enthält zwei Geräusche pro Durchlauf. Gesperrte Waffen erzeugen keine neuen Lauf-/Ruhegeräusche. Die zwei Soundreferenzen werden an die bestehende Bank angehängt; vorhandene Dialog-, Waffen-, Gegner- und Effektrollen bleiben stabil. Android verwendet das bestehende konsumierbare Audioereignisprotokoll.
4. **Speichern und Fortsetzen:** Die bereits gespeicherten Clipframes und Entryflags sind die Ereignisposition; kein zweiter Timer oder separater Notifyzähler wird eingeführt. Ein Fortsetzen vor dem Übergang erzeugt den Ton einmal; ein Fortsetzen danach löst ihn nicht erneut aus. Wartende Audioereignisse bleiben Teil der vorhandenen Queue. Für neue Foley-Szenen wird sie nach jedem Simulationsschritt auf die neuesten 256 Ereignisse begrenzt, einschließlich früher Rückkehr bei Filmsequenzen oder Missionsende. So können auch ältere Dialogproduzenten die gemeinsame Queue nicht überfüllen. Ungültige Soundrollen werden vor einer Checkpointübernahme verworfen. Alte Szenen ohne optionales Foleyprofil behalten ihr bisheriges Verhalten.
5. **Prüfung und Lieferung:** Die zentrale Missionspipeline führt `rc-weapon-foley-check` für A, B, C und den Fabrikeinstieg aus. Der Prüfer verwendet Originalclips und Original-PCM in ausdrücklich kontrollierten Darstellungsfixtures; unabhängige normale Missionseingaben prüfen weiterhin die gesamte Kartenkette. APK-Prüfung: `scripts/Test-CampaignAndroid.py --expect-semi-auto --expect-weapon-animation --expect-weapon-foley`.

## Originale Zeitpunkte

| Clip | Notify | Normalisierte Zeit |
|---|---|---:|
| Run | Sound574 | 0,004347826354205608 |
| Run | Sound587 | 0,45652174949645996 |
| Idle | Sound214 | 0,00001886792597360909 |

Die sehr kleinen positiven Entryzeiten müssen im ersten Animationsschritt genau einmal erkannt werden. Der vorhandene Frame-/Notifyhost verwendet die feste 60-Hz-Simulation und die originalen 30-FPS-Clips.

Die drei `ThrowGrenade`-Notifies (Sound217, ReleaseGrenade6, Sound218; Exporte 34–36) bleiben vollständig beim bestehenden Wurfhost. Seine Foley- und Freigabe-Ereignisse werden nicht zusätzlich durch das neue Profil erzeugt. Fire, Walk, ActionBreathe und Reload enthalten in diesem Animationssatz keine Notifies. Es werden keine weiteren Töne hinzuerfunden.

## Grenzen

`cc_armor_walk` ist eine originale `SoundMultiple`-Gruppe mit acht Varianten. Der Import nutzt ausdrücklich die vorhandene begrenzte Auswahl des ersten Originalkindes `fol_GEN_ccFol_wlkArm_01`. Native Zufalls-/Likelihood-Auswahl, ursprüngliche räumliche Lautstärke-/Radius-/Pitchregeln und vollständige AnimNotify-/Scriptdispatch-Parität werden nicht behauptet. Das ist Rüstungs-Foley aus dem Waffenclip; bodenmaterialabhängige Schritte sind ein eigener Pfad.

Eine gespeicherte Audioqueue kann wartende Ereignisse erhalten; die Abspielposition eines bereits laufenden kurzen PCM-Tons wird weiterhin nicht gespeichert. Android-SoundPool-Aufruf und erfolgreiches Sampleladen sind prüfbar, ersetzen aber keine subjektive Hörabnahme. Der freie physische Handy-Gesamtlauf bleibt offen. Zoom, Tweening, Pawnanimationen und die vollständige Waffen-Zustandskette sind weitere technische Schritte.

## Prüfstand

Der vollständige Zentralbau besteht mit je **1.180 Bibliothekstests in Debug und Release**, Clippy aller Targets, Java-/Parserprüfungen, Android ARM64/x86_64 und 30 Szenenstufen. Die normale Eingabekette A/B/C→Fabrik, 25 Checkpointproben, 39 Spieleransichten und die bisherigen Karma-/Waffenanimationsprüfer bestehen. Alle vier neuen Foley-Prüfläufe bestätigen die Originalbindung, fünf Laufgeräusche in 101 kontrollierten Ticks, paralleles Feuern, stille Sperren, einmaligen Idle-Eintritt, exakten Fortlauf nach Wiederherstellung, fünf abgewiesene beschädigte Profile sowie atomar abgewiesene ungültige Ereignisgruppen und gespeicherte Soundrollen.

Der zusätzliche native Audio-ABI-Lauf prüft die vollständige exportierte Bank jeder Karte und die beiden angehängten PCM-Samples gegen ihre Originalfingerprints. Sechzehn historische Spielstände aus 0.52/0.53/0.54/0.55 und vier alte Einstiegsframes bleiben exakt. Der tatsächliche Java-GPU-Parser liest alle 39 Spielerframes und die native Welt unter 96 MiB Heap.

Die normale APK besteht auf dem Android-x86_64-Emulator Import, kleine Touch-Steuerung, Halbautomatik, echten Hintergrund-Prozessverlust, kaltes Fortsetzen und alle drei Kartenwechsel. Der neue Zusatztest bestätigt die geladene Samplebank, Rüstungs-Foley nach tatsächlicher Touchbewegung und einen erfolgreichen SoundPool-Aufruf für den Ruheton genau am gespeicherten Animations-Eintrittstick nach gewöhnlichem Ablauf der originalen Ruhefrist. Das ist keine subjektive Hörabnahme und kein frei gespielter vollständiger Touch-Durchlauf; Kartenwechsel nutzen echte Replay-Endspielstände.

Der Gesamtlauf deckte außerdem eine bisherige Annahme im Geschütztutorial auf: Seine Suche nach dem Tutorialdialog indizierte jede Audioqueue-Rolle ungeprüft in der ursprünglichen Missionsbank. Sie prüft jetzt nur tatsächlich zu dieser Bank gehörige Referenzen; angehängte Foley-Rollen können keinen Indexfehler auslösen. Ein weiterer Langlaufbefund betraf unbeschränkte ältere Dialogproduzenten: Neue Foley-Szenen begrenzen deshalb die gesamte Audioqueue nach jedem Simulationsschritt auf 256 Ereignisse. Eine eigene Bibliotheksregression prüft diesen späten Dialogpfad und den unveränderten Legacy-Pfad.

Zusammengehörige Prüflieferung **0.56.0-weapon-foley**:

- [APK](../build/releases/0.56.0/republic-commando-0.56.0-weapon-foley.apk)
- [Szenenpaket](../build/releases/0.56.0/mission1-scenes.rcpack.zip)
- [Prüfbericht](../build/releases/0.56.0/verification-summary.json) und [Quell-/Dateizuordnung](../build/releases/0.56.0/source-and-delivery.json)

APK und Szenenpaket gemeinsam aktualisieren, Paket importieren und eine neue Mission starten. Frühere Szene-/Spielstandpaare behalten ihren bisherigen Umfang. Die eingefrorenen älteren Lieferungen werden nicht ersetzt.

Nachweise unter `build/weapon-foley`: `complete/validation.json`, `complete/scenes/weapon-foley`, `complete/scenes/runtime-validation`, `complete/scenes/player-presentation`, `native-audio.json`, `legacy-052/053/054/055.json`, `gpu-java.log` und `campaign-android/validation.json`. Zentral reproduzierbar über `build.ps1 -Target All` mit den deklarierten Szeneneingaben, danach der oben genannte APK-Zusatztest.
