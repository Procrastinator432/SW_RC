# Hände und Rüstung beim Abseilen

Stand: 28.10.2026 · **0.77.0-rappel-arms / Code 81**.

Beim Herunterschauen während der Abfahrt sind jetzt Hände und Unterarme mit der originalen Delta-38-Rüstung sichtbar. Der Adapter verwendet die vorhandene Spieleranimation und verändert weder Kamera noch Eingaben. Bei geradem Blick können die Hände je nach Pose außerhalb des Bildes liegen; es gibt keine künstlich ständig eingeblendeten HUD-Arme.

[APK für ARM64 und x86_64](../build/rappel-arms/android-final/artifacts/republic-commando-0.77.0-rappel-arms.apk) · [Neues Szenenpaket](../build/rappel-arms/package/mission1-scenes.rcpack.zip) · [Gesamtnachweis](../build/rappel-arms/verification-summary.json)

**APK und Paket gemeinsam aktualisieren**, Paket importieren und **Neue Mission starten** wählen. Ältere Pakete und Checkpoints bleiben lesbar und behalten ihre bisherige Darstellung. Das neue Paket benötigt 0.77; Fortsetzen eines alten gespeicherten Szenenstands aktualisiert dessen Assets nicht.

## Umsetzung

Die vorhandene vollständige Spielfigur enthält 3.500 Dreiecke. Eine vorbereitete Sichtbarkeitsmaske behält **1.150 Arm-/Handdreiecke**. Jeder beteiligte Skinning-Einfluss muss zu den Schulter- und Armnachfahren gehören; gemischte Arm-/Torsoflächen werden konservativ verworfen. Die starren Kopf- und Rumpfsektionen bleiben ausgeschlossen. Kopierte Skinning-Vertices übernehmen dieselbe Sichtbarkeit wie ihre gecachte Quelle.

Die Körperanimation hängt an der aktuellen Kollisionskörperposition. Ihre Root-Translation wird in der Pose gesperrt, weil sie den Spieler bereits bewegt hat. Die Platzierung verwendet den vorhandenen MeshToWorld-Kern mit seiner Höhenkorrektur und der stehenden Körperhöhe des ausgewählten Hosts. Damit werden weder die Bewegung doppelt angewendet noch Kopf/Torso vor die Kamera gesetzt.

Das bisherige Spielermaterial war tatsächlich **CloneCommandoProxy**, eine 64×64-Platzhaltertextur. Der Paketbuilder ersetzt es für diese Darstellung durch **CloneTextures.CloneTextures.CloneCommando38_Shader**. Das vorhandene Originalmaterial wird mit dem etablierten Materialimport aufbereitet; seine belegte deckende Behandlung erhält die RGB-Farben und setzt Alpha deckend. Weltmaterialien und das bisherige Waffenmodell bleiben unverändert.

Die neue Paketoption `rappel.show_arms` aktiviert die Darstellung. Fehlt sie, bleibt das bisherige Verhalten erhalten. Arme werden ausschließlich während des gestarteten, noch laufenden Spielerclips gezeichnet und enden vor der anschließenden normalen Waffenansicht. Es gibt keinen neuen Timer, Sound- oder Notify-Dispatch und keine neue Checkpointzustandsvariable.

**Dies ist weiterhin ein ausdrücklicher Portadapter.** Die Sichtbarkeitsauswahl und die Delta-38-Materialbindung sind gewählte Ergänzungen. Die ursprüngliche eigene Körperdarstellung ist durch `bOwnerNoSee` ausgeblendet; ihre ursprüngliche Skinzuweisung ist damit nicht nachgewiesen. Die Lieferung behauptet weder eine vollständig rekonstruierte Original-Egoansteuerung noch eine physikalische Hand-Seil-Kopplung für jede Pose. Die originale Rope7-Receiverfrage bleibt offen.

## Spieleransichten

[Vorher beim Herunterschauen](../build/rappel-arms/views-down/a-420-before.png) · [Nachher](../build/rappel-arms/views-down/a-420-after.png) · [Produktive Android-Ansicht](../build/rappel-arms/android-runtime/rappel-paused.png)

Der Variantenlauf verwendet gewöhnliche zusätzliche Blickeingaben: bei Tick 270 um 0,55 rad nach unten und bei Tick 550 zurück. Beide Vergleichsinstanzen erhalten identische Eingaben. Kein Spielzustand wird für die Bilder verändert. Im unveränderten geraden Replay sind die beobachteten Bilder weiterhin gleich; das ist bei außerhalb des Blickfeldes liegenden Händen beabsichtigt.

## Prüfungen

- Zentralcheck: **je 1.524 Debug-/Release-Bibliothekstests**, Materialimport, Doctests, Clippy, Inspectorprogramme, Java und Buildvertrag. Drei neue Gruppen prüfen gemischte Knocheneinflüsse, Skin-Cachekopien und fehlerhafte Records/Referenzen. Die Clippy-Beanstandung des ersten Versuchs ist korrigiert; ausgeliefert wird der abschließend geprüfte Quellstand.
- Zwei direkte A-Vergleiche mit **je 19.319 Eingaben, 93 identischen Gameplayzuständen und 459 identischen Audioereignissen**: einmal normale Blickführung, einmal die dokumentierte Blickvariante.
- **Zehn kalte Animationsrestores** liefern dieselben Checkpointbytes, Bilder und jeweils dieselbe Fortsetzung mit dem nächsten echten Input. Der tatsächlich suspendierte Arm-Zwischenstand behält seine Pose.
- Vollständiger C-ABI/OpenAL-Missionslauf: **49.795 Eingaben**, 1.230 WAV-Ereignisse, drei Übergänge und vier kalte Wiederherstellungen.
- **36 ältere Checkpoints und neun Einstiegskameraframes** bestehen. Ein zusätzlicher echter Abseilstand aus 0.76 wird mit exakt denselben Checkpointbytes und demselben Spielerbild geladen.
- Wiederholter Paketbau ist einschließlich ZIP bytegleich. B, C, Fabrik und die inneren A-Szenenschichten sind bytegleich; auf A ändern sich die Armoption, das ausgewählte Spielermaterial und die Herkunftsnachweise.
- ARM64-/x86_64-Build und ELF-/JNI-Prüfung bestehen.

[Zentralcheck](../build/rappel-arms/check-final/validation.json) · [Gerader Blick](../build/rappel-arms/views-default/validation.json) · [Gesenkter Blick](../build/rappel-arms/views-down/validation.json) · [Mission](../build/rappel-arms/campaign/mission-audio.json) · [Ältere Stände](../build/rappel-arms/legacy/validation.json) · [Stand aus 0.76](../build/rappel-arms/legacy-076.json) · [Android-Build](../build/rappel-arms/android-final/validation.json) · [Android-Laufzeit](../build/rappel-arms/android-runtime/validation.json)

Die Android-Prüfung importiert das Paket über den produktiven Dateidialog. Ein echter bei Tick 420 suspendierter Windows-Replaystand mit gesenktem Blick prüft die sichtbaren Rüstungshände, GPU-Laden, Prozessverlust und Animationsfortschritt. Die vollständigen pausierten Screenshots vor/nach Prozessverlust sind pixelgleich. Anschließend werden B/C-Musik und Fortsetzen geprüft. Die Checkpoints sind ausdrücklich injizierte echte Replaystände; dies ersetzt keine freie physische Handy-/Hörabnahme.

Die bisherigen Seil-, Material- und 42 A-Sprachbindungen bleiben erhalten. Noch offen sind Original-Receiver-/Ego-Parität, freie Kamera-/Hand-Seil-Abnahme auf dem Handy, gemeldete Sprachüberlagerungen im schnellen Spielerweg und die anfänglich vermisste A-Musik. Die neuen Maskentests sind Porttests und erhöhen keine Zahl ausgeführter Originalfunktionsvergleiche.

## Reproduzieren

```powershell
cargo build --release --offline --locked -p rc-inspect --bin rc-rappel-arms-build
python scripts/Build-RappelArms.py --builder target/release/rc-rappel-arms-build.exe --game-data "D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData" --package build/rappel-visual/package/mission1-scenes.rcpack.zip --output build/rappel-arms/package
./build.ps1 -Target Check -OutputDirectory build/rappel-arms/check-final
./build.ps1 -Target Android -OutputDirectory build/rappel-arms/android-final -Abis arm64-v8a,x86_64
```

Vorherige Lieferungen bleiben erhalten. Kein Commit, Tag oder Push wurde erzeugt.
