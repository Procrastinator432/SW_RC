# Texturen und Waffen-Audio: Korrektur 0.20.1

APK **republic-commando-0.20.1.apk** und **geo_01a-crash.rcplay** liegen unter `analysis/mission2-av`. Bitte beide ersetzen: die neue Szene enthält zusätzliche Materialbindungen und das originale DC-15s-Schuss-Sample; sie benötigt die neue App-Version (Code23). Bedienung und Missionsziele bleiben wie in [der Abschnittsanleitung](ORIGINAL_CRASH_MISSION.md).

## Befund und Korrektur

Die Abnahme von 0.20.0 bestätigte den Spielablauf, ließ aber bekannte Darstellungs- und Audio-Lücken als spätere Arbeit stehen. Die fehlenden Texturen waren keine fehlenden Dateien auf dem Gerät: Der alte Offline-Export setzte bei allen 557 BSP-Dreiecken die Materialbindung auf `None`. Drei StaticMesh-Materialien scheiterten außerdem an `TexCoordSource.SourceChannel=1`. Dadurch erschienen Boden und Fels-/Wandflächen in diagnostischen Ersatzfarben.

`rc-scene-repair` übernimmt die vorhandene Missionsdatei und bindet die ursprünglichen BSP-Materialien über Oberflächenbasis und U-/V-Vektoren; die Texelkoordinaten werden durch die ursprünglichen Texturabmessungen normiert. Für `BurnRoqBlend`, `WallTrimUV2` und `ArenaPlastrUV2` wird tatsächlich UV-Stream1 des Originalmeshes verwendet. Auch verschachtelte ausgewählte Combiner-/TexCoordSource-Eingaben werden berücksichtigt. 557 BSP- und 8.176 Mesh-Dreiecke sind korrigiert. Der Export verweigert eine Lieferung, wenn noch eine beibehaltene Oberflächentextur fehlt oder eine Materialauflösung fehlschlägt.

1.264 Dreiecke gehören zu `HsFalloff`-Lichteffekten. Ihr Shader besitzt keinen einfachen undurchsichtigen Oberflächen-Diffusepfad. Diese nicht unterstützten Volumen werden ausdrücklich ausgespart, statt als feste graue Flächen gezeichnet. Das ist noch keine Rekonstruktion der ursprünglichen volumetrischen Beleuchtung. Ausgewählte Combiner-Basistexturen, Beleuchtung, Blendmasken und weitere Effekte bleiben vereinfacht; null untexturierte Flächen bedeutet keine vollständige Materialparität. Fehlende dekorative Actors werden durch diese Reparatur ebenfalls nicht ergänzt.

Der ursprüngliche DC-15s-Default `properties.DC15sDefaults.FireSound` verweist auf `GEN_Sound.Weapons.wep_GEN_pistolFire_01`. Dieses PCM-Sample wird als fünfter Bankeintrag in die neue Szene übernommen. Jeder vom Spielkern akzeptierte Spielerschuss erzeugt genau ein konsumierbares Audioereignis, einschließlich Fehlschüssen. Gedrücktes Feuer während der Schusssperre oder beim Nachladen erzeugt keine zusätzlichen Geräusche. Advisor und Waffe teilen sich einen monotonen Ereigniszähler; Neustart und Pause bereinigen die Wiedergabe. Die noch vorläufigen DC-15s-Schadens-/Munitionsregeln werden dadurch nicht als originalgetreu ausgewiesen. Weitere Waffenhandhabungs-, Gegner- und Umgebungsgeräusche sind noch offen.

## Abnahme

- 990 Bibliothekstests einschließlich neuer Prüfungen zu Schusssperre, Nachladen, Pause, Neustart, begrenzter Ereigniswarteschlange und ungültiger Soundzuordnung.
- Physischer Weg durch beide Originalziele im gemeinsamen Rust-Kern und über die native ABI; fünf Abschüsse und exakter Neustart. Die ABI prüft die Anzahl der Waffenereignisse gegen tatsächlich akzeptierte Schüsse sowie einmaligen Verbrauch und kurze Puffer.
- Normale Android-Dateiauswahl und Touch-Bedienung mit GPU und CPU bis zum Missionsabschluss; pro Durchlauf 15 Schüsse und 15 erfolgreiche Waffen-Sample-Wiedergaben sowie vier Advisor-Zeilen. SoundPool-Laden/Abspielen wird auf Fehler geprüft. Das bestätigt den Android-Audiopfad; Lautsprecher und Lautstärke auf einem physischen Gerät bleiben zu prüfen.
- Visuelle Kontrolle an Bacta und Absturzstelle sowie CPU-/GPU-Bildvergleich, Grafik-Kontextneubau und wiederholtes Laden. Alte Bacta-Szene und 1002 gespeicherte Replay-Eingaben bleiben unverändert.

Prüfbelege, Quelldateien und gebundene Originalpakete stehen im `validation-manifest.json`; der vorherige Lieferstand bleibt unverändert archiviert.

## Priorität vor weiteren Missionsabschnitten

Sichtbare Materiallücken und fehlende grundlegende Spielergeräusche werden vor dem Granatenabschnitt korrigiert. Neue Lieferungen benötigen zusätzlich zur Spielbarkeitsprüfung eine Materialinventur ohne ungewollte Ersatzflächen, kontrollierte Ansichten entlang des Spielwegs und eine Prüfung der hörbaren Waffenereignisse. Die noch fehlenden Shader-/Lichteffekte und Geräuschgruppen bleiben getrennt und konkret erfasst; die Abnahme auf dem physischen Gerät folgt mit dieser Lieferung.
