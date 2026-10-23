# Aufnahme und Freigabe der ausgewählten Umgebungstöne

Stand: 23.10.2026 · Prüflieferung 0.60.0-ambient-lifecycle.

Die sechs in [0.59.0 angebundenen Dauerquellen](SPATIAL_AMBIENT.md) erhalten einen gespeicherten Quellenlebenszyklus. Neue Weltquellen starten innerhalb des ursprünglichen Aufnahmebereichs. Beim Verlassen des Freigabebereichs blendet der bestehende Stream aus und endet; eine spätere erneute Aufnahme wählt neue Parameter. Eine reine Blickdrehung, Abfrage oder Wiederherstellung würfelt weiterhin nichts neu.

## Fünf umgesetzte Punkte

1. **Aufnahmegrenze:** Originaler PC-Standard `GAudioMaxRadiusMultiplier = 100`. In der ausgewählten unbedeckten Weltquellen-Variante wird bei `Abstand² <= (Params.Radius × 100)²` aufgenommen. Bacta hat damit Aufnahmebereich 10.000, die Feuerquellen jeweils 1.000 Welteinheiten. Das ändert nicht den Referenzradius der hörbaren Distanzdämpfung: dieser bleibt 100 beziehungsweise 10.
2. **Freigabe und Ausblendung:** Eine laufende Quelle beginnt außerhalb von `(Aufnahmeradius + 10)²` eine lineare Ausblendung von 0,3 Sekunden. Die zusätzlichen 10 Einheiten verhindern unmittelbares Ein-/Ausschalten an der Aufnahmegrenze. Eine bereits laufende Ausblendung wird weder durch weitere Abfragen noch durch Rückkehr in den Aufnahmebereich zurückgesetzt. Danach kann eine neue Aufnahme stattfinden.
3. **Aufnahmepriorität:** Die ursprüngliche `SoundPriority`-Rechnung wird beim Quellenbeginn mit dem Ambient-Slot 3 und dem PC-Multiplikator verwendet. Die Priorität bleibt mit der Stimme gespeichert; anstehende Starts werden absteigend danach geordnet, bei Gleichstand stabil nach Owner. Dies ersetzt keinen vollständigen ursprünglichen Quellenallocator oder dessen Verdrängungs-/Warteschlangenpolitik. Der ausgewählte Host verwaltet höchstens sechs solche Quellen; SoundPool behält seine bisherige Streampriorität.
4. **Persistenter Lebenszyklus:** Auswahlseed, gewählte Lautstärke/Pitch, Aufnahmepriorität, Ausblendbeginn, Kurvenwert und abgelaufene Simulationszeit liegen im Checkpoint. Auch eine teilweise ausgeblendete Quelle wird nach kaltem Laden mit demselben Faktor wiederaufgebaut. Quellenende, Missionserfolg und Neustart entfernen die Stimmen. Ungültige Prioritäten und veränderte beziehungsweise zukünftige Ausblendwerte werden atomar abgewiesen.
5. **Original- und Integrationsnachweis:** Eingefrorene Originalfälle, normaler Missionseingabelauf, echte Ausblend-/Rückkehrzustände, historische Szene-/Spielstandpaare und abschließende Androidausgabe. Die normale APK prüft neben den bisherigen Stereo-/Bacta-Proben auch, dass ein kalt geladener Ausblendzustand in der Simulation endet und sein SoundPool-Stream durch die Sollzustandsaktualisierung gestoppt wird.

## Originalbefunde

Verwendet wurden die bereits in [WORLD_AUDIO_CORE.md](WORLD_AUDIO_CORE.md) identifizierten PC-DLLs. Der neue Originalvergleich prüft ihre SHA-256 vor der Ausführung.

| Funktion / Daten | Adresse | Befund |
|---|---|---|
| Core `GAudioMaxRadiusMultiplier` | `101ba518` | Initialwert 100, hier ausdrücklich der unterstützte PC-Standard. |
| `UpdateAmbientList` | `103d7800` | Aufnahme mit `Abstand² <= (Radius × Multiplikator × Geometriefaktor)²`. |
| `UpdateAmbients` | `103d7fc0` | Aktive Quellen: Freigabe außerhalb `(R + 10)²`; bereits ausblendende Quelle nicht nochmals starten. |
| Derselbe Warteschlangenpfad | `103d8633` ff. | Andere Grenze: `Abstand² > R² + 10`. Diese Rechnung ist separat geprüft, noch kein integrierter Quellen-Warteschlangenhost. |
| `StopSound` | `103d6b10` | Fade-out-Modus 2, Dauer 0,3; nicht gestreamte Sounds verwenden lineare Kurvenart 0. |
| Kurvensetter / Kurvenwert | `103d1cf0` / `103d1dc0` | Lineare Steigung und begrenzter Kurvenwert. |
| `GetFadeMultiplier` | `103d2810` | f32-Fortschritt und Abschluss der Fade-out-Phase. |

Die beiden Freigabeformeln dürfen nicht zusammengelegt werden: `R² + 10` und `(R + 10)²` sind verschiedene Grenzen. Auch die Aufnahmegrenze ist inklusive, die Freigabegrenze strikt außerhalb. Die Tests enthalten gezielte Grenzfälle und tatsächliche Originalinstruktionsausführung.

`Policy::distances` kann einen bereits ermittelten Geometriefaktor verarbeiten; im aktuellen Missionshost wird ausdrücklich Faktor 1 verwendet. Eine geometrische Verdeckungsabfrage, originale Zonenhörbarkeit/EAX und die dynamische Radiusglättung sind weiterhin nicht integriert. Die ursprüngliche vollständige Einblend-/Duck-/Masterregelung gehört ebenfalls nicht zu diesem Block; der bisherige Host blendet die hier ausgewählten nicht gestreamten Quellen von seinem aktuellen Vollpegel linear aus.

## Zustands- und Protokollvertrag

Neue Opening-Szenen enthalten optional `ambient_policy`. Nur diese Szenen aktivieren den neuen Lebenszyklus. Alte Szenen einschließlich 0.59.0 lassen das Feld weg und behalten ihre frühere Quellenlebensdauer. Neue optionale Stimmenfelder `priority` und `fade` werden für alte Szene-/Spielstandpaare nicht serialisiert. Es gibt keine stille Migration zwischen unterschiedlichen Szenenidentitäten.

Die gewählte Originallautstärke bleibt unverändert im gespeicherten `playback`. Die räumliche Sollzustandsabfrage übermittelt für eine ausblendende Quelle die effektive Lautstärke `playback.volume × fade.value`, bei unverändertem Pitch und Referenzradius. Der Shader-/Materialpfad und die Einmalgeräuschqueue bleiben davon getrennt.

Die neue lesende Abfrage `rc_game_audio(kind=6)` liefert `RCLOOP03`: Kopf und räumliche Parameter wie bei `RCLOOP02`, zusätzlich pro Quelle eine u64-Aktivierungskennung (`Voice.started`). Ein Datensatz ist damit 44 statt 36 Byte lang. `RCLOOP01` und `RCLOOP02` bleiben über ihre bisherigen Abfragen erhalten. Abfragen und kurze Puffer verändern auch bei Version 3 weder Stimme noch Seed.

Der Androidhost erhält einen Stream nur bei gleicher Kombination aus Owner, Soundrolle und Aktivierungskennung. Eine neue Aktivierung startet denselben Originalton neu, auch wenn innerhalb eines Renderframes eine Ausblendung endet und eine erneute Aufnahme erfolgt. Positions-/Gainänderungen derselben Aktivierung erhalten den Stream. Ungültige Aktivierungskennungen oder Pakete verändern die laufenden Stimmen nicht.

Die Ausblendung verwendet feste Simulationsschritte von 1/60 Sekunde und reproduziert die ausgewählte originale f32-Rechnung. Bei unveränderter 0,3-Sekunden-Dauer endet die Stimme nach 18 Schritten. Hintergrundzeit wird nicht nachgeholt. Der Zufallsstrom wird erst bei einer neuen Aufnahme weitergeschaltet, nicht bei Gainänderung oder Host-Neuanlage. Die ursprüngliche globale RNG-Aufrufhistorie und die exakte PCM-Abspielphase bleiben außerhalb dieses Hosts.

Bei den ausgewählten Ambientquellen bleibt die einmal berechnete Aufnahmepriorität erhalten. Das passt zum untersuchten Ambient-Slot: der allgemeine laufende `UpdatePriority`-Pfad überspringt Slot 3. Eine spätere Aufnahme verwendet die dann gültige Entfernung und einen neuen gespeicherten Parametersatz.

## Reproduzieren

```powershell
./build.ps1 -Target All -SceneInputs local-assets/mission1/scene-inputs.json `
  -GameData 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' `
  -OutputDirectory build/ambient-lifecycle/complete

python -X utf8 scripts/Test-SpatialAmbient.py --dll target/release/rc_native.dll `
  --scenes build/ambient-lifecycle/complete/scenes --output build/ambient-lifecycle/native-spatial
```

Frischer Originalvergleich mit Python, `unicorn==2.1.4` und `pefile==2024.8.26`:

```powershell
python -X utf8 scripts/Test-AmbientLifecycleOriginal.py `
  --game-data 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' `
  --output build/ambient-lifecycle/original-lifecycle.json `
  --verify-fixture fixtures/ambient-lifecycle-native.json
```

800 Eingangsfälle führen je drei ursprüngliche Entfernungszweige aus; dazu kommen alle 18 Originalschritte der gewählten linearen Ausblendung. Die Rusttests vergleichen Ergebnisse und f32-Kurvenbits. Geometrie-/Actorabfragen werden im Originalvergleich als kontrollierte Eingaben geliefert; weder Engine-Allocator noch Audiotreiber werden vollständig emuliert. Die Fixture enthält keine Originalbinärbytes.

Für die APK-Abnahme wird die normale Kampagnenprüfung mit `--spatial-checkpoints build/ambient-lifecycle/native-spatial` verwendet. Dort liegen die aus normalen Eingaben gewonnenen Bacta-, Feuer- und Ausblend-Fixtures. Die allgemeine Anleitung steht in [SPATIAL_AMBIENT.md](SPATIAL_AMBIENT.md); die zusammengehörigen 0.60.0-APK-/Paketdateien und Abschlussberichte sind unten verlinkt.

## Anschließende Arbeit

Als Nächstes sind originale geometrische Hörbarkeit/Zonenfaktoren und die dazugehörigen dynamischen Radien sinnvoll. Danach folgt der allgemeine räumliche Host für Einmalgeräusche. Vollständige Quellenverdrängung/Warteschlangen, alle Kurvenarten, ursprünglicher Mixer/HRTF/Doppler und eine freie physische Hör-/Handyabnahme bleiben eigenständige offene Umfänge.

## Abgeschlossene Prüfung und Lieferung

Der Gesamtbau und der anschließende finale Quellcheck bestehen mit je **1.196 Debug-/Release-Bibliothekstests**, Clippy und Java. Beide Android-ABIs, 30 Szenenstufen, 25 Missions-Checkpointproben, 39 Spieleransichten und die Missionskette A/B/C bis zum Fabrikeinstieg bestehen. Der normale A-Lauf erreicht nach 19.319 Eingaben den Sieg. Acht Aufnahmen einschließlich echter Wiederaufnahmen haben geprüfte Originalprioritäten; alle fünf Feuerquellen zeigen die Freigabe. Drei echte Zustände prüfen Bacta, Feuer und laufende Ausblendung. Fünf beschädigte Szenenprofile und 25 beschädigte Spielstände werden atomar abgewiesen. 32 ältere Spielstände und acht alte Einstiegsbilder bleiben exakt.

Die normale APK bestätigt Paketimport, kleinen Touch-Aufbau, beide Blickachsen und gespeicherte Empfindlichkeit, Halbautomatik, Waffen-Foley und ursprüngliche Audioereignisparameter. Laufende Feuerstreams erhalten Stereo-/Gainupdates; kaltes Fortsetzen erhält Quellparameter, Touch-Nutzen erreicht Bacta. Ein kalt geladener echter Fade endet in der Simulation und stoppt seinen tatsächlichen SoundPool-Stream. Alle drei automatischen Kartenübergänge bestehen. Die Touchprüfung wartet vor der Messgeste auf das geschlossene Pausenmenü und die laufende Ansicht; Sollwinkel und Toleranzen bleiben unverändert.

Zusammengehörige Lieferung:

- [APK 0.60.0](../build/releases/0.60.0/republic-commando-0.60.0-ambient-lifecycle.apk)
- [Szenenpaket](../build/releases/0.60.0/mission1-scenes.rcpack.zip)
- [Prüfbericht](../build/releases/0.60.0/verification-summary.json), [Quellzuordnung](../build/releases/0.60.0/source-and-delivery.json) und [Lieferhashes](../build/releases/0.60.0/SHA256SUMS.txt)

APK und Paket gemeinsam aktualisieren, Paket importieren und eine neue Mission starten. Ältere unveränderte Szene-/Spielstandpaare behalten ihren bisherigen Audioumfang. Der finale Androidbau unter `build/ambient-lifecycle/android-final` ersetzt die frühere APK des Gesamtbaus; das Szenenpaket stammt aus dessen unveränderten abschließenden Szenenstufen. Native Berichte, Paket und tatsächliche APK-Abnahme sind per SHA-256 zugeordnet. Die Lieferung enthält den lokalen Quellpatch, zusätzliche Quellen und Dateihashes auf Basis von `55afa77`; es wurde kein neuer Commit oder Push angelegt. Frühere eingefrorene Lieferungen bleiben erhalten.
