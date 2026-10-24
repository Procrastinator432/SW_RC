# Originalfunktionen und Umsetzungsstand

**Aktuellste Quellfortsetzung 24.10.2026:** [FALSource-Steuerung und Abschlussregeln](../AUDIO_DRIVER.md) mit 2.720 ganzen Originalmethodenaufrufen intern geprüft. Echte FALSource-Methodenkörper bei kontrolliertem OpenAL, keine neue Audioausgabe oder pauschale Hochstufung des Gesamtkatalogs.

**Neueste Quellfortsetzung 24.10.2026:** [AL-Upload und PlaySource-Befehlsfolge](../AUDIO_UPLOAD.md) intern geprüft: 512 ganze Originalaufrufe und 2.048 EAX-Grenzproben. Reiner Befehls-/Zustandskern mit kontrolliertem AL und Basis-Play/Pause; kein vollständiger Audiotreiber und keine pauschale Hochstufung des Gesamtkatalogs.

**Quellfortsetzung 24.10.2026:** [Audioaktivierung und Stimmenzählung](../AUDIO_ACTIVATION.md) mit klar begrenzter AL-Vorbereitung. Kein vollständiger AL-Treiber und keine pauschale Hochstufung des älteren Gesamtkatalogs.

**Quellfortsetzung 24.10.2026:** [Pause-/Stopbefehle und Warteschlange](../AUDIO_COMMANDS.md) intern geprüft. Der umfassende Katalog bleibt seine datierte Bestandsaufnahme; keine pauschale Hochstufung von App-/Treiberfunktionen.

**Weitere Quellfortsetzung 24.10.2026:** [Audioquellenverwaltung](../AUDIO_SOURCE_RUNTIME.md) – Initialisierung, Actoranbindung, Lebensdauer, Suche und persistente Controllerkomposition intern geprüft. Keine pauschale Hochstufung des auf 0.53.0 datierten Gesamtkatalogs.

**Weitere Quellfortsetzung 24.10.2026:** [Allgemeine Audio-Controller](../AUDIO_CONTROLLERS.md) – fünf intern geprüfte Kerne, sieben Testgruppen und 7.048 Originalvergleiche. Allgemeine App-/Treiberanbindung bleibt offen; der umfassende Katalog wird dadurch nicht pauschal hochgestuft.

**Quellfortsetzung 24.10.2026:** [Audio-Geometrie-Laufzeitanbindung](../AUDIO_GEOMETRY_RUNTIME.md) im ausgewählten Host; vollständige Missionsregression und neue Androidabnahme offen. Keine pauschale Änderung des umfassenden Katalogs.

**Neuer Quellbaustein nach 0.60.0:** [Audio-Geometrie](../AUDIO_GEOMETRY.md) mit originalen BSP-/Regionsabfragen, Zonenradiusfaktoren und gespeichertem Radiusfortlauf. Noch keine Szenen-/Android-Anbindung; keine pauschale Änderung des umfassenden Katalogs.

**Neuere Laufzeitumsetzung:** [Ambient-Lebenszyklus / Prüflieferung 0.60.0](../AMBIENT_LIFECYCLE.md) – Aufnahme, Freigabe, lineare Ausblendung und Priorität für sechs ausgewählte Dauerquellen, mit persistenter Aktivierung und Android-Nachweis. Der umfassende Katalog bleibt auf 0.53.0 datiert.

Dies ist der zentrale Einstieg zum Funktionsbericht des Androidports. Bewerteter Stand: **Release 0.53.0 vom 21.10.2026**. Bericht und Detailtabellen liegen gemeinsam im Dokumentationsbereich des Repositorys.

**Neuere Laufzeitumsetzung:** [Räumliche Dauerquellen / Prüflieferung 0.59.0](../SPATIAL_AMBIENT.md) – sechs ausgewählte Quellen mit Originalparametern und Android-Nachweis. Der umfassende Katalog bleibt auf 0.53.0 datiert.

**Quellbaustein nach 0.58.0:** [Welt-Audio-Rechenkern](../WORLD_AUDIO_CORE.md) mit Originalradius-, Lautstärke- und Prioritätsrechnung. Noch keine Welt-/Android-Anbindung und keine pauschale Hochstufung des Katalogs.

**Neuere Umsetzung:** [Audioparameter / Prüflieferung 0.58.0](../AUDIO_PARAMETERS.md), zuvor [SoundMultiple / Prüflieferung 0.57.0](../SOUND_SELECTION.md), zuvor [Waffen-Foley / Prüflieferung 0.56.0](../WEAPON_FOLEY.md), zuvor [DC-15s-Waffenanimation / Prüflieferung 0.55.0](../DC15_ANIMATION.md), zuvor [Halbautomatik / 0.54.0](../DC15_TRIGGER.md). Der folgende umfassende Katalog bleibt ausdrücklich die datierte Bestandsaufnahme von 0.53.0; seine Einzelbewertungen wurden dadurch nicht pauschal hochgestuft.

## Bericht lesen

**[Vollständigen Funktionsbericht öffnen](../ORIGINAL_FUNCTION_STATUS.md)**

Der Bericht enthält 240 Funktions- und Systembereiche mit Umsetzung, Grenzen, Codeverweisen und Prüfbelegen sowie sämtliche 79 installierten Karten. Die Status unterscheiden integrierte Teilumfänge, Teilumsetzungen, Analysebausteine, offene Bereiche, ungeprüfte Funktionen und Plattformersatz.

**[Bericht und sämtliche Tabellen als ZIP herunterladen](Originalfunktionen_Umsetzungsstand_2026-10-21.zip)**

## Detailtabellen

| Inhalt | Datei |
|---|---|
| Funktionsmatrix mit 240 Bereichen | [CSV](data/2026-10-21/features.csv) · [JSON](data/2026-10-21/features.json) |
| Alle 79 Originalkarten | [maps.csv](data/2026-10-21/maps.csv) |
| Alle 6.810 exportierten UFunction-Definitionen | [original-functions.csv](data/2026-10-21/original-functions.csv) |
| Alle 3.170 Klassen und 260 States | [Klassen](data/2026-10-21/original-classes.csv) · [States](data/2026-10-21/original-states.csv) |
| Originalpakete und SHA-256 | [original-packages.csv](data/2026-10-21/original-packages.csv) |
| Native Exportsymbole, getrennt nach Code und Daten | [native-exports.csv](data/2026-10-21/native-exports.csv) |
| Binärdateien und SHA-256 | [native-binaries.csv](data/2026-10-21/native-binaries.csv) |
| Dokumentierte interne Adresse ohne PE-Exporttreffer | [known-internal-addresses.csv](data/2026-10-21/known-internal-addresses.csv) |
| Portmodule und ihre deklarierten Grenzen | [port-modules.csv](data/2026-10-21/port-modules.csv) |
| Zählungen, Releasezuordnung und Hashprüfung | [audit.json](data/2026-10-21/audit.json) |
| Prüfung der Übersicht und Dateiverweise | [overview-validation.json](data/2026-10-21/overview-validation.json) |

Die CSV-Dateien verwenden UTF-8 mit BOM und Semikolon als Trennzeichen. Nicht einzeln bewertete Routinen bleiben ausdrücklich ungeprüft. Der technische Index erfasst exportierte Definitionen; nicht exportierte interne Enginefunktionen sind noch nicht vollständig inventarisiert.

## Pflege und Reproduzierbarkeit

Die redaktionelle Bewertung steht in [features.psv](features.psv); [Generate-OriginalFunctionStatus.py](../../scripts/Generate-OriginalFunctionStatus.py) erzeugt den Bericht und die datierten Tabellen. Die Standardausgabe liegt hier unter `data/2026-10-21`.

```powershell
.\target\release\rc-inspect.exe 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/function-status-20261021/original-scan
python scripts/Generate-OriginalFunctionStatus.py --scan analysis/function-status-20261021/original-scan
```

Vor einer fachlichen Aktualisierung Katalog und Einzelbelege gegen den neuen Quell-/Releasestand prüfen. Eine höhere Versions- oder Testzahl aktualisiert die Bewertungen nicht automatisch.

Rohscan, extrahierte Originalquellen und historische Buildprotokolle bleiben im lokalen `analysis/`- beziehungsweise `build/`-Verzeichnis. Zum Lesen des Berichts und der zentralen Statusdateien werden sie nicht benötigt; einzelne vertiefende Rohbelege setzen diese lokale Arbeitsumgebung voraus.
