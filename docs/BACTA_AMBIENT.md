# Bacta: Original-Dauerton und Effektgrundlagen

Stand: 17.10.2026. Der Original-Dauerton ist angeschlossen; die sichtbaren Partikeleffekte sind noch nicht integriert. Der gesamte zweite Handytest-Korrekturblock bleibt offen.

## Laufende Wiedergabe

`BactaDispenserGEO.DispenserEffectClass` verweist auf `CTEffects.BactaZap_Dispenser`. Dessen tatsächlicher `AmbientSound` ist `GEO_Sound.GEO_03.ele_GEO_genEnrgy_lp_01`. Der Bactabuilder löst diese Bindung erneut aus den Originaldefaults auf und übernimmt die unveränderten PCM-Daten mit Quellen-/PCM-Fingerabdruck. Die WAV-Datei enthält62.472 Samplebytes und keine separaten Loopmarker. Die bereits vorhandenen Intro-/Dialog-/Foleyrollen bleiben erhalten; die neue Rolle ist90.

Die Originalfolge `Load → FinishAnim → SpawnAttachment('Eye',...)` bestimmt den Start. `EndState` zerstört das Eye-Attachment; deshalb endet dessen Dauerton sofort beim Loslassen oder bei voller Heilung. Der Body-Effekt hat keinen eigenen AmbientSound. Sein späteres Auslaufen darf diesen Ton nicht verlängern.

Die native Laufzeit liefert über `rc_game_audio(kind=3)` einen **aktuellen Sollzustand** statt einmaliger Start-/Stop-Ereignisse. `RCLOOP01` enthält einen u32-Zähler und je Loop u32-Soundrolle sowie i32-Actor-Schlüssel. Die Abfrage ist vollständig lesend: Größenabfragen, kurze Puffer und vollständige Kopien verändern weder Audiocursor noch Checkpoint oder wartende Einmalgeräusche. Rollen werden wie die übrigen Opening-Cues nach ihrer Originalidentität aufgelöst.

Ein Loop ist nur bei laufendem Spiel, lebendem/nicht kampfunfähigem Spieler, gehaltener Benutzung und aktiver Station gewünscht. So können ein neuer Androidhost oder ein geladener Checkpoint den gegenwärtigen Loop wiederherstellen, ohne historische Startbefehle abzuspielen. PCM-Abspielpositionen werden weiterhin nicht gespeichert; die Aufnahme startet nach Wiederaufnahme neu.

Android vergleicht jeden aktuellen Simulationsframe mit seinen laufenden Ambient-Streams. Gleicher Actor und gleiche Rolle behalten ihren Stream; entfernte/geänderte Rollen werden gestoppt. `SoundPool` spielt den Originalclip mit `loop=-1`. Der Port reserviert ihn mit Priorität2 gegenüber den bisherigen Priorität1-Einzeltönen, damit Schüsse den Dauerton nicht verdrängen. Pegel0,35 und fehlende räumliche Abschwächung sind weiterhin die bisherige Androidhost-Policy, keine rekonstruierte native Audioausbreitung.

Pause, Fehler, Szenenwechsel und Schließen stoppen auch diese separat verwalteten Streams. Sie fallen nicht aus dem64-Ereignisring für kurze Sounds. Veraltete oder reine Pausenbilder setzen keinen neuen Loopzustand. Ein abgewiesenes Snapshotpaket verändert die laufende Zuordnung nicht.

## Ursprüngliche Emitterdaten

`rc-bacta-assets GAME_DATA OUT` liest jetzt die tatsächlichen Komponentenarrays direkt aus den beiden Effektklassendaten und exportiert14 Templates:

| Effekt | Originalobjekte | Komponenten |
|---|---|---|
| BactaZap_Dispenser |757–767| Mist, Beams, FlareA, Holo_circle, Holo_man, Holo_Bar, Holo_text, ArmA, ArmB, Lightning, CU_fx |
| BactaZap_Dispenser_Body |2579–2581| Mist, Spray, Swirls |

Scalarwerte werden über ihre Klassen-/Instanzdefaults aufgelöst. Verschachtelte Bereiche und Kurven liegen mit dekodierten Tags und vollständigen Rohbytes vor; die Basisdefaults von `Emitter`, `ParticleEmitter` und `SpriteEmitter` werden separat erhalten. Die generische Compound-Defaultauflösung wird dadurch nicht pauschal als vollständig unterstützt erklärt.

Die Body-Emitter haben `RespawnDeadParticles=false`; ihre Originallebensdauern reichen beim Nebel bis8 Sekunden, Spray/Swirls beginnen teilweise erst nach1,1 Sekunden. `Emitter.FadeOut` beendet im Original das Nachspawnen und lässt aktive Partikel auslaufen. Ein sofortiges Ausblenden aller Effekte wäre deshalb keine passende Umsetzung. Die räumliche Zuordnung, Spawn-/Kurvenrechnung und sichtbare Renderintegration sind der nächste Abschnitt. Es wird kein erfundener Heilungssprite eingesetzt.

## Nachweise und weitere Arbeit

`Test-BactaLifecycle.py` prüft den unveränderten Stations-/Missionsablauf mit18.829 normalen Eingaben und zusätzlich den gewünschten Loop bei allen fünf gespeicherten Stationsphasen, Suspend, Fortsetzen und Abbruch. Im Referenzlauf ist er bei Tick2789–2969 aktiv, davor/danach nicht. Die Loopabfrage lässt Einmalgeräusche erhalten; alle sechs mechanischen Notifies bleiben einmalig. `AmbientLoopsCheck` prüft idempotente Wiedergabe, Ersetzung, fehlgeschlagene Wiedergabe, Stopp, Pause/Neuaufbau und atomare Ablehnung ungültiger Pakete. Die native Soundprüfung umfasst31 Referenzen und27 eindeutige Originalcues.

Arbeitsnachweise liegen unter `build/bacta-effects`: `original`, `checks`, `android`, `pipeline` sowie native Sound-/Bacta- und Paketberichte. Je1.132 Debug-/Release-Bibliothekstests, Java-/Clippy-/Buildvertrag und Android-Kompilation für ARM64/x86_64 bestehen. Das APK bleibt ein interner Zwischenstand; es gibt noch keine neue Geräte-/Hörfreigabe oder Releaseversion.

Der vollständige21-Stufen-Neubau und25 Zustands-/Bild-/Fortlauf-/Neustartproben A/B/C→Fabrik bestehen. Am endgültigen ZIP sind31 Cuebindungen, beide Droidentöne, Rampenmaterial und vier historische Checkpoint-Restores geprüft. SHA-256 des Arbeits-ZIP: `0c4812ea1320044f01cc1b950a581c0433a820736d0919304d48a2552b04f351`. Die native A-Soundbank hat7.323.062 Bytes und bleibt innerhalb des vorhandenen JNI-Paketlimits. `Test-BactaEffectBindings.py` bestätigt14 Templates, drei Basisdefaults und119 verschachtelte Datenfelder sowie die korrekte Ambientquelle in der gebauten Szene; dieser Nachweis ersetzt keine Partikel-Renderabnahme.

Offen bleiben die sichtbaren Eye-/Body-Effekte mit korrektem Auslaufen, die konservative Stationskollision und anschließend Verwundung/Barrieren sowie Scorchs Bunker, Tür und A→B aus normaler Spieleransicht.
