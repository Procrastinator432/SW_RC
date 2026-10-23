# Welt-Audio: originaler Rechenkern

**Nachfolgender Quellbaustein:** [Audio-Geometrie nach 0.60.0](AUDIO_GEOMETRY.md) ergänzt originale BSP-Abfragen, Punktregionen und Radiusfaktoren sowie einen bitgenauen Speichervertrag für den Radiusfortlauf. Die Laufzeitanbindung dieses neuen Bausteins bleibt offen.

Stand dieser Rechenkern-Stufe: 23.10.2026 · nach 0.58.0, damals ohne neue Androidlieferung.

**Nachfolgende Integration:** [0.59.0 / ausgewählte räumliche Dauerquellen](SPATIAL_AMBIENT.md). Der folgende Text beschreibt weiterhin den damaligen isolierten Rechenkern und seine Grenzen.

Der Welt-Audiokern in `rc-package::world_audio` bildet ausgewählte originale PC-Regeln für Radius, Quellenpriorität und Lautstärke nach. Er ist noch nicht mit den Weltquellen der Mission oder dem Android-Mixer verbunden. Die spielbare Prüflieferung bleibt [0.58.0](AUDIO_PARAMETERS.md).

## Fünf abgeschlossene Punkte

1. **Radiusregeln:** `FAbstractSource::UpdateRadii` untersucht und die Rechnung nach der Geometrieabfrage umgesetzt. Der Prioritätsradius wird bei Verdeckung mit dem gelieferten Geometriefaktor skaliert; der Wiedergabe-Zielradius erhält zusätzlich Faktor 0,35. Ohne Verdeckung bleibt der ursprüngliche Radius erhalten. Die Geometrieabfrage selbst ist kein Teil dieses Bausteins.
2. **Lautstärke und Stoppen:** `FAbstractSource::UpdateVolume` umgesetzt: Basislautstärke mit Duck-Faktor multiplizieren, Fade-Ergebnis abhängig vom vorherigen Modus übernehmen, anschließend Fade anwenden oder die Quelle stoppen. Pausierte Quellen, Fade-in und Flag `0x4000` verhindern den ausgewählten automatischen Stopp. Die vollständigen Duck-/Fade-Controller bleiben eigene Arbeit.
3. **Aufnahmepriorität:** `UAudioSubsystem::SoundPriority` einschließlich quadratischem Entfernungsgewicht, Lautstärkeanteil, Slot- und Quellenflags portiert. Das Gewicht hat die Untergrenze 0,01. Es dient der Quellenaufnahme und darf nicht als hörbarer Gain an SoundPool übergeben werden.
4. **Rust-Schnittstelle und Zustandsgrenzen:** Validierte Eingaben, getrennte Typen für Priorität, Radiuszustand und Lautstärkeupdate. Radiuszeit und vorheriges Ziel bleiben serialisierbar; ein Roundtrip erhält den Fortlauf. Ungültige Eingaben verändern den Ausgangszustand nicht. Das ist noch keine Integration in Spiel-Checkpoints.
5. **Originalvergleich und Prüfpfad:** 3.000 tatsächlich ausgeführte x86-Fälle mit bitgenauem Vergleich der jeweiligen f32-Ausgaben und Radiuszeit; sieben neue Bibliothekstests einschließlich Grenzfällen. Die eingefrorenen Ergebnisdaten laufen ohne Originalinstallation im zentralen Debug-/Release-Testpfad mit. Frische Originalausführung kann zusätzlich gegen diese Daten geprüft werden.

## Originalbefunde

Untersucht wurde `GameData/System/engine.dll`, SHA-256 `f8b3fc9d62414828a1c31d8b5f3b2bcb951f7c0f142fdf2a9846c98d7b5d35a5`. Die Adressen gelten ausschließlich für diese PC-Datei; die ebenfalls installierte `System-Scalar/engine.dll` hat andere Adressen.

| Originalfunktion | Adresse | Verifizierter Umfang |
|---|---|---|
| `FAbstractSource::UpdateRadii` | `103d2520` | Rechenregionen nach der Geometrieabfrage; Profiling übersprungen. |
| `FAbstractSource::UpdateVolume` | `103d0c60` | Vollständige Funktion mit ausdrücklich vorgegebenen Duck-/Fade-/Pause-/Stopp-Callbacks. |
| `UAudioSubsystem::SoundPriority` | `103d64e0` | Vollständige Funktion mit vorgegebenem globalem Radiusmultiplikator. |
| `FAbstractSource::UpdatePriority` | `103d50e0` | Untersucht, noch nicht portiert; unterscheidet sich von der Aufnahmepriorität. |
| `FAbstractSource::UpdateBase` | `103d5340` | Aufrufreihenfolge untersucht, noch kein allgemeiner Quellenhost. |

Die Aufnahmepriorität berechnet das Entfernungsgewicht als `clamp(1 - Abstand² / (Radius × Multiplikator)², 0.01, 1)`. Radius 0 und lokale Quellen mit Flag `0x100` erhalten Gewicht 1. Der Lautstärkeanteil ist bei Lautstärke 0 ebenfalls 0, andernfalls `0.15 × clamp(Lautstärke, 0, 1) + 0.85`. Hinzu kommen originale Flag-/Slotbeiträge. Obwohl der Decompiler hier einen Integer-Zwischenwert vermutet, speichert der Maschinencode einen Float. SSE-Zwischenrundungen und der abschließende x87-Summenpfad werden getrennt berücksichtigt.

Die Radiusglättung verwendet Enginezeit und den Zeitpunkt der letzten Zieländerung, nicht schlicht `DeltaSeconds`. Der Originalzweig interpoliert auch dann, wenn der gespeicherte Änderungszeitpunkt positiv und die verstrichene Zeit größer als eine Sekunde ist. Das kann extrapolieren. Die Regression bewahrt dieses Verhalten; eine Begrenzung auf 0–1 wäre eine Abweichung. Der Zeitpunkt ändert sich nur bei geändertem Zielradius.

## Reproduzieren

Normale Prüfung ohne Originaldateien oder Python-Zusatzmodule:

```powershell
./build.ps1 -Target Check -OutputDirectory build/world-audio/check
```

Frischer Vergleich mit der lokalen Originalinstallation, Python mit `unicorn==2.1.4` und `pefile==2024.8.26`:

```powershell
python -X utf8 scripts/Test-WorldAudioOriginal.py `
  --game-data 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' `
  --output build/world-audio/original-replay.json `
  --verify-fixture fixtures/world-audio-native.json
```

Das Skript prüft die Original-DLL anhand ihres SHA-256. Die Fixture enthält nur Eingaben, Ergebnisbits und Herkunftsangaben, keine Originalinstruktionen. Tests vergleichen jeweils 1.000 Prioritäts-, Radius- und Lautstärkefälle. Der Vergleich belegt diese Fälle und die genannten Rechenregionen, keine vollständige Engine- oder Mixerparität. Unterstützte Eingabegrenzen und die Abweisung nicht endlicher Resultate sind Schutzregeln des Ports.

## Prüfung und nächster Abschnitt

Der lokale Abschlussnachweis steht unter `build/world-audio`: zentraler Check, frischer Originalvergleich und Quell-/Prüfzuordnung. Die eingefrorene APK und das Szenenpaket von 0.58.0 bleiben ihre bisherige zusammengehörige Lieferung. Für diesen noch nicht angebundenen Rechenkern gibt es keine neue Android- oder Hörabnahme.

Als Nächstes: den ursprünglichen OpenAL-Ausgabepfad für Distanzdämpfung, Koordinaten und Panning ermitteln; dann ausgewählte Weltquellen samt importierten Params und Listener anschließen. Danach Quellenlebensdauer und kaltes Fortsetzen integrieren, neue passende Szenen/APK bauen und die echte Android-Ausgabe prüfen. Geometrische Verdeckung, Doppler und materialabhängige Schritte bleiben getrennt nachvollziehbare Teilaufgaben.
