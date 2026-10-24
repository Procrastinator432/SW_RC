# Native Dialogbesitzregel und NoOverride

**Folgestand 0.69.0:** [Poolbereinigung und Fehlerdiagnose](MISSION_AUDIO_POOL.md) ergänzt Wiederverwendung fertiger Slots, Kapazitäts-/Fehlertrennung, Sitzungssperre, Statuszähler und tatsächliche OpenAL-Sättigungstests. Die folgenden Zahlen beschreiben 0.68.0.

Stand: 26.10.2026. **0.68.0-voice-owner** schließt eine Lücke zwischen den vorhandenen Missionsereignissen und dem nativen WAV-Mixer. Das `NoOverride`-Bit war in `RCAUD003` bereits vorhanden, wurde im nativen Adapter aber nach der Eingabeprüfung verworfen. Dadurch konnte eine neue Zeile desselben Besitzers eine noch laufende geschützte Sprachquelle ersetzen.

## Fünf umgesetzte Punkte

1. **Originalflag übernehmen.** Das bereits übertragene Bit wird in wartenden Ereignissen und aktiven Quellen als boolesches `no_override` erhalten. Der binäre Ereignisvertrag, Szenenpakete und Gameplaycheckpoints ändern sich nicht. Unzulässige Flagwerte werden weiterhin vor Zustandsänderungen abgewiesen. Auch beim Versiegeln und Filtern der Java-Mailbox bleibt das Bit erhalten.
2. **Bestehende geschützte Sprache erhalten.** Für Kategorie 1 wird zunächst derselbe Sprachbesitzer ermittelt. Auf diese bereits zugeordnete Identität wird der vorhandene `audio_pool::select`-Kern angewendet: Schutz der **bestehenden** Quelle verhindert Ersetzung, unabhängig vom Flag der eingehenden Quelle. Eine ungeschützte alte Zeile darf durch eine geschützte neue ersetzt werden. Andere Sprachbesitzer und Effektkategorien bleiben unabhängig.
3. **Abgewiesene Ereignisse einmal konsumieren.** Eine blockierte Zeile startet keine neue physische Quelle. Sie wird aus der Warteschlange entfernt und führt den relativen Tick-/Rendertakt weiter, damit spätere Ereignisse ihren Abstand behalten. Der Annahmezeiger wurde bereits beim gültigen Eingang fortgeschrieben; dieselbe Ereignisnummer kann nicht erneut eingereicht werden. Eine physisch abgeschlossene geschützte Quelle wird vor der nächsten Aufnahme bereinigt und blockiert ihren Besitzer nicht weiter.
4. **Schutz über Pause und Restore bewahren.** RAM-Snapshots enthalten das Flag für aktive und wartende Dialoge. Nach Kontextwechsel gilt die Regel weiter. Ältere Snapshots ohne das zusätzliche Feld sind weiterhin lesbar und erhalten `false`; ein damals nicht gespeicherter Schutz wird nicht erfunden. Ein nicht boolesches Feld wird vor Backendänderungen abgewiesen. Benutzermischwerte bleiben wie zuvor getrennte aktuelle Einstellungen.
5. **Grenzen und tatsächliche Ausgabe prüfen.** Neun neue Rust-Gruppen prüfen die vier Flagkombinationen, verschiedene Besitzer, Effektkategorien, physische Abschlüsse, Konsum/Tickabstände, aktive und wartende Restores, ältere Snapshots und falsche Feldtypen. Eine zusätzliche Java-Gruppe prüft das unveränderte Bit bei pausierter Mailboxübergabe samt Kopienbesitz und Abweisung ungültiger Flags. Ein Windows-C-ABI-Prüfer führt die Regel mit einem tatsächlich aus der Mission gewonnenen geschützten WAV-Ereignis und echtem OpenAL aus.

## Originalbezug und Umfang

Der bereits rekonstruierte `FindSource`-Kern (`engine.dll` `103d0d10`) prüft bei passender Identität das Bit 0 der bestehenden Quelle. Dieser Unterzweig wird jetzt für die bereits vom Missionsadapter zugeordnete Sprachbesitzeridentität verwendet. Die zentralen Tests prüfen den vorhandenen Kern weiterhin gegen seine eingefrorenen Originalfälle. In diesem Abschnitt wurde kein neuer Originalfunktionsvergleich erzeugt und keine neue Zahl dekompilierter Funktionen gezählt.

Das ist **keine vollständige Übernahme der originalen Quellenpool-Suche**. Diese beendet die Suche beim ersten freien Slot; Prioritäts- und Slotregeln sind getrennte Teile. Der Missionsadapter hat weiterhin 16 Quellen, reservierte Dauerquellen und seine bisherige Altersverdrängung. Der neue Schutz gilt beim gezielten Ersetzen desselben Sprachbesitzers; er garantiert keinen generellen Schutz vor Verdrängung bei vollem Pool. Allgemeine Aufnahme-/laufende Priorität, Ducking, vollständige Poolorchestrierung und Vorbis-Musik bleiben offen.

Der Gameplaykern hatte bereits eine eigene ausgewählte Sprachbesitzregel. Die Ergänzung sichert dieselbe Information am tatsächlichen Audioausgang ab, dessen Position durch Queue, Gerätepuffer und Pause von der Gameplayzeit abweichen kann. Sie verändert keinen Missionsziel-, Gegner- oder Spielereingabestatus.

## Prüfungen

Der zentrale Check besteht mit je **1.465 Debug-/Release-Bibliothekstests**, Materialimport, Doctests, Clippy, Java und Buildvertrag. Die Audio-Javaprüfung umfasst jetzt **52 Gruppen**. APK/ELF werden für arm64 und x86_64 geprüft.

Der neue physische Prüfer entnimmt nach **930 normalen GEO_01A-Eingaben** das echte geschützte Ereignis 18, Sound 70, Besitzer 523. Anschließend normalisiert er Ereignisnummer/Tick und variiert Flags/Besitzer als **kontrollierte Audio-Testeingaben**. Der Spielzustand wird dafür nicht manipuliert. Alle vier Flagkombinationen, aktive und wartende Kontextrestores, alte Snapshots, das natürliche Ende des originalen WAV-Clips und unabhängige Besitzer werden mit tatsächlichen AL-Quellen geprüft. Dies ist kein zusätzlicher freier Spieler- oder Hörtest.

Außerdem werden die vier Mischgruppen/26 OpenAL-Sitzungen, Java-Pausenübergabe, realer Dialogrestore, der vollständige Windows-Missionslauf mit 49.795 Eingaben/1.230 Ereignissen/drei Übergängen und 36 ältere Spielstände/neun Einstiegskameraframes mit der neuen DLL erneut geprüft. Die Standardmischwerte bleiben unverändert.

[Zentralcheck](../build/voice-owner/check/validation.json) · [Physische Sprachbesitzprüfung](../build/voice-owner/voice-owner.json) · [Mischpegel](../build/voice-owner/mix.json) · [Pausenübergabe](../build/voice-owner/pause-ingress.json) · [Dialogrestore](../build/voice-owner/dialog-resume.json) · [Vollständige Mission](../build/voice-owner/campaign/mission-audio.json) · [Ältere Spielstände](../build/voice-owner/legacy/validation.json) · [Android-Build](../build/voice-owner/android/validation.json) · [Gesamtnachweis](../build/voice-owner/verification-summary.json)

## Lieferung und offene Abnahme

[APK 0.68.0 für beide Architekturen](../build/voice-owner/android/artifacts/republic-commando-0.68.0-voice-owner.apk) · [Quellstandzuordnung](../build/voice-owner/source-and-verification.json)

APK-SHA-256: `7de177d2fb299ab457c76290ac1e2642a3dafedb4311fec812a4c3083ea5e604`.

Das vorhandene Szenenpaket, die Missionscheckpoints und die gespeicherten Lautstärkeregler können weiterverwendet werden. Im Startmenü den nativen Mixer wählen. Der SoundPool-Vergleichspfad bleibt beim bisherigen Verhalten.

**Android-Laufzeit und Hören stehen weiterhin aus.** Auf dem Gerät insbesondere überlappende Sprachzeilen desselben Besitzers, Pause mitten im Satz, rasches Hintergrund/Fortsetzen und den Kartenwechsel prüfen. Queue-/Trackpuffer können die hörbare Position verschieben. Die RAM-Fortsetzung bildet weiterhin die Rendergrenze ab; Prozessverlust speichert keine halbe Dialogzeile. Die Meldung eines geschützten Audioereignisses allein beweist keine vollständig originalgetreue Inszenierung oder Mischqualität.

Frühere Lieferungen bleiben erhalten; dieser Abschnitt erzeugt keinen Commit, Tag oder Push.

```powershell
./build.ps1 -Target Check -OutputDirectory build/voice-owner/check
./build.ps1 -Target Android -OutputDirectory build/voice-owner/android -Abis arm64-v8a,x86_64
python scripts/Test-MissionVoiceOwner.py --dll target/release/rc_native.dll --library tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll --package build/linux-sync-20261025/package/artifacts/mission1-scenes.rcpack.zip --inputs build/linux-sync-20261025/complete/scenes/resolved-inputs.json --output build/voice-owner/voice-owner.json
```
