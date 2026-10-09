# Originale Buffer-Quelldaten und CPU-Kopien

Stand: 2026-10-09. Fuenf Aufgaben: rohe 16-Bit-Indexdaten, rohe 32-Bit-Indexdaten, Skin-Stream-Metadaten, gespeicherte/delegierte Skin-Fuellpfade und Verbindung zu statischen Uploadplaenen ueber einen CPU-Zielpuffer. Implementierung: `crates/rc-package/src/d3d_source.rs`; Diagnose: `rc-d3d-source-check`.

Originalreferenzen aus Engine.dll: FRawIndexBuffer GetSize/GetContents/GetIndexSize bei 104c7530/104c7540/104c6d80; FRaw32BitIndexBuffer bei 104c7570/104c7580/104c6d90; FSkinVertexStream GetComponents/GetStride/GetSize/GetStreamData/GetRawStreamData bei 103b0800/104fde10/104ffce0/104ffbf0/104ffcb0. Die Exportnamen und Adressen wurden aus dem originalen PE gelesen. C-Befunde stehen in `analysis/decompiled/d3d-source-callbacks.c`, Instruktionen in den Dateien `d3d-index-source.asm`, `d3d-index-width.asm`, `d3d-skin-source.asm`, `d3d-skin-components.asm` und `d3d-skin-stride.asm`.

## 1. 16-Bit-Indexkopie

Das originale Array liegt mit Datenzeiger an Quelle+10 und gepacktem Zaehler an +14. Die Anweisungen SHL 3 / SAR 3 ignorieren die oberen drei Bits und erweitern das Vorzeichen des verbleibenden 29-Bit-Wertes. GetSize multipliziert diesen Wert umlaufend mit zwei. GetIndexSize liefert zwei.

GetContents kopiert genau diese Byteanzahl: zunaechst volle DWORDs, dann die restlichen Bytes. Bei ungerader Indexanzahl verbleiben zwei Restbytes. Es findet keine Umordnung der Little-Endian-Daten statt. Ein Nullbytebereich benoetigt keinen dereferenzierbaren Datenzeiger.

## 2. 32-Bit-Indexkopie

FRaw32BitIndexBuffer verwendet dieselben Arrayfelder und dieselbe Zaehlerarithmetik, multipliziert aber mit vier; GetIndexSize liefert vier. GetContents uebernimmt die rohen DWORDs. Die API unterscheidet beide Klassen durch `IndexWidth`, nicht anhand zufaelliger Nutzdaten.

Die rohe Groessenrechnung bleibt von der sicheren Erfassung getrennt: negative oder grosse Counts koennen einen sehr grossen DWORD-Bytewert ergeben. Das wird als roher GetSize-Wert erhalten, aber nicht als unkontrollierte Hostallokation ausgefuehrt. `payload` begrenzt CPU-Kopien auf 16 MiB und verlangt vollstaendig erfasste Quelldaten.

## 3. Skin-Stream-Metadaten

Der gespeicherte Skin-Stream hat Besitzerzeiger +10, Moduswort +18, Datenzeiger +1c und gepackten Zaehler +20. Im gespeicherten Modus ist GetSize die umlaufende 32-fache Zaehlergroesse; die Stride ist immer 32. Bei Nichtnullmodus liest GetSize die Vertexanzahl aus Besitzer-VTable +10c und multipliziert sie mit 32. Ein fehlender Besitzer wird fuer diese Groessenabfrage sicher abgewiesen; das Original enthaelt hier keinen Nullschutz.

GetComponents schreibt nur sechs Bytes: `01 00 01 01 02 04`, und liefert drei zurueck. Der aufrufende Streamcode kopiert danach vier DWORDs plus das Rueckgabewort. `components` uebernimmt deshalb einen ausdruecklichen 16-Byte-Eingangspuffer und erhaelt dessen letzten zehn Bytes. Es wird weder eine vollstaendige Nullinitialisierung noch eine neue Interpretation der gepackten Komponententypen behauptet.

GetRawStreamData liefert bei Nichtnullmodus null. Sonst ergibt sich Datenzeiger plus umlaufender Vertexoffset mal 32; negative Offsets bleiben rohe Pointerarithmetik, keine sichere Arrayindexierung oder Speicherfreigabe.

## 4. Gespeicherter und delegierter Skin-Fuellpfad

GetStreamData delegiert nur, wenn Modus und Besitzer beide ungleich null sind: Besitzer-VTable +110 erhaelt den Zielzeiger. Der Datenfuellpfad besitzt damit einen zusaetzlichen Besitzer-Nullschutz, den GetSize nicht hat. Bei Nichtnullmodus mit Nullbesitzer verwendet die Fuellfunktion weiterhin die gespeicherten Arraybytes, obwohl die Groessenfunktion keinen gueltigen Besitzer lesen kann. Beide Regeln werden getrennt modelliert.

Gespeicherte Daten werden als rohe 32-Byte-Vertices kopiert; DWORD-/Restbytekopie entsprechen den Originalinstruktionen. Auch ein durch DWORD-Ueberlauf auf null gewickelter Bytewert wird erhalten. Besitzer-Vertexanzahl und Besitzer-Fuellbytes bleiben explizite externe Antworten: Der eigentliche Besitzer-Skinningcallback wird hier nicht ausgefuehrt oder als rekonstruiert bezeichnet.

`RawIndex::capture` und `Skin::capture` verwenden den vorhandenen sicheren Snapshot-Leser. Felder duerfen ueber direkt benachbarte erfasste Regionen reichen, aber nicht ueber Luecken. Nullobjekte, ueberlappende Regionen und Adressueberlaeufe werden abgewiesen. Das liest gelieferte Speicherabbilder, keinen laufenden Windows- oder Androidprozess.

## 5. CPU-Zielpuffer und Uploadplan

`vertex` und `index` verbinden die neuen Groessen-/Breiten-/Quelldaten mit den zuvor geprueften statischen Upload-Lebenszyklen. Die Ergebnisse enthalten den Lebenszyklusplan und die tatsaechlich in einen bereitgestellten CPU-Zielpuffer kopierte Byteanzahl. Eine neue Ressourcenstruktur wird erst nach erfolgreicher Planung und Kopie uebernommen.

`fill_upload` prueft vor Writes die Lock/Fuellcallback/Unlock-Folge, Quellobjekt, Callbackoffset, Zielzeiger, Lockoffset, Groesse, Flags und Zielkapazitaet. Nur der Nutzdatenpraefix wird kopiert; uebrige Bytes bleiben erhalten. Bei einer Index-Revisionsuebereinstimmung entfaellt die Fuellung vollstaendig, einschliesslich Zugriff auf eventuell nicht erfasste Quelldaten. Das gilt wie im Original auch nach einer Neuallokation. Eine leere Indexquelle kann einen zwei Byte grossen Lockplan haben, ohne diese zwei Bytes zu ueberschreiben.

Diese Kopie ist ein CPU-Mirror des Lockziels. Create/Release/Lock/Unlock bleiben geplante Befehle mit vorgegebenen Antworten. Es wird kein realer Direct3D-, Vulkan- oder Androidbuffer gefuellt. Fehler erhalten Ressourcenstruktur und Zielpuffer; diese sichere Transaktion wird nicht als native Fehlerbehandlung behauptet.

## Pruefung und Grenzen

`Generate-D3DSource.py` prueft die SHA256 der vorherigen Upload-Eingaben und erzeugt sparse synthetische Originalspeicherlayouts. Alle acht oberen Countflagmuster, beide Indexbreiten, Null-/ungerade-/negative-/Ueberlaufzahlen, beide Moduszustaende, Null-/Nichtnullbesitzer, signierte Pointeroffsets und vorbelegte Komponentenpuffer sind enthalten. Sechs malformed Snapshotfaelle pruefen sichere Erfassungsfehler. Die 64 kombinierten Proben variieren Quellbytes, Revisionen und gespeicherte/delegierte Fuellung.

`Record-D3DSource.py` interpretiert die originalen Getter, Pointer-/Countarithmetik, partiellen Komponentenwrites und REP-Kopien sowie die bereits geprueften Upload-Lebenszyklen. Die Byte-VM wurde fuer SAR und MOVSB.REP erweitert. ABI-Voraussetzung bleibt geloeschtes Directionflag. Besitzerergebnisse werden an den geprueften +10c/+110-Aufrufen eingesetzt; deren eigene Berechnung wird nicht als Originalnachweis gezaehlt. Zu grosse native Kopierbereiche werden nur bis zur Groessenberechnung geprueft und dann sicher abgewiesen, nicht ausgefuehrt.

```powershell
python scripts/Generate-D3DSource.py
cargo run -p rc-inspect --bin rc-d3d-source-check -- analysis/reports/d3d-source.input.json analysis/reports/d3d-source.json
cargo run --release -p rc-inspect --bin rc-d3d-source-check -- analysis/reports/d3d-source.input.json analysis/reports/d3d-source-release.json
cargo test --workspace *> analysis/reports/d3d-source-tests.log
python scripts/Record-D3DSource.py
```

Ergebnis: **144 Indexfaelle**, **256 Skinfaelle**, **64 kombinierte Transfers**, **sechs sichere Capturefehler**. **17745 Originalinstruktionen** insgesamt; **2112 Index-Nutzdatenbytes**, **20224 Skin-Nutzdatenbytes** und **3694 Transferbytes** verglichen, einschliesslich ausdruecklicher Besitzer-Fuellfixtures. 48 Index- und 24 Skin-Kopierbereiche wegen Groesse begrenzt; 64 Skin-Groessenabfragen ohne Besitzer sicher abgewiesen. Die gesamten CPU-Zielpuffer einschliesslich ungeschriebener Restbereiche stimmen ueberein; 21 Index-Transferfaelle ueberspringen die Datenfuellung. Debug/Release bytegleich. Zwoelf neue Tests ergeben **689 Workspace-Tests**; Clippy `-D warnings`, Format- und Diffpruefung bestanden.

Artefakte: `analysis/reports/d3d-source.input.json`, `d3d-source.json`, `d3d-source-release.json`, `d3d-source-validation.json`, `d3d-source-tests.log`; Evidenzschluessel `d3d_source_validation`. Fruehere Nachweise behalten ihre historischen Quellhashes und Testzahlen.

Weiter offen: Ausfuehrung des Besitzer-Skinningcallbacks, echte GPU-Buffer und Backend-Aufrufe, Ressourcenlookup/Lebensdauer, Verbindung der Nutzdaten-Lebenszyklen mit der ganzen Material-/Draw-Szene, dynamische Ringbuffer, Shadererzeugung und Live-Spiel. Die Fixtures dieses Blocks sind synthetische Speicherabbilder, keine neuen Live-Meshdumps. Diffuse-Abdeckung bleibt 271/289; Android-Pruefung zum Schluss.
