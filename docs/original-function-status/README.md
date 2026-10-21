# Originalfunktionen und Umsetzungsstand

Dies ist der zentrale Einstieg zum Funktionsbericht des Androidports. Bewerteter Stand: **Release 0.53.0 vom 21.10.2026**. Bericht und Detailtabellen liegen gemeinsam im Dokumentationsbereich des Repositorys.

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
