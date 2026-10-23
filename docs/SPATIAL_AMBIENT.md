# Räumliche Dauergeräusche in GEO_01A

Stand: 23.10.2026 · Zielversion 0.59.0-spatial-ambient.

Der ausgewählte Bacta-Energieton und fünf Feuerquellen erhalten Original-Audioparameter, aktuelle Weltpositionen und räumliche Android-Wiedergabe. Der ausgewählte Umfang sind sechs kontinuierliche Quellen in GEO_01A. Einmalgeräusche, Dialoge, Gegner- und Waffentöne erhalten dadurch noch keinen allgemeinen räumlichen Host.

## Fünf Punkte

1. **Originaler Ausgabevertrag:** `alaudio.dll` untersucht: inverses begrenztes Distanzmodell, Rolloff 1, ursprüngliche Koordinatenumrechnung und Quellenradius. Die Aufnahmepriorität aus dem [vorherigen Rechenkern](WORLD_AUDIO_CORE.md) wird nicht als hörbare Lautstärke verwendet.
2. **Parameterimport:** Sechs Bindungen an die tatsächlich vorhandenen direkten Original-Soundobjekte. Min/MaxVolume, Min/MaxPitch, Radius, Kategorie und Flags werden mit Herkunftsnachweisen importiert. Soundgruppen werden in diesem Importpfad ausdrücklich abgewiesen, statt Gruppenparameter mit einem bereits ausgewählten PCM-Blatt zu vermischen. Die vorhandenen Soundrollen bleiben stabil.
3. **Simulation und Fortsetzen:** Lautstärke und Pitch werden beim Beginn einer Quellenaktivierung einmal gewählt. Ein eigener lokaler appSRand-Strom und die gewählten Werte liegen im Checkpoint. Abfragen, SoundPool-Neuanlage und Konsum der Einmalgeräusche verändern diesen Zustand nicht. Beschädigte Quellenidentitäten, fehlende aktive Quellen und unzulässige Parameter werden vor der Zustandsübernahme abgewiesen.
4. **Native/Android-Verbindung:** Neues lesendes `RCLOOP02` mit Listener, Quellenpositionen und gespeicherten Parametern. Bacta folgt dem originalen Eye-Anschluss; Feuer verwendet die ursprünglichen Actorpositionen. Android aktualisiert Gain und Rate bestehender Streams. Entfernung und Blickdrehung starten eine unveränderte Quelle nicht neu. Pause, Quellenende, Sieg und Kartenwechsel stoppen beziehungsweise rekonstruieren den Sollzustand.
5. **Prüfpfad:** Originalinstruktionen für Koordinaten/Radius, Java-Rechen-/Paket-/Streamtests, gewöhnlicher A-Eingabelauf mit echten gespeicherten Quellenzuständen, kaltes natives Fortsetzen, atomare Fehlerfälle sowie abschließende APK-Prüfung über echten Touch und SoundPool. Die Lieferung wird erst nach bestandenem Gesamtbau und Androidlauf eingefroren.

## Originalbelege und Plattformanpassung

`GameData/System/alaudio.dll`, SHA-256 `60e0c8495a94b297c36bc96f29e4c64a208f020e1ac55f5d6848d5697cff0dcd`:

- `UALAudioSubsystem::Init`, `10007560`: `alDistanceModel(0xd002)` und Rolloff 1.
- `UALAudioSubsystem::SetListenerTranforms`, `10005940`: Positionsachsen X/Z/Y mit Faktor 0,01.
- `UALAudioSubsystem::PlaySource`, `10008800`: lokale Quellen, Referenzradius und Falloffflags.
- `FALSource::Update`, VTable+0x28 → `10006270`: laufende Gain-/Positions-/Radiusaktualisierung; Position X/Z/Y × 0,01, Referenzradius × 0,01.

`engine.dll` mit dem SHA-256 aus [WORLD_AUDIO_CORE.md](WORLD_AUDIO_CORE.md): `UpdateAmbients` (`103d7fc0`) ruft `PlayUSound` auf. `GetAmbientVolume` (`103ba770`) und `UpdateAmbientList` (`103d7800`) gehören zum allgemeinen Ambienthost. Dessen vollständige Aufnahme-/Entfernungs-/Prioritätsverwaltung ist noch nicht übernommen. `FAbstractSource::Init` (`103d2a40`) verändert den Radius für weitere Slot-/Sonderfälle; dieser Block verwendet ausschließlich die sechs importierten positiven Ambientradien.

Das Modell `AL_INVERSE_DISTANCE_CLAMPED` und seine Formel sind in der [offiziellen OpenAL-1.1-Spezifikation, Abschnitt 3.4.2](https://www.openal.org/documentation/openal-1.1-specification.pdf) beschrieben. Bei Rolloff 1 ergibt sich oberhalb des Referenzradius der Faktor `Radius / Entfernung`, darunter Faktor 1. Da Position und Radius im Original denselben Maßstab und dieselbe Achsenpermutation erhalten, kann dieser Quotient direkt in Weltkoordinaten berechnet werden.

Der Androidhost multipliziert diesen Faktor mit der gespeicherten Quelllautstärke und dem vorhandenen Host-Gain 0,35. Stereo verwendet einen expliziten Equal-Power-Adapter; Bildschirm-rechts entspricht `(sin(yaw), -cos(yaw), 0)` im bestehenden Renderer. Das ist keine Rekonstruktion des originalen OpenAL-Treibers, seiner HRTF oder Lautsprecherkonfiguration. Lokale Quellen und Legacydaten behalten gleiche Kanäle. Das importierte Disable-Falloff-Flag deaktiviert die Entfernungsdämpfung, nicht das Panning.

Die ursprünglichen Radien sind **100 für Bacta** und **10 für alle fünf Feuerquellen**. Die gewählten Werte bleiben als Original-Params erhalten; es wird kein künstlich größerer Hörbereich hinzugefügt. Die Bacta-Quelle liegt am animierten Eye, nicht an einer geschätzten Spieler- oder Stationsposition.

## Paket und Zustand

`rc_game_audio(kind=5)`: Kennung `RCLOOP02`, u32 Anzahl, drei f32 Listenerposition und f32 Kamerayaw. Pro Quelle 36 Byte: u32 Soundrolle, i32 Owner, drei f32 Position, f32 Lautstärke, f32 Pitch, f32 Radius, u32 Flags. Flag 1 bezeichnet importierte Parameter, 2 eine lokale Quelle, 4 deaktivierten Falloff. Ohne Profil werden Legacywerte 1/1/0 und Flag 0 übertragen. Größenabfragen und kurze Puffer sind lesend. Das bisherige `RCLOOP01` über Kind 3 bleibt unterstützt; beide Javaformate werden atomar gelesen.

Neue Szenen besitzen optionale `ambient_profiles`, ihre Opening-Spielstände optional `ambient_audio`. Alte Szene-/Spielstandpaare lassen die Felder weg und behalten ihre bisherige Wiedergabe. Neue und alte Szenen werden nicht still migriert. Der Auswahlstrom ist lokal und persistent, nicht die ursprüngliche globale Zufallsaufruffolge. Einmalgeräusche und kontinuierliche Quellen haben getrennte Lebensdauern.

Android-Pause beendet wie bisher eine aktive Bacta-Benutzung. Fortsetzen erfindet keinen weiterhin gedrückten Nutzen-Knopf; eine neue Benutzung ist eine neue Aktivierung. Weiterhin lebende Feuerquellen behalten beim Fortsetzen ihre zuvor gewählten Parameter. Der Host startet deren Schleifen aus dem aktuellen Sollzustand neu; die exakte PCM-Abspielphase wird nicht gespeichert.

## Reproduzieren

```powershell
./build.ps1 -Target All -SceneInputs local-assets/mission1/scene-inputs.json `
  -GameData 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' `
  -OutputDirectory build/spatial-audio/complete

python -X utf8 scripts/Test-SpatialAmbient.py --dll target/release/rc_native.dll `
  --scenes build/spatial-audio/complete/scenes --output build/spatial-audio/native-spatial
```

Original-Koordinaten-/Radiusvergleich mit Python, `unicorn==2.1.4` und `pefile==2024.8.26`:

```powershell
python -X utf8 scripts/Test-SpatialOutputOriginal.py `
  --game-data 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' `
  --output build/spatial-audio/original-output.json
```

Das Skript führt 512 ausgewählte SSE-Rechenfälle aus der Original-DLL aus und prüft Ausgangsbits. Es emuliert weder OpenAL-Mixing noch Listener-HRTF. Die bestehenden 800 Original-Params- und 3.000 Welt-Audiokern-Fälle laufen im zentralen Bibliothekstestpfad mit.

APK-Prüfung erst nach gebautem Paket und nativen Replay-Fixtures:

```powershell
python -X utf8 scripts/Test-CampaignAndroid.py `
  --apk build/spatial-audio/complete/artifacts/republic-commando-0.59.0-spatial-ambient.apk `
  --package build/spatial-audio/complete/artifacts/mission1-scenes.rcpack.zip `
  --checkpoints build/spatial-audio/complete/scenes/runtime-validation `
  --spatial-checkpoints build/spatial-audio/native-spatial `
  --output build/spatial-audio/campaign-android `
  --expect-semi-auto --expect-weapon-animation --expect-weapon-foley --expect-audio-params
```

Der Androidtest nutzt echte Replay-Spielstände und bedient die normale APK: Blickdrehung aktualisiert laufende Feuerstreams, kaltes Fortsetzen erhält Auswahlwerte, gedrücktes Nutzen erreicht den Bacta-Loop. Er ersetzt keinen frei gespielten gesamten Abschnitt und keine subjektive Hörabnahme auf dem physischen Handy.

## Nächste Grenzen

Geometrische Verdeckung/EAX, dynamische Radiusglättung bei wechselnder Verdeckung, originale Ambient-Aufnahme-/Prioritätsverwaltung, Master-/Duck-/Fade-Controller, Doppler, materialabhängige Schritte und ein allgemeiner räumlicher Einmalgeräuschhost bleiben offen. Die früheren fünf Feuerloops leben weiterhin nach dem ausgewählten Effekt-Sollzustand; die neue Entfernung beeinflusst ihre hörbaren Kanäle, führt aber noch keine originalgetreue Distanzaufnahme/-freigabe ein.


## Abgeschlossene Prüfung und Lieferung

Der zentrale Gesamtbau besteht mit je **1.192 Debug-/Release-Bibliothekstests**, Clippy aller Targets, Java, beiden Android-ABIs, 30 Szenenstufen, 25 Missions-Checkpointproben und 39 Spieleransichten. Der native Quellenlauf erreicht mit 19.319 gewöhnlichen A-Eingaben den Sieg. Zwei echte Quellen-Fixtures bestätigen lesende Abfragen, kurze Puffer, unveränderte Auswahl nach kaltem Laden und getrennten Einmalgeräuschkonsum. Fünf beschädigte Profile und je sieben beschädigte Quellen-Spielstände werden abgewiesen. 28 ältere Spielstände und sieben alte Einstiegsbilder bleiben exakt.

Die normale APK bestätigt Paketimport, kleinen Touch-Aufbau, physische Blickbewegung mit Gainupdates ohne Streamneustart, gespeicherte Feuerparameter nach kaltem Fortsetzen, Bacta per gedrücktem Nutzen sowie alle drei Kartenübergänge. Die bisherigen Halbautomatik-/Foley-/Original-Parameterprüfungen bestehen ebenfalls. Dies ist eine Emulatorprüfung mit echten Replay-Spielständen, keine neue freie physische Hör-/Gesamtmission-Abnahme.

Zusammengehörige Prüflieferung **0.59.0-spatial-ambient**:

- [APK](../build/releases/0.59.0/republic-commando-0.59.0-spatial-ambient.apk)
- [Szenenpaket](../build/releases/0.59.0/mission1-scenes.rcpack.zip)
- [Prüfbericht](../build/releases/0.59.0/verification-summary.json) und [Quellzuordnung](../build/releases/0.59.0/source-and-delivery.json)

APK und Paket gemeinsam aktualisieren, Paket importieren und eine neue Mission starten. Ältere Szene-/Spielstandpaare behalten ihren bisherigen Audioumfang. Der neue Quellstand ist lokal; Patch, zusätzliche Quellen und SHA-256-Zuordnung begleiten die Lieferung auf Basis des zuvor gepushten Commits `55afa77`. Die eingefrorene Lieferung 0.58.0 bleibt erhalten.
