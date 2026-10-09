# Dynamische Buffer vergroessern

Stand: 2026-10-09. Fuenf Aufgaben abgeschlossen in `crates/rc-package/src/d3d_resize.rs`; Diagnoseadapter `rc-d3d-resize-check`. Dieser Block erweitert den direkten Uploadpfad aus `D3D_DYNAMIC.md` um die zuvor externe Resizeplanung.

## 1. Streamhandles invalidieren

Originalfunktion D3DDrv 10029fe0: Die vorzeichenbehaftete Streamkapazitaet wird auf 0 bis 16 begrenzt. Applied- und Desired-Handles werden unabhaengig verglichen. Ein Applied-Treffer loescht Handle und Stride und plant sofort SetStreamSource (+14c) mit null; ein Desired-Treffer loescht nur dessen Paar. Dirtybits, Indizes, Shader und Transformationen bleiben erhalten. Auch Handle null ist ein gueltiger Helperparameter; die Resizeaufrufer invalidieren nur alte Nichtnullhandles.

## 2. Vertex-Doppelbuffer neu anlegen

Originalfunktion 1002a5f0: Beide alten Handles werden in Slotreihenfolge invalidiert und freigegeben, auch bei gleichen Handles zweimal. Neue Kapazitaet und Cursor null werden geplant; aktiver Slot und Quellzeiger bleiben erhalten. HardwareVertex waehlt Usage 208 oder 218. CreateVertexBuffer (+5c) bekommt Format null und die jeweiligen Outslots +30/+34.

Ein Fehler beim ersten Create beendet den Pfad. Nur ein Fehler beim zweiten Create fuehrt zur Wiederholung des Paars. Die Poolfolge ist 0, 0, 2; mit SkipEviction 0, 2. Ohne SkipEviction wird vor jeder Wiederholung EvictManagedResources (+14) aufgerufen. Der native Retry ueberschreibt den ersten neuen Handle ohne zusaetzlichen Release; diese Aufrufreihenfolge bleibt im Plan erhalten. Es wird kein echter GPU-Handle erzeugt oder freigegeben.

## 3. Indexbuffer neu anlegen

Originalfunktion 1002a880: Der alte Handle wird einmal freigegeben, ohne Streaminvalidierung und ohne Loeschen des Indexcaches. Kapazitaet und Cursor werden neu gesetzt; die gespeicherte Breite bleibt erhalten. Breite vier waehlt Format 66, alle anderen Werte 65. CreateIndexBuffer (+60) nutzt dieselben Usage- und Poolregeln, wiederholt aber jeden fehlgeschlagenen Create. Groesse null und rohe DWORD-Groessen werden nicht durch eine Mindestgroesse ersetzt.

## 4. Vertex-Resize mit Ringupload verbinden

Der integrierte Runtimeadapter entscheidet mit dem originalen vorzeichenbehafteten Wachstumsgate, fuehrt bei Wachstum die neue Resizeplanung aus und uebergibt deren Handles an den bestehenden direkten CPU-Upload. Damit entfallen von aussen vorgegebene Resizehandles. Die Cacheinvalidierung bleibt erhalten; Wachstum und anschliessender Ringreset zaehlen weiterhin separat. Wiederverwendung benoetigt keine Createantworten.

## 5. Index-Resize mit Ringupload verbinden

Indexwachstum verwendet ebenfalls automatisch den neuen Createplan. Der gespeicherte Erzeugungstyp bleibt vom Quell-Breitengetter fuer den Ringoffset getrennt. Der Diagnoseadapter verbindet beide Uploads mit der bestehenden Rendererbindung und prueft den vollstaendigen Zustand nach jedem Schritt. Sichere Fehler erhalten CPU-Puffer und Modellzustand; diese Transaktion beschreibt keine Rueckabwicklung bereits ausgefuehrter GPU-Aufrufe.

## Nachweis

`Generate-D3DResize.py` bindet seine synthetischen Eingaben per SHA256 an den vorherigen dynamischen Nachweis. `Record-D3DResize.py` interpretiert die exportierten Originalkoerper fuer Cacheinvalidierung und Resize, auch verschachtelt innerhalb der Ring- und Bindefunktionen. COM-, Getter-, Fuell- und Shaderantworten werden an expliziten ABI-Grenzen vorgegeben. SEH, Logging, Zeitmessung und Returns sind ausgeklammert. Die Instruktions-VM unterstuetzt jetzt auch SETZ.

Verglichen werden Aufrufreihenfolge und Argumente, beide Ringwrapper, Device-Zaehler, Rendererfelder, Deferred-Cache und vollstaendige CPU-Ziele. Ergebnis: **48 Sequenzen / 192 integrierte Schritte**, **96 separate Resizeablaeufe**, **24 Invalidierungsproben**, **12 sichere Fehlerfaelle**, **55627 Originalinstruktionen**. Ueber alle Proben: 462 Creates, 180 Releases, 118 Evictions und 60 unmittelbare Stream-Unbinds. Die integrierten Sequenzen enthalten 60 Resizeablaeufe.

**15 neue Tests, insgesamt 746 Workspace-Tests**. Debug- und Releaseberichte sind bytegleich; Clippy mit `-D warnings`, Format- und Diffpruefung bestanden.

```powershell
python scripts/Generate-D3DResize.py
cargo run -p rc-inspect --bin rc-d3d-resize-check -- analysis/reports/d3d-resize.input.json analysis/reports/d3d-resize.json
cargo run --release -p rc-inspect --bin rc-d3d-resize-check -- analysis/reports/d3d-resize.input.json analysis/reports/d3d-resize-release.json
cargo test --workspace *> analysis/reports/d3d-resize-tests.log
python scripts/Record-D3DResize.py
```

Artefakte liegen unter `analysis/reports/d3d-resize*`; Evidenzschluessel `d3d_resize_validation`. Neue Exporte: `d3d-resize-cache.c`, `d3d-resize-cache.asm`, `d3d-resize-index.asm`; der Vertexhelper verwendet den vorhandenen Export `d3d-dynamic-vertex.asm`.

Offen bleiben echte COM-/GPU-/Allocatorausfuehrung, Poolkonstruktoren, Scratchpfad, Verbindung dynamischer Rueckgabeoffsets mit Draws, Skinningcallback, Shadererzeugung und Live-Spiel. Die Fixtures sind keine neuen Live-Aufnahmen. Native Fehlerbehandlung wird nicht nachgestellt. Android wird wie vereinbart zum Schluss geprueft; das Spiel ist weiterhin nicht spielbar.
