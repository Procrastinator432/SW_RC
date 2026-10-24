# Waffenübergang nach dem Landungsbriefing

Stand: 28.10.2026 · **0.78.0-weapon-raise / Code 82**.

Am Ende des Briefings wird die DC-15 nun mit ihrem Originalclip hochgenommen. Das zugehörige originale Ladegeräusch stammt aus dem Animationsnotify. Der Übergang ersetzt das sofortige Einblenden der Ruhepose.

[APK für ARM64 und x86_64](../build/weapon-raise/android-final/artifacts/republic-commando-0.78.0-weapon-raise.apk) · [Szenenpaket](../build/weapon-raise/package/mission1-scenes.rcpack.zip)

APK und Paket gemeinsam aktualisieren, das Paket importieren und **Neue Mission starten** wählen. Alte gespeicherte Szenen behalten ihre bisherige Darstellung; sie bleiben lesbar.

## Originalbeleg und Umsetzung

`PlayerController.Briefing.EndState` ruft `Pawn.Weapon.BringUp(None)` auf. Der ausgewählte `WT_Secondary`-Zweig gelangt über `Active.BeginState` und `PlaySelect` zu `LoadAnim`. Für die ursprüngliche DC15s ist das `Load`: 18 Frames bei 30 fps. Das Notify in Export 29 verweist auf `GEN_Sound.Foley_Movement.fol_GEN_dc15Load_01`.

Der Paketbuilder übernimmt diesen Clip aus `hudarms.HudArmsDC15sSet` und importiert das Soundnotify mit dem vorhandenen Foleyimport. Die bisherigen Soundbankeinträge behalten ihren Index. Ausschließlich A erhält die zusätzliche Option `dc15.animation.raise_after_briefing`; B, C und der Fabrikeinstieg bleiben bytegleich.

Im ausgewählten Replay kommt `EndBriefing` bereits während der letzten Abseilschritte. Deshalb startet die Waffenanimation direkt an diesem Ereignis und läuft während der restlichen Bewegungssperre weiter. Die Abseilarme enden an diesem Übergang, damit kein zweites Paar Arme die Waffenansicht überlagert. Eine zweite Uhr oder neue Checkpointzustandsvariable ist nicht erforderlich. Alte Pakete ohne die Option behalten den bisherigen Ablauf.

Das ist der ausgewählte visuelle und akustische BringUp-Pfad. Die vollständige ursprüngliche `Active`-Zustandsmaschine mit `bWeaponUp`, Netzrollen und gepufferten Auslösern ist weiterhin offen. Bestehende Spielereingaben für Schießen, Nachladen und Granaten behalten ihre bisherigen Regeln und können eine angenommene Waffenaktion auslösen.

## Prüfverfahren

`Test-WeaponRaise.py` vergleicht den echten A-Eingabelauf mit dem vorherigen Paket. Nur Waffenpose sowie Ereignisqueues und Audiozähler werden vom Gameplayvergleich ausgenommen. Der Originalnotify muss genau einmal auslösen. Gespeicherte Zwischenbilder werden in neuen Instanzen geladen und über weitere echte Eingaben mit dem ununterbrochenen Lauf verglichen. Zusätzliche gewöhnliche Feuer-/Nachlade-/Granateneingaben prüfen die Interaktion, ohne Checkpointwerte zu erfinden.

`Test-MusicAndroid.py --raise-checkpoint` lädt einen tatsächlich suspendierten Zwischenstand über den produktiven Android-Checkpointweg. Er prüft den unveränderten Pausenstand nach Prozessverlust und die abgeschlossene Animation nach Fortsetzen; anschließend die bestehenden Musik-/Fortsetzungsfälle auf B und C. Das ist ein Emulatornachweis mit eingespeisten echten Replayständen, keine freie Handy- oder Hörabnahme.

[Clip-/Defaultbelege](../build/weapon-raise/original.json) · [Foleybelege](../build/weapon-raise/package/foley.json) · [Replayprüfung](../build/weapon-raise/views/validation.json)

Anfangsmusik auf A, die vollständige ursprüngliche Ego-/Seilbindung und die gemeldeten Sprachüberlagerungen auf dem freien Handyweg bleiben gesonderte offene Punkte.

## Ergebnisse

- Zentralcheck mit je **1.524 Debug-/Release-Bibliothekstests**, Materialimport, Doctests, Clippy, Java und Buildvertrag bestanden.
- Zwei A-Läufe mit je **19.319 Eingaben und 134 übereinstimmenden Gameplayzuständen**, einmal mit normalem und einmal mit gesenktem Blick. Der neue Load-Notify löst je einmal aus. Feuer wird wie bisher angenommen; leeres Nachladen und die zu diesem Zeitpunkt noch nicht verfügbare Granate werden wie im vorherigen Paket abgewiesen.
- **Zehn kalte Animationsrestores** erhalten Checkpointbytes und Bilder sowie jeweils 80 weitere echte Eingaben einschließlich Audio exakt. Der visuelle Übergang läuft von Replaystand 494 bis 527; ab 528 beginnt die normale Waffenansteuerung.
- **36 ältere Spielstände und neun Einstiegskamerabilder** sowie der echte Abseilstand aus 0.77 bleiben byte-/bildgleich.
- Vollständiger C-ABI/OpenAL-Missionslauf mit **49.795 Eingaben, 1.231 WAV-Ereignissen**, drei Kartenübergängen und vier kalten Wiederherstellungen bestanden. Durch den neuen Load-Clip verschieben sich auch spätere private Waffen-Foleyphasen; Audioidentität zum alten Paket wird nicht behauptet.
- Produktiver Android-Paketimport, pixelgleicher Pausenstand nach Prozessverlust mitten im Load-Clip und abgeschlossene Animation nach Fortsetzen bestanden. B/C-Musik samt Pause und kalter Fortsetzung ebenfalls bestanden.
- Paketwiederholung bytegleich; ARM64-/x86_64-Build und ELF-/JNI-Verträge bestanden.

[Gesamtnachweis](../build/weapon-raise/verification-summary.json) · [Zentralcheck](../build/weapon-raise/check-final/validation.json) · [Gesenkter Blick](../build/weapon-raise/views-down/validation.json) · [Kampagne](../build/weapon-raise/campaign/mission-audio.json) · [Android](../build/weapon-raise/android-runtime/validation.json)
