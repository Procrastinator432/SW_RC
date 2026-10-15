# GEO_01C als dritte Karte / 0.36.0

GEO_01C wird jetzt nach dem tatsächlichen Abschluss von GEO_01B geladen. Gesundheit, Magazin, Reservemunition, Granaten und der Zustand von Scorch und Fixer bleiben erhalten. Diese Lieferung erreicht **Kartenimport und B→C-Übergabe**. Die ursprünglichen Sun-Fac-Aufgaben und Begegnungen sind noch nicht als spielbarer Ablauf integriert; die gesamte erste Mission ist weiterhin unvollständig.

## Ausprobieren

- Neue APK: `analysis/mission19-map3/delivery/republic-commando-0.36.0.apk` (ARM64 und x86_64, VersionCode40).
- Passende dritte Szene: `analysis/mission19-map3/delivery/geo_01c.rcplay`.
- GEO_01A weiter mit `analysis/mission15-complete/delivery/geo_01a-first-mission.rcplay` spielen; GEO_01B mit `analysis/mission18-map2-route/delivery/geo_01b.rcplay`.
- Nach Abschluss von GEO_01B **Weiter: GEO_01C** wählen und die dritte Szenendatei öffnen. Erst eine vollständig geladene, passende Karte ersetzt die bisherige Sitzung.
- Die App speichert den abgeschlossenen B→C-Übergang getrennt unter `files/geo_01b-handoff.json`. Beim direkten Öffnen von GEO_01C wird ein vorhandener gültiger Datensatz wiederhergestellt. Die bestehende A→B-Datei bleibt separat. Ohne passenden gespeicherten Übergang beginnt der eigenständige Erkundungsstart mit Standardwerten und ohne beigetretene Begleiter.
- Bewegen, Springen, Ducken, Schießen, Nachladen und Pause funktionieren wie bisher. Es gibt in GEO_01C noch keinen Sun-Fac-Kampf, keine Sev-Rekrutierung und keinen Missionsabschluss. Der Granatenbestand wird übernommen; der Live-Wurf dieser Karte ist noch nicht angebunden.

## Originalimport und Darstellung

Original-PlayerStart0 ist Actor92 bei `[-20039.7890625, 12575.505859375, 1144]`, Blickrichtung aus dessen Rotator `[-7,-7239,-7]`. Der Controller landet nach wenigen Ticks regulär auf dem Boden. 81.748 sichtbare Dreiecke, 74 Kollisionsquellen und 134 statisch gezeichnete Mesh-Instanzen bilden die importierte Karte. Alle 195 BSP- und 81.553 verbleibenden Mesh-Dreiecke besitzen erfolgreiche Oberflächenbindungen; der Oberflächenbeleg meldet keine Fehler. Die Szene umfasst 19.779.504 Bytes.

Der statische Export berücksichtigt den originalen DrawType. Die beiden Seil-Akteure Rope5/Rope6 und EndLevelIconProp2 verwenden DrawType2, nicht DT_StaticMesh8: Ihre DefaultProp-Meshes sind keine geeignete Laufzeitdarstellung und werden nicht als graue Hilfswürfel exportiert. Zwei ursprünglich versteckte Marker bleiben ausgeblendet. Die eigentliche Seil-/Intro- und Endsymbol-Darstellung folgt mit der Inszenierung. Weitere 1.108 nicht unterstützte Effekt-Dreiecke werden separat als ausgelassen erfasst; statische Materialbindungen sind keine vollständige Shader-/Beleuchtungsparität.

Das originale Kollisionsmodell `Arena_HR.ArenaWalls.SunFacAnteRoom` enthält 141 gespeicherte Hüllen und eine solide BSP-Zelle ohne Hull-Datensatz. Der neue explizite Importpfad ergänzt diese kleine, räumlich begrenzte Zelle aus den 13 ursprünglichen Trennebenen. Ihre vier Eckpunkte liegen innerhalb der Originalgrenzen. Es wird keine Kollision ausgeschaltet und kein Ersatzquader eingesetzt. Unbeschränkte, widersprüchliche oder planare Zellen werden abgewiesen. Die bisherigen strikten Hull-Leser und Importe von GEO_01A/B bleiben unverändert; dies ist ein mathematischer Adapter, keine behauptete native LineCheck-Parität. `collision-completion-receipt.json` dokumentiert die Ergänzung.

## Übergabe und Squad

Schema1 bleibt für A→B kompatibel. Schema2 bindet B→C an Quelle, Ziel, Originalteleporter567 und alle acht abgeschlossenen GEO_01B-Ziele. Die Abfrage ist wiederholbar und ändert den Spielzustand nicht. C erhält Scorch und Fixer mit Mitgliedschaft, Gesundheit, Schilden und Kampfunfähigkeit. Alte Positionen, Wege, Gegner und Ereignisse werden verworfen. Beide Begleiter werden getrennt in freien, bodengeprüften Bereichen um den neuen Start platziert und mit Originalskins, Clips und Waffenbefestigungen dargestellt.

Der lokale C-Folgehost bewegt die Begleiter mit begrenzten physischen Körperschritten. Allgemeine Navigation durch die ganze dritte Karte, Kampf-/Befehls-/Wiederbelebungsregeln und Sev sind noch nicht integriert. Der Neustart stellt den übernommenen Eintrittszustand wieder her. Früher Wechsel, falsche Szene, beschädigte Daten, ungültige Bestände oder Fixer-Daten sowie eine wiederholte gespeicherte Wiederherstellung werden ohne Zustandsänderung abgewiesen. Native Grafik-Caches werden nach erfolgreichem Wechsel erneuert.

## Nachweise

`playthrough/validation.json` spielt die tatsächlichen 18.730 A- und 16.414 B-Eingaben ab und reist dann nach C. Der Eintritt hat 10 HP, 20/141 Munition, vier Granaten und beide Begleiter mit 100 HP/100 Schilden. 360 neue C-Eingaben prüfen Bewegung um etwa 122,53 Einheiten, einen Sprung, zwei Schüsse, Nachladen und Ducken. Nach dem Nachladen: 10 HP, 20/139 Munition. Neun ungültige gespeicherte Übergaben, separate sichere Platzierungen, wiederholbare Receipts, gespeicherte Wiederaufnahme sowie identischer Neustartzustand/Grafikframe sind geprüft.

`native/validation.json` und `android/native-validation.json` prüfen den tatsächlichen vollständigen A/B-Eingabestrom und C-Wechsel über die jeweiligen C-Schnittstellen. Der B-Abschluss verwendet die bereits dokumentierte halbe Spieleinheit Toleranz beim Plattformvergleich; Statuswerte werden exakt geprüft. `android/validation.json` prüft zusätzlich die APK mit tatsächlich erzeugtem gespeichertem B→C-Übergang, Dateiöffnung, GPU, Touch, Waffenaudio und Pause. Diese APK-Prüfung verwendet einen gespeicherten Übergang und ist kein vollständiger manueller Touch-Lauf durch A/B/C. Physische ARM64-Abnahme bleibt offen.

1.114 Rust-Bibliothekstests, Clippy/Format und der echte Java-Dateileser mit bekanntem/unbekanntem Umfang unter 96 MiB Heap sichern die Lieferung. `validation-manifest.json` enthält SHA-256, Quellenarchiv und die unveränderten vorherigen Lieferungen.

## Nächster Meilenstein: Sun-Fac-Missionsablauf

Die Inventur enthält 997 originale Actors/Aktionen, 47 Aktionsketten, 22 PawnFactory-Deklarationen und sieben originale Ziele: Sun Fac eliminieren, Zero-Seven treffen, Glas im Strategieraum zerschießen, Feuer auf den Superkampfdroiden konzentrieren, Türbrechmanöver, Scharfschützenmanöver und Fabrikaufzug betreten. Die Originalausfahrt39 führt nach `GEO_03A.ctm#playerstart`. Diese Daten sind in `original-bindings.zip` vorbereitet, werden in dieser Lieferung aber nicht ausgeführt. Als Nächstes werden Einstieg/Abseilen, Sun-Fac-Inszenierung und anschließende Ziele samt Sev, Glas-/Türzuständen, Gegnern und Aufzug eingebunden; danach folgt der durchgehende Abschluss der ersten Mission.
