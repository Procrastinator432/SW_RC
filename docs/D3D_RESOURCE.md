# Ressourcen-Cache und Wrapper-Lebensdauer

Stand: 2026-10-09. Implementierung: `crates/rc-package/src/d3d_resource.rs`; Diagnose: `rc-d3d-resource-check`. Dieser Block schliesst fuenf Aufgaben der statischen Ressourcenverwaltung ab.

## 1. Hashberechnung

Die Originalfunktion bei D3DDrv 1002a110 bildet 4096 Buckets: `(((low >> 4) & 0xff0) + ((low >> 16) & 0xf)) ^ (low & 0xfff)`. Nur das untere DWORD des Ressourcenschluessels geht in den Hash ein. Das obere DWORD darf daher nicht als zusaetzliche Bucketmischung verwendet werden.

## 2. Suche nach 64-Bit-Schluesseln

GetCachedResource bei 10015040 startet bei Device+b4+Bucket*4, vergleicht beide DWORDs an Wrapper+8/+c und folgt +2c. Der erste vollstaendige Treffer gewinnt; gleiche Hashwerte und gleiche untere DWORDs reichen nicht. Die Suche veraendert keine Wrapperbytes. Ein Treffer wird vor weiterer Kettenverfolgung zurueckgegeben.

## 3. Initialisierung und Einfuegen

Der Basiskonstruktor bei 1002a140 setzt Besitzer, Schluessel, Revision, Frame, gespeicherten Hash und bekannte Statusfelder. Er haengt den Wrapper vorne in die globale Liste an Device+b0 ueber +28 sowie in die Hashliste ueber +2c. Die Bytes +1e/+1f bleiben erhalten; ein Basiswrapper behaelt auch +30..+3f.

Der Vertexkonstruktor bei 1002a320 setzt VTable 10072c10 und loescht +30/+34/+38/+3c. Der Indexkonstruktor bei 1002a7d0 setzt VTable 10072d28 und loescht nur +30/+34. Die uebrigen Bytes des bereitgestellten 64-Byte-Allokationsabbilds bleiben unveraendert. `insert` verwendet bereits bereitgestellte Abbilder und fuehrt keinen Host- oder Spielallocator aus.

## 4. Entfernen aus beiden Listen

Der Basisdestruktor bei 1002a1d0 repariert Kopf oder Vorgaenger beider Listen, setzt die Basis-VTable 10072bfc und loescht die eigenen +28/+2c. Fuer die Hashliste wird das gespeicherte +18 verwendet. Die Allokation bleibt im erfassten Cache vorhanden; Nutzdatenfelder und Bufferhandles werden durch diesen Basisdestruktor nicht geloescht. Wiederholtes Entfernen ist fuer einen bereits entfernten gueltigen Wrapper moeglich.

## 5. Buffer-Freigaben

Vertex-Reset bei 1002a350 plant Release an VTable+8 fuer den Nichtnullhandle +30, dann fuer +34, und loescht jeweils den Slot. Zwei gleiche Handles ergeben zwei Aufrufe. Listeneintraege, Kapazitaet und Revision bleiben erhalten.

Der Indexdestruktor bei 1002a800 plant Release fuer Nichtnullhandle +30 und fuehrt danach den Basisdestruktor aus. Anders als Vertex-Reset loescht er weder den Handle noch die Kapazitaet. Er ist kein wiederholt aufzurufender Reset: Ein weiterer Destruktoraufruf koennte denselben Handle erneut freigeben. Die Befehle werden hier nur aufgezeichnet.

## Nachweis und Grenzen

`Generate-D3DResource.py` bindet die Eingaben per SHA256 an den vorherigen Quelldatennachweis. 8196 Hashproben decken alle 4096 Buckets ab. 64 synthetische Cache-Sequenzen mit 1408 Schritten pruefen Kollisionen, doppelte Schluessel, unterschiedliche obere DWORDs, Einfuegen verschiedener Wrapperklassen, Kopf-/Mittel-/Endentfernung und Freigabereihenfolgen. Nach jedem Schritt werden alle Bucketkoepfe und alle 64 Bytes jedes Wrapperabbilds verglichen.

`Record-D3DResource.py` interpretiert die exportierten Originalinstruktionen. COM Release wird an seiner ABI-Grenze als geordneter Aufruf erfasst. Funktionsreturns und SEH-Rahmen des Indexdestruktors werden ausgeklammert; die drei Returnzweige des Basisdestruktors erhalten einen gemeinsamen Stopmarker. Getter-/Konstruktor-/Listenlogik wird aus den Originalinstruktionen ausgefuehrt, nicht aus Rust abgeleitet.

Ergebnis: 122356 interpretierte Instruktionen einschliesslich der markierten Release-/Returngrenzen, 640 Lookups (448 Treffer, 192 Fehlschlaege), 256 Inserts, 448 Basis-Unlinks und 144 Release-Aufrufe. Zwoelf ungueltige Cachefaelle werden ohne Mutation sicher abgewiesen; sie werden nicht als native Fehlerpfade ausgefuehrt. Zwoelf neue Unit-Tests ergeben 701 Workspace-Tests. Debug und Release sind bytegleich; Clippy mit `-D warnings`, Format- und Diffpruefung bestanden.

Die sichere API begrenzt die Abbilder, prueft Besitzer/Bucket, eindeutige Wrapperadressen und verfolgte Ketten auf fehlende Knoten oder Zyklen. Mutierende Listenoperationen pruefen vor dem Schreiben die vollstaendigen relevanten Ketten. Das ist eine bewusst strengere Port-Grenze als die nativen Pointerzugriffe, keine Behauptung gleicher Fehlerbehandlung.

```powershell
python scripts/Generate-D3DResource.py
cargo run -p rc-inspect --bin rc-d3d-resource-check -- analysis/reports/d3d-resource.input.json analysis/reports/d3d-resource.json
cargo run --release -p rc-inspect --bin rc-d3d-resource-check -- analysis/reports/d3d-resource.input.json analysis/reports/d3d-resource-release.json
cargo test --workspace *> analysis/reports/d3d-resource-tests.log
python scripts/Record-D3DResource.py
```

Artefakte: `analysis/reports/d3d-resource.input.json`, `d3d-resource.json`, `d3d-resource-release.json`, `d3d-resource-validation.json`, `d3d-resource-tests.log`; Evidenzschluessel `d3d_resource_validation`. Native Exporte: `d3d-resource-cache.c/.asm`, `d3d-resource-lifecycle.c/.asm`, `d3d-resource-unlink.c`, `d3d-index-resource.asm`.

Offen bleiben Verbindung von Cache, Quelldaten, Upload und Draw in einer durchgehenden Szene, echte GPU- und Allocatorausfuehrung, dynamische Buffer, Skinningcallback, Shadererzeugung und Live-Spiel. Diffuse-Abdeckung weiterhin 271/289. Android-Pruefung auf Benutzerwunsch zum Schluss.
