# Originale Opening-Dialogregeln

16.10.2026. Dieser Abschnitt ergänzt die Quellen-/PCM-Bindungen aus dem zweiten Handytest. Er schließt die nachgewiesene Dialogüberlagerung im GEO_01A-Auftakt; die übrige Verwundungs-/Bacta-/Barriereninszenierung bleibt offen.

## Nachweis im Original

`Gameplay.ACTION_PlaySound.InitActionFor` wählt den Pawn bzw. die ScriptedSequence als Soundquelle oder sucht den ersten Actor mit `SoundSourceTag`. Wartende Aktionen verwenden `GetSoundDuration(Sound) + 0.1`, nicht nur die WAV-Laufzeit.

Der tatsächliche skalare VM-Einstieg `AActor::execPlayOwnedSound`104ea5a0 liest die optionalen Parameter ein. Slot, Volume und Attenuate werden an dieser Stelle nicht in den Aufruf weitergereicht. `bNoOverride=false` setzt das Flag0x2000; `true` lässt es aus. Der Sound selbst liefert zusätzlich sein Looping-Flag.

`UAudioSubsystem::PlayUSound`103d5950 leitet die Quell-ID aus den AudioParams ab: Voice erhält Kategorie-ID10, kombiniert mit dem Actorindex. Music/SFX erhalten generierte unabhängige IDs. Bei fehlendem Flag0x2000 wird Bit0 der Quell-ID gesetzt. `AudioParams.ActorTag` kann vorher die Quelle verankern.

`UAudioSubsystem::FindSource`103d0d10 vergleicht die IDs ohne Bit0. Bei einer passenden laufenden Quelle prüft es das Bit der **bestehenden** Quelle. Ist diese geschützt, wird der neue Aufruf verworfen. Andernfalls wird die bestehende Quelle ersetzt. Dies ist keine globale Sprachwarteschlange.

Die entscheidenden Assembleroperanden sind zusätzlich direkt in der lokalen `engine.dll` geprüft:

```powershell
python scripts/Verify-OpeningAudioProof.py "GAME_DATA/System/engine.dll" --output build/presentation-flow/original-audio-proof.json
```

Der Prüfer verwendet nur die Python-Standardbibliothek, prüft die Originalbytes an104ea69c und103d0da1 und hält den SHA-256 der untersuchten Binärdatei fest. Ghidra-Exporte des gesamten verfolgten Pfads liegen als Arbeitsnachweise unter `build/presentation-flow`, einschließlich des OpenAL-Treibers. Die entscheidende Schutzbit-Prüfung hängt nicht vom teilweise ungenauen Ghidra-C-Pseudocode ab.

## Umsetzung

Die deklarierte Stufe `map-a-audio` liest alle13 originalen `ACTION_PlaySound`-Aktionen des ausgewählten Auftakts erneut aus der Karte. Sie prüft die erwartete Original-Soundquelle gegen die fertig geladene Bank, löst Kategorie, SoundSourceTag, AudioParams.ActorTag und Override-Flag auf und setzt die Originalwartezeit. Unklare wiederverwendete Cues mit widersprüchlicher Eigentümerschaft werden abgewiesen.

Die native Simulation entscheidet über die Zulassung. Eine verworfene Aktion behält ihre ursprüngliche Skriptwartezeit, erzeugt aber weder Audioereignis noch Untertitel. Laufende Quellenschutzzeiten gehören zum Checkpointzustand. Pause/Prozesswiederherstellung stoppt wie bisher laufende PCM-Wiedergabe; Suspend entfernt deshalb auch deren Quellenschutz. Eine PCM-Wiedergabeposition wird weiterhin nicht gespeichert.

Die ursprünglichen Export-IDs dienen als stabile Actor-Schlüssel im Port; die prozessabhängigen globalen UObject-Indizes werden nicht übernommen.

Android erhält über `rc_game_audio(kind=2)` das Format `RCAUD002`: die bisherigen24 Bytes pro Ereignis plus Kategorie und NoOverride-Flag als zwei u32. Die erfolgreiche vollständige Abholung verbraucht die Ereignisse; Größenabfragen erhalten sie. Das bisherige `kind=1/RCAUD001` bleibt für vorhandene Werkzeuge erhalten. Der Androidhost stoppt bei einer zugelassenen ersetzenden Voice nur den alten Voice-Stream desselben Akteurs. Andere Akteure sowie SFX/Music bleiben unabhängig. Pause/Neustart leert die Streamzuordnung.

## Konkrete Regression

Die Aktionen2502/2503 starten weiterhin im selben Skripttick.2502 (`CA1G1541`, Sun Fac als Hauptziel) setzt `bDontOverrideOtherSounds=true`. Die folgende2503 (`CA1G1545`, Scannen nach Sun Fac) wird deshalb beim bereits geschützten Sprecher verworfen. Ihre WAV bleibt korrekt gebunden und im Paket vorhanden, wird an dieser Stelle aber nicht abgespielt. Die bisher gleichzeitig angezeigte Scannen-Untertitelzeile entfällt ebenfalls.

`Test-OpeningSoundBindings.py --ticks 2200` prüft die tatsächlichen nativen Ereignisse und den Checkpointzustand:24 korrekte Referenzen, passende PCM-/Skriptlaufzeiten, keine Überlappung der ausgewählten Skriptstimmen desselben Akteurs, verworfene konkurrierende Stimme ohne Untertitel und konsistente Legacy-/erweiterte Ereignisansichten. Rust prüft Schutz/Ersetzung/verschiedene Akteure/Fristablauf und serialisierten Schutz. Java prüft die Streamersetzung, SFX-Unabhängigkeit und das Leeren bei Pause.

Geprüft unter `build/presentation-flow`: jeweils1.130 Debug-/Release-Bibliothekstests, zwei Materialimporttests, Java/Clippy/Buildvertrag, vollständiger19-Stufen-Neubau, A/B/C→Fabrik mit25 Checkpointproben, vier echte alte Snapshot-Restores und native Prüfung des endgültigen ZIP. Die frühere fehlerhafte0.50.0-A-Bank wird vom neuen Audiobuilder wegen der falschen Originalquelle abgewiesen. ARM64- und x86_64-APK kompilieren einschließlich des aktualisierten Audiohosts. Ein physischer Hörtest oder eine vollständige Bacta-/Inszenierungsabnahme wird damit nicht behauptet. Das kompilierte APK ist ein interner Zwischenstand; die letzte freigegebene Lieferung bleibt0.50.0.

## Grenzen und anschließender Bacta-Abschnitt

Fortsetzung: Animation, originale Geräusch-Notifies und Heilungsfreigabe nach `FinishAnim` sind inzwischen integriert. Der folgende Absatz dokumentiert die vorausgehende Extraktion. Effekt-/Loop-Lebensdauer bleibt offen; siehe [Bacta-Laufzeit](BACTA_LIFECYCLE.md).

Diese Umsetzung betrifft die ausgewählten Opening-Skripte. Vollständige räumliche Abstrahlung, zufällige Pitch-/Volumeparameter, alle SoundMultiple-Auswahlregeln und die gesamte native Kanal-/Prioritätsverwaltung sind nicht rekonstruiert.

`rc-bacta-assets GAME_DATA OUT` erzeugt inzwischen die Originaldaten für den nächsten Laufzeitabschnitt: Actor736, Mesh `BactaDispenserGEO`, Clips `Load`/`Holster` mit je30 Frames bei30Hz, sechs originale Sound-Notifies und Knochen `eye`. Die drei Notify-Cues sind `fol_GEN_trapFiddle_01`, `mach_GEN_Bdroid_Dispense_open` und `mach_GEN_Bdroid_Dispense_close`. Die Effekte heißen `CTEffects.BactaZap_Dispenser` und `CTEffects.BactaZap_Dispenser_Body`; der erste verwendet den originalen Ambient-Loop `GEO_Sound.GEO_03.ele_GEO_genEnrgy_lp_01`. Diese Daten sind geprüft, aber die Bacta-Animation, Effekt-/Loop-Lebensdauer und Heilungsfreigabe nach FinishAnim sind noch nicht in die A-Laufzeit integriert. Die zusammengesetzten Emitterdefaults müssen dabei ausdrücklich aufgelöst werden.
