# Nächste intern prüfbare Arbeiten

**Aktuellste Quellfortsetzung 24.10.2026:** [FALSource-Steuerung und Abschlussregeln](AUDIO_DRIVER.md): fünf intern umgesetzte Pakete mit ganzen Originalmethoden und gespeichertem Fortlauf. Nächste Kandidaten sind Buffer-/PCM-Lebensdauer, Streamingnachfüllung und FALSource-Update samt Uhr-/Mixerpfad; echte Treiber-/Appanbindung und Windows-Abnahme bleiben offen.

**Neueste Quellfortsetzung 24.10.2026:** [AL-Upload und PlaySource-Befehlsfolge](AUDIO_UPLOAD.md) erweitert die vorbereitete Aktivierung um fünf intern geprüfte Pakete. Noch offen: echte FALSource-Treibermethoden, PCM-/Buffer-/Streamingbesitz und globale Update-Sortierung/-Budgets. Die älteren Auswahlen unten bleiben historische Ausgangspunkte.

**Weitere Quellfortsetzung 24.10.2026:** [Pause-/Stopbefehle und persistente Warteschlange](AUDIO_COMMANDS.md), fünf intern umgesetzte Punkte mit 1.088 Originalbefehlsaufrufen. Dort ist auch die korrigierte Zuordnung `IsPaused +1c` / `IsQueued +20` dokumentiert. Allgemeiner App-/Treiberanschluss bleibt offen.

**Nachfolgende Quellfortsetzung 24.10.2026:** [Audioquellenverwaltung](AUDIO_SOURCE_RUNTIME.md) ergänzt fünf intern umgesetzte Punkte, neun Testgruppen und 768 originale Zusammenschritte. Allgemeiner App-/Treiberanschluss und Missions-/Hörabnahme bleiben offen.

Stand: 24.10.2026. Auswahl nach dem Audio-Geometrieblock. **Die fünf ausgewählten Kerne sind inzwischen intern umgesetzt und geprüft.** [Implementierung, Originalvergleiche und Integrationsgrenzen](AUDIO_CONTROLLERS.md). Die folgenden Abschnitte halten die ursprüngliche Auswahl und ihren damaligen Ausgangspunkt fest.

Die vollständige Missions-/historische Spielstandprüfung der Geometrieanbindung ist auf Nutzerwunsch bis zur Rückkehr an den Windows-PC zurückgestellt. Sie bleibt eine offene Abnahme und verhindert die folgenden Rechen-/Zustandsarbeiten nicht. [Windows-Prüfliste](AUDIO_GEOMETRY_RUNTIME.md#windows-abnahme-bewusst-zurückgestellt).

Mit „ohne Emulator“ ist hier der Android-Emulator gemeint. Die vorgeschlagenen internen Nachweise verwenden Rusttests und bei Bedarf den bereits eingerichteten lokalen Unicorn-Prüfer für Original-x86-Instruktionen. Dafür laufen weder Android noch eine Mission; es werden keine Audiosamples tatsächlich abgespielt. Eine spätere Hör-/Spielabnahme bleibt erforderlich, sobald diese Kerne an die App angeschlossen werden.

## Ursprüngliche Auswahl der fünf Pakete

| Reihenfolge | Konkrete Umsetzung | Interner Abschlussnachweis | Aufwand / Grenze |
|---|---|---|---|
| 1 · Allgemeine Lautstärkekurven | Kurvensetter/-auswertung für die im Original vorhandenen Arten 0–3 rekonstruieren und als unabhängigen Kern portieren. Aktuell ist nur der ausgewählte lineare Ambient-Fade integriert. | Originalfälle für Start/Ziel, Dauer 0, steigende/fallende Kurven, Umkehr während eines laufenden Übergangs, Endgrenzen und wiederholte unveränderte Abfragen. Gespeicherte Zustände laufen bitgenau weiter. | Gut abgrenzbar. Nichtlineare Hilfsfunktionen und deren ursprüngliche Rundung zuerst klären; keine angenommenen Standard-Easingformeln einsetzen. |
| 2 · Allgemeine Fade-Zustände | Fade-in, Fade-out, Unterbrechung und Abschluss um den Kurvenkern aufbauen. Den vorhandenen Rechenkern `Volume::update` mit echten Controllerergebnissen statt ausschließlich vorgegebenen Fade-/Moduswerten verbinden. | Kontrollierte Zeitfolgen gegen `GetFadeMultiplier`, Übergänge bei Dauer 0 und beim Ende, getrennte Untersuchung gestreamter/nicht gestreamter Zweige, Pause-/Stoppgrenzen sowie kaltes Speichern/Laden. | Mittel; baut auf Paket 1 auf. Zunächst allgemeiner Controllerkern, keine Behauptung einer neuen App-Lautstärkeregelung. |
| 3 · Ducking | Den durch andere Stimmen ausgelösten Lautstärkeabsenkungs- und Rückkehrcontroller sowie die Faktorenwahl pro Quelle portieren. Die aktuelle Lautstärkerechnung erhält `duck` noch als externen Eingabewert. | Originalfolgen mit 0/1/mehreren Ducking-Stimmen, Beginn/Ende und erneuter Unterbrechung, drei Kategorien, Slot-/Quellen-Ausnahmen und exaktem gespeicherten Fortlauf. | Gut intern prüfbar. Konfigurationswerte und Stimmenzählung zunächst explizite Eingaben; globale Original-Stimmenverwaltung erst mit separatem Nachweis. |
| 4 · Laufende Quellenpriorität | `FAbstractSource::UpdatePriority` als eigenen Kern neben der bestehenden **Aufnahmepriorität** portieren. Ambient-Slot 3 behält seinen belegten Sonderpfad. | Originalvergleiche für Entfernung, Radius, Lautstärke, Flags/Slots, Nullabstand und Prioritätswechsel. Aufnahme- und laufende Priorität werden ausdrücklich getrennt geprüft. | Mittel, mit Präzisionsrisiko: Der Originalpfad verwendet `RSQRTSS` samt Verfeinerung. Seed-/CPU-Näherungsgrenze explizit behandeln; ein gewöhnliches `sqrt` belegt keine Bitparität. |
| 5 · Auswahl im Quellenpool | Den Entscheidungskern von `FindSource` für freie/belegte Slots, passende IDs, Schutzbit und Verdrängung durch Prioritäten rekonstruieren. Die bereits vorhandene ausgewählte Sound-ID-/Schutzlogik wird als Teilumfang erhalten. | Künstliche Pools gegen Originalfunktion mit kontrollierten Listen-/Treiber-Callbacks: freier Slot, voller Pool, strikte Prioritätsgrenze/Gleichstand, passende geschützte/ungeschützte ID und Reihenfolge Stoppen→Übernehmen. | Größer als 1–3, aber unabhängig von Szenen prüfbar. Ergebnis ist ein belegter Auswahlkern, kein vollständiger Quellenallocator, Streaming- oder Android-SoundPool-Ersatz. |

Diese Reihenfolge nutzt den vorhandenen Audio- und Originalvergleichsrahmen. Die Pakete sind nach Rechen-/Zustandsumfang zugeschnitten, nicht als fünf gleich große Entwicklungsabschnitte oder Gesamtzählung aller offenen Spielfunktionen.

## Ausgangslage vor dieser Umsetzung

- `crates/rc-package/src/ambient_lifecycle.rs`: Die bestehende `Fade`-Struktur ist fest auf den ausgewählten linearen 0,3-Sekunden-Fade mit 1/60-Sekunden-Schritten begrenzt.
- `crates/rc-package/src/world_audio.rs`: `Volume::update` verarbeitet vorgegebene Duck-/Fade-/Moduswerte; die Controller fehlen dort ausdrücklich. `Priority::admission` ist bereits vorhanden und ersetzt die laufende Quellenpriorität nicht.
- `crates/rc-game/src/ambient_audio.rs`: Der aktuelle ausgewählte Host speichert Aufnahmepriorität und Fade-/Geometriezustände. Er verwaltet keinen allgemeinen konkurrierenden Quellenpool.
- `crates/rc-game/src/opening.rs`: Die ausgewählte Besitzer-/Schutzregel existiert bereits; Paket 5 ergänzt die allgemeine Poolentscheidung statt diesen Teil nochmals als neu zu zählen.

Originalbefunde aus der lokal installierten `engine.dll`, SHA-256 `f8b3fc9d62414828a1c31d8b5f3b2bcb951f7c0f142fdf2a9846c98d7b5d35a5`:

| Funktion | Adresse | Zuordnung |
|---|---|---|
| Kurvensetter / Kurvenwert | `103d1cf0` / `103d1dc0` | Paket 1; Verzweigungen für Arten 1–3 und linearen Fallback vorhanden. |
| `FAbstractSource::GetFadeMultiplier` | `103d2810` | Paket 2; ausgewählter linearer Fade bereits originalgeprüft, allgemeiner Controller noch offen. |
| `FAbstractSource::GetDuckMultiplier` | `103d2920` | Paket 3; Kategorie-/Slot-/Quellen-Ausnahmen. |
| `UAudioSubsystem::UpdateDuckVolume` | `103d1210` | Paket 3; Zustandswechsel bei Änderung der aktiven Ducking-Stimmen und laufende Kategorienfaktoren. |
| `FAbstractSource::UpdatePriority` | `103d50e0` | Paket 4; Ambient-Bypass und eigener Entfernungs-/Näherungspfad. |
| `UAudioSubsystem::FindSource` | `103d0d10` | Paket 5; freie Slots, ID-/Schutzprüfung und Auswahl einer streng niedrigeren Priorität. |

Die Funktionen für Pakete 1, 3, 4 und 5 wurden für diese Auswahl neu dekompiliert; das beweist ihre Verfügbarkeit und die genannten Kontrollpfade. Zum Zeitpunkt der Auswahl war dies noch kein ausgeführter Originalvergleich. Die inzwischen bestandenen 7.048 Vergleiche sind im [Umsetzungsbericht](AUDIO_CONTROLLERS.md) dokumentiert. Private Dekompilate bleiben unter `analysis/decompiled/next-internal-audio.c`; der lokale Auswahlbeleg liegt unter `build/next-internal-work/assessment.json`.

## Verwendete Abnahmeregel für diese Pakete

Jeder neue Kern erhält zuerst Originalbefund und eingegrenzten Funktionsvertrag, dann eingefrorene synthetische Originalfälle, Rust-Implementierung, Grenz-/Fortlauf-/Fehlerprüfungen sowie den zentralen Quellcheck. Vorgaben und kontrollierte Callbacks werden im Prüfbericht benannt. Missions-, Android- und Hörabnahme werden bei einer späteren Integration separat nachgetragen; interne Ergebnisse werden nicht als bereits gehörtes oder vollständig gespieltes Ergebnis ausgewiesen.
