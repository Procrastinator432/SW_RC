# Belegte Pausenquittierung für verworfene Effekte

Stand: 26.10.2026. **0.71.0-pause-receipt** schließt eine konservative Lücke der optionalen Audioergänzung: Ein zuletzt empfangenes Waffen-/Foley-Ereignis wird beim Pausieren bewusst nicht mehr abgespielt. Lag es noch in der Java-Mailbox, konnte der native Zeiger dahinter bleiben. Der strikte Checkpointabgleich verzichtete dann auf die Audiofortsetzung, obwohl die wartenden Dialoge vollständig übergeben waren.

## Fünf umgesetzte Punkte

1. **Native Empfangsquittierung.** `rc_scene_audio_consume` und `NativeMixer.sceneConsume` bestätigen die höchste vom Besitzerproduzenten belegte Ereignisnummer. Die Szene akzeptiert ausschließlich einen monotonen, nicht negativen Java-Long-Wert in einer gültigen Sitzung. Gleiche Nummern sind idempotent; kleinere, zu große, veraltete oder falschen Hosttypen zugeordnete Quittierungen werden abgewiesen. Die Operation erzeugt kein Audio, keinen Rendertakt, kein neues Play und keinen Quellen-/Prioritätswechsel. Wartende Dialoge und ihre Tickabstände bleiben unverändert. Gesperrte Sitzungen akzeptieren keine Quittierung.
2. **Unveränderlicher Mailbox-Beleg.** Versiegelung liefert gemeinsam die gefilterten wartenden Dialogpakete und die höchste Nummer eines tatsächlich erfolgreich angenommenen, nicht leeren Ereignispakets. Geerbter Startzeiger, leere Pakete und zuvor verworfene/geschlossene Übergaben erzeugen keinen Beleg. Die Liste ist unveränderlich; eine zweite Entnahme quittiert nicht nochmals. Es wird weder der globale Spielzeiger geraten noch ein nicht eingetroffenes Paket nachträglich erfunden.
3. **Geordneter Produktionsabschluss.** Der Besitzerproduzent übergibt zuerst sämtliche behaltenen Dialogpakete an Rust. Erst danach bestätigt er den Mailbox-Beleg; anschließend erstellt er Snapshot und Cursor und schließt die Ausgabe. Erst nach erfolgreichem Abschluss kann der bestehende Capture ein persistierbares Ergebnis liefern. Scheitern Dialogübergabe oder Quittierung, werden RAM-/persistente Fortsetzung verworfen und die Sitzung freigegeben.
4. **Nutzung beim Checkpointabgleich.** Der native Snapshot trägt danach auch die belegten bewusst ausgelassenen Effektsequenzen im bisherigen Eingangs-/Verbrauchszeiger. Damit kann die unveränderte strenge Gleichheitsprüfung gegen den gespeicherten Spielcursor erfolgreich sein. Szene, genaue Checkpointbytes, Bankhash und Cursor bleiben Pflichtbindungen. Andere Ursachen einer fehlenden Übergabe werden weiterhin nicht übergangen. Alle aktiven Dialogpositionen, NoOverride, Mischwerte und Poolregeln bleiben erhalten; ein bestätigter Effekt wird nach Restore nicht erneut zugestellt.
5. **Quittierungs- und Prozessprüfungen.** Vier neue Rust-Gruppen und eine ABI-Gruppe prüfen Monotonie, Idempotenz, unveränderte IO/Clock/Timeline, Pending-Restore, Ablehnung wiederholter Eingaben, Fehlersperre, Sitzungs-/Hostgrenzen und anschließend gültiges Rendern. Zwei neue Mailbox- und zwei Produktionsproduzentengruppen prüfen Belegbildung, geerbte/leere/gelöschte Übergaben, Reihenfolge vor Snapshot und Fehler ohne Veröffentlichung. Die tatsächlichen Java-Produzentenbytes gehen zusätzlich an Windows-OpenAL; ein kalter Prozesslauf verwendet ein echtes nachlaufendes Effektereignis aus der Mission.

## Vertrauensgrenze

Die native Quittierung ist ein **interner Besitzerproduzentenvertrag**. Rust kann aus einer Nummer allein nicht prüfen, welche Java-Pakete empfangen wurden. Deshalb ist der Beleg ausschließlich nach validierter Mailboxannahme und nach Übergabe aller darin behaltenen Dialoge zulässig. Der C-/JNI-Aufruf ist kein allgemeiner Befehl „an den aktuellen Spielstand angleichen“. Er bestätigt keine lückenlose Ereignishistorie vor dem Sitzungsbeginn und behandelt keine nie empfangenen späten UI-Callbacks. Der Checkpointabgleich wird nicht gelockert.

`RCPOOL01.last` und das Snapshotfeld `last` bleiben der höchste angenommene beziehungsweise belegte Verbrauchszeiger, nicht die zuletzt hörbar abgespielte Nummer. Die Zähler erfolgreich gestarteter Quellen steigen durch Quittierung nicht. Zeitpunkt des letzten tatsächlich verarbeiteten Wiedergabeereignisses und wartende Dialoge bleiben getrennte Daten. Kein Gameplaycursor wird dadurch verändert.

## Nachweise

Der Zentralcheck umfasst je **1.484 Debug-/Release-Bibliothekstests**, Materialimport, Doctests, Clippy, Java und Buildvertrag. Die Java-Audioprüfung umfasst jetzt **74 Gruppen**. Beide Android-ABIs werden einschließlich neuem C-/JNI-Export geprüft.

Der Java-zu-Nativ-Prüfer erhält Sprache 1/2/4, verwirft Foley 3 und den nachlaufenden Effekt 5, bestätigt den tatsächlichen Mailboxbeleg 5 und stellt anschließend genau die wartenden Dialoge 2/4 wieder her. Paketformat `RCJAV002` enthält diesen Beleg; der Prüfer kann weiterhin ältere `RCJAV001`-Fixtures lesen. Die verwendeten Ereignisse sind kontrollierte Testeingaben mit realer WAV-Bank.

Der neue Fall erreicht nach 227 normalen Eingaben einen Beleg für Effekt 10 bei nativem Zeiger 9. Quittierung 9→10 verändert den Renderstand nicht.

Der kalte Prozessprüfer läuft zweimal: einmal mit dem bisherigen geschützten Missionsdialog, einmal mit einem **tatsächlichen nachlaufenden reinen Effektpaket**, während ein Originaldialog weiterläuft. Die zweite Variante hält genau dieses Paket vor der nativen Übergabe zurück, lässt die produktive Java-Mailbox den Beleg erzeugen und bestätigt ihn nativ. Der native Renderzeiger verändert sich dabei nicht; der Audio-/Spielcursor stimmt danach überein. Separate JVMs speichern/laden die tatsächlichen Speicherklassen; ein neuer nativer Prozess stellt die Dialogparameter/-positionen exakt wieder her und rendert weiter, ohne den Gameplaycheckpoint zu verändern.

Außerdem werden Pooldruck, Sprachbesitz, Pegel/26 OpenAL-Sitzungen, Dialogrestore, vollständiger A/B/C→Fabrik-Lauf sowie 36 ältere Spielstände/neun Einstiegskameraframes mit der neuen DLL erneut geprüft.

[Zentralcheck](../build/pause-receipt/check/validation.json) · [Java-Quittierung bis OpenAL](../build/pause-receipt/pause-ingress.json) · [Kalter geschützter Dialog](../build/pause-receipt/process/audio-checkpoint.json) · [Kalter Dialog mit nachlaufendem Effekt](../build/pause-receipt/process-receipt/audio-checkpoint.json) · [Vollständige Mission](../build/pause-receipt/campaign/mission-audio.json) · [Ältere Spielstände](../build/pause-receipt/legacy/validation.json) · [Android-Build](../build/pause-receipt/android/validation.json) · [Gesamtnachweis](../build/pause-receipt/verification-summary.json)

## Lieferung und Grenzen

[APK 0.71.0 für beide Architekturen](../build/pause-receipt/android/artifacts/republic-commando-0.71.0-pause-receipt.apk) · [Quellstandzuordnung](../build/pause-receipt/source-and-verification.json)

APK-SHA-256: `e9257523a7a0acd9737eb45267dadd03f5722d0273b6532fe82fced57aa93f2f`.

Bestehende Szenenpakete, Gameplaycheckpoints, optionale Audioergänzungen und Lautstärkeregler können weiterverwendet werden. Die Ergänzung entsteht weiterhin nur nach abgeschlossener Pauseübergabe; ein laufender Autosave oder abgebrochener Export darf ohne Audiofragment laden. Rendergrenze, ungehörtes gepuffertes PCM und fehlende Resamplerhistorie bleiben die Grenzen der Fortsetzung.

**Android-Laufzeit, echte Prozessverluste und Hören sind noch nicht abgenommen.** Die Geräteprüfung sollte insbesondere Pause unmittelbar nach Schuss/Foley bei gleichzeitigem Dialog und anschließendes kaltes Fortsetzen umfassen. Originalpriorisierung, Ducking und Vorbis-Musik bleiben getrennte offene Integrationen. Frühere Lieferungen bleiben erhalten; kein Commit, Tag oder Push wird erzeugt.

```powershell
./build.ps1 -Target Check -OutputDirectory build/pause-receipt/check
./build.ps1 -Target Android -OutputDirectory build/pause-receipt/android -Abis arm64-v8a,x86_64
python scripts/Test-AudioCheckpoint.py --dll target/release/rc_native.dll --library tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll --package build/linux-sync-20261025/package/artifacts/mission1-scenes.rcpack.zip --inputs build/linux-sync-20261025/complete/scenes/resolved-inputs.json --output build/pause-receipt/process-receipt --java tools/jdk21/jdk-21.0.12.1+1/bin/java.exe --classes build/pause-receipt/check/java/classes --trailing-effects
```

**Fortsetzung 27.10.2026:** [0.72.0: Missionsmusik und Sprachverständlichkeit](MISSION_SOUNDTRACK.md) erweitert denselben Pausen-/Checkpointanschluss um Musikpositionen, Fade- und Duckingzustände. Die hier dokumentierte 0.71.0-Lieferung bleibt historisch unverändert.
