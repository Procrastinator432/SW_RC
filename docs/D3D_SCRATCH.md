# Gemeinsamer Scratchpuffer fuer dynamische Uploads

Stand: 2026-10-09. Fuenf Aufgaben abgeschlossen in `crates/rc-package/src/d3d_scratch.rs`, Diagnoseadapter `rc-d3d-scratch-check`. Der neue Runtimeadapter erweitert `d3d_resize::Runtime`; die bisherigen direkten APIs behalten ihren abgegrenzten Vertrag. Dieser Nachweis erweitert `D3D_RESIZE.md`.

## 1. Gemeinsamen Scratchpuffer vergroessern

Originalkoerper: Vertex 1002d7fb..1002d8b1 und Index 1002da75..1002db1d. Beide verwenden denselben Devicepuffer: Zeiger +e7e8 und gepackter Arraycount +e7ec. `SHL 3 / SAR 3` extrahiert den vorzeichenbehafteten 29-Bit-Count; die oberen drei Flags bleiben erhalten.

Wenn Count vorzeichenbehaftet kleiner als Groesse+4 ist, ruft der Originalpfad den Arrayhelper 10001db0 erst mit (0,0), dann mit (2*max(Groesse,65535),0) auf. Der neue Bereich wird vollstaendig auf null gesetzt. Die API plant beide Aufrufe und erhaelt deren Rueckgabezeiger als ausdrueckliche Antworten. Nach Wachstum umfasst der CPU-Spiegel den neuen logischen Count; Wiederverwendung erhaelt vorhandene Bytes und gegebenenfalls erfassten physischen Zusatzplatz. Der Originalhelper bewahrt die oberen Countflags; seine Commitinstruktionen 10001ed6..10001ee8 werden separat mitinterpretiert. Seine Allocatorzweige bleiben eine externe Grenze.

## 2. Guard schreiben und vollstaendig kopieren

Das Original schreibt DWORD 03221977 an Scratch+Groesse+1. Das Byte direkt hinter dem Nutzbereich bleibt unberuehrt. Die Wachstumspruefung gegen Groesse+4 garantiert allein noch nicht den benoetigten Bereich bis Groesse+5. Der sichere Port akzeptiert einen erfassten physischen Zusatzplatz oder weist den Grenzfall vor Mutation ab. Er verschiebt weder den Guard noch die originale Wachstumsbedingung. Eine Guardpruefung nach dem Callback ist in den untersuchten Originalkoerpern nicht vorhanden und wird nicht hinzuerfunden.

Der explizite Fuellcallback ueberschreibt nur die mitgelieferten Praefixbytes. Danach kopiert der Scratchpfad die gesamte Lockgroesse mit DWORD- und Byte-Restkopie. Nicht neu beschriebene Scratchbytes gelangen ebenfalls ins Lockziel; nach Wachstum sind sie null, bei Wiederverwendung koennen sie von vorigen Uploads stammen. Zielbytes hinter der Lockgroesse bleiben erhalten. Das Feld `upload.copied` zaehlt weiterhin die expliziten Callbackbytes; die Nachweisstatistik `scratch_copy_bytes` zaehlt die vollen Scratchkopien.

## 3. Vertex-Scratchupload

Bei Device+4114 ungleich null verwendet der Vertexcallback +24 den Scratchzeiger statt des Lockzeigers. Lock (+2c), aktiver Doppelbufferslot, Ringoffset, DISCARD/NOOVERWRITE, Unlock (+30) und Zaehler folgen der bestehenden Originalplanung. Die anschliessende Kopie beginnt am bereits vom Lock gelieferten CPU-Zeiger; der GPU-Offset wird nicht erneut addiert. Bei ausgeschaltetem Scratchmodus wird der direkte Pfad verwendet und der Scratchzustand nicht benoetigt oder veraendert.

## 4. Index-Scratchupload

Der Indexcallback +14 verwendet denselben Scratchzustand und kann deshalb vom vorherigen Vertexupload erhaltene Restbytes sehen. Ringcursor und Rueckgabeoffset folgen der nativen Indexplanung. Die gespeicherte Erzeugungsbreite bleibt vom Quell-Breitengetter fuer die Offsetdivision getrennt; Scratchmodus und Kopie aendern diese Trennung nicht.

## 5. Resize, Scratch und Rendererbindung verbinden

Wachstum plant zuerst die bereits rekonstruierten Create-/Release-/Evictionpfade und gegebenenfalls die Streaminvalidierung, anschliessend Ringlock und Scratchtransfer. Der Diagnoseadapter bindet die neuen Handles an den Renderer. Er vergleicht nach jedem wechselnden Vertex-/Indexschritt den gesamten Runtime-, Renderer-, Deferred- und CPU-Zustand. Fehler beim Unlock, bei ungueltigen Zeigern, Guardgrenzen, Divisoren oder einer erst nach dem Upload ungueltigen Shaderantwort erhalten den vorherigen Modellzustand und das Ziel. Dies ist eine sichere Porttransaktion und keine Rueckabwicklung echter GPU-Aufrufe.

## Nachweis und Grenzen

`Generate-D3DScratch.py` bindet 48 synthetische Sequenzen per SHA256 an die vorherigen Resizeeingaben. Die Proben variieren aktiven Slot, Ringwachstum, Hardware-/Evictionmodus, obere Arrayflags, negative gepackte Counts, Guardgrenzen mit physischem Zusatzplatz, kurze Callbacks, erhaltene Restbytes, abgeschalteten Scratchmodus und beide Indexpools. Zwölf Fehlerfaelle pruefen den gemeinsamen Rollback. Die 18 neuen Rusttests pruefen zusaetzlich grosse Wachstumsanforderungen und die Guardgrenze ohne physischen Zusatzplatz.

`Record-D3DScratch.py` fuehrt die zuvor ausgeschlossenen Original-Scratchzweige innerhalb der bestehenden Ring-/Resize-/Bindekoerper aus. Die Byte-VM unterstuetzt jetzt auch STOSD.REP und STOSB.REP. Allocatorantworten, Quellengetter, Callbackbytes, Shader und COM-Ergebnisse bleiben explizite Antworten. SEH, Zeitmessung, Logging und Returns sind ausgeklammert; native Fehlerpfade werden nicht ausgefuehrt.

Ergebnis: **40847 interpretierte Originalinstruktionen**, **48 Sequenzen / 192 Schritte**, **168 Scratchvorbereitungen** (10 Wachstum, 158 Wiederverwendung), **20 Arrayhelper-Aufrufe**, **60 integrierte Resizes**, **198 COM-Creates**, **4504 Callbackbytes**, **4284 vollstaendig kopierte Scratchbytes**, **12 sichere Fehlerfaelle**. Alle modellierten Felder, geordneten Aufrufe und vollstaendigen CPU-Puffer stimmen nach jedem Schritt ueberein.

**18 neue Tests, insgesamt 764 Workspace-Tests**; Debug-/Releaseberichte bytegleich, Clippy mit `-D warnings`, Format- und Diffpruefung bestanden.

```powershell
python scripts/Generate-D3DScratch.py
cargo run -p rc-inspect --bin rc-d3d-scratch-check -- analysis/reports/d3d-scratch.input.json analysis/reports/d3d-scratch.json
cargo run --release -p rc-inspect --bin rc-d3d-scratch-check -- analysis/reports/d3d-scratch.input.json analysis/reports/d3d-scratch-release.json
cargo test --workspace *> analysis/reports/d3d-scratch-tests.log
python scripts/Record-D3DScratch.py
```

Berichte: `analysis/reports/d3d-scratch*`; neue Originalexporte: `analysis/decompiled/d3d-scratch-array.c/.asm`; Evidenzschluessel `d3d_scratch_validation`. Die Quellenhashes erfassen auch die verwendeten vorherigen Replayhelfer. Historische Nachweise behalten ihre damaligen Testzahlen und Hashes.

CPU-Groessen und erfasster Speicher sind auf 16 MiB begrenzt; ein erforderliches verdoppeltes Wachstum kann deshalb bereits bei kleinerer Nutzgroesse abgewiesen werden. Echte Allocator-/COM-/GPU-Ausfuehrung, Poolkonstruktoren, Verbindung dynamischer Rueckgabeoffsets mit Draws, Skinningcallback, Shadererzeugung und Live-Spiel bleiben offen. Die synthetischen Antworten sind keine Live-Aufnahmen. Android folgt wie vereinbart zum Schluss; das Spiel ist weiterhin nicht spielbar.
