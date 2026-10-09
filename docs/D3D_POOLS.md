# Dynamische Bufferpools anlegen und verwenden

Stand: 2026-10-09. Fuenf Aufgaben abgeschlossen in `crates/rc-package/src/d3d_pools.rs`; Diagnoseadapter `rc-d3d-pools-check`. Dieser Block verbindet die bisher vorausgesetzten Poolwrapper mit dem Ressourcen-Cache und dem Scratch-/Resize-/Ringpfad aus `D3D_SCRATCH.md`.

## 1. Dynamischen Vertexwrapper konstruieren

Originalfunktion 1002c120: Eine explizit bereitgestellte 72-Byte-Allokation erhaelt ueber den Basiskonstruktor 1002a140 ihren Deviceowner, den Quellschluessel an Quelle+4/+8, globale und Hashlistenlinks sowie die initialisierten Basisfelder. Danach folgen dynamische Vtable 10072f68, Handles null, Kapazitaet null, Quellzeiger null und aktiver Slot null. Der Konstruktor ruft den bestehenden Vertex-Resizehelper mit 20000 auf, erzeugt also initial ein Paar mit 128 KiB je Buffer. Dessen Cursorreset initialisiert auch Wrapper+40.

Der Cache akzeptiert nun neben 64-Byte-Bildern auch die vollstaendigen 72-Byte-Vertexbilder und prueft deren gesamte Adressausdehnung. Statische typisierte Zugriffe behalten ihre Vtablepruefung. Unbeschriebene Basisbytes bleiben erhalten; alte zufaellige Handlebytes der bereitgestellten Allokation werden vor dem Resize geloescht und nicht freigegeben.

## 2. Dynamischen Indexwrapper konstruieren

Originalfunktion 1002c460: Die 64-Byte-Allokation wird ebenfalls in beide Ressourcenlisten eingefuegt. Der Konstruktor setzt Vtable 10073018, Handle und Kapazitaet null sowie die uebergebene Erzeugungsbreite an +3c. Der Index-Resizehelper bekommt 4000, entsprechend initial 16 KiB; dessen Cursorreset initialisiert +38. Breite vier waehlt Format 66, sonst 65. Create-/Retry-/Evictionplanung wird vollstaendig aus dem vorhandenen Resizepfad verwendet.

## 3. Vertexpool bei Bedarf anlegen

Originalfunktion 10020540 verwendet Device+40bc. Bei Slot null plant der Adapter eine 48-Byte-Allokation und den dynamischen Konstruktor; erst dessen erfolgreicher Abschluss installiert den Wrapper im Slot. Die hexadezimale Allokationsgroesse 48 entspricht 72 Bytes. Ein vorhandener Slot wird direkt wiederverwendet, auch bei geaendertem Quellschluessel. Der neue Schluessel verursacht keine Hashsuche und keine Neukonstruktion; der urspruengliche Cachekey bleibt erhalten.

Die typisierte Portansicht prueft dazu erfasste Bildgroesse, dynamische Vtable und Owner. Nullallokation, fehlende Createantwort, falsche Bildgroesse und doppelte Allokationsadresse werden sicher abgewiesen. Dies ersetzt keine echte Allocatorimplementierung.

## 4. Zwei unabhaengige Indexpools waehlen

Originalfunktion 10020a90: Nur Quell-Breitengetter vier waehlt Device+40c4. Jeder andere Wert waehlt +40c0. Ein leerer Slot plant eine 40-Byte-Allokation (hexadezimal, also 64 Bytes) und Konstruktion mit Breite vier beziehungsweise zwei. Ein vorhandener Slot bleibt erhalten. Beide Pools besitzen eigene Handles, Cursor, Kapazitaeten und urspruengliche Cachekeys.

Der Quell-Breitengetter fuer die spaetere Offsetdivision bleibt davon getrennt: Beispielsweise kann Getter drei den mit Breite zwei erzeugten Pool verwenden. Getter null wird beim anschliessenden sicheren Divisionscheck abgewiesen; der gemeinsame Adapter veroeffentlicht dann keine bereits geplante Poolanlage.

## 5. Pool, Upload und Renderer verbinden

Die neue Runtime haelt Cache, drei Device-Poolslots, gemeinsame Device-Zaehler, Deferred-Zustand und Scratchpuffer. Ein Request verbindet bedarfsgerechte Konstruktion, Resize bei spaeterem Wachstum, Ringplanung, direkten oder Scratchupload, Rendererbindung und Rueckschreiben der Ringfelder in das vollstaendige Ressourcenbild. Die uebrigen Pools und alle Listenlinks bleiben erhalten. Die Vertexbindung speichert den aktuellen Quellzeiger; die Basisframefelder werden durch den dynamischen Upload nicht als statischer Usagepfad umgeschrieben.

Der Rueckgabewert `offset` entspricht dem originalen Wrapperreturn: Vertexfunktion speichert den Ringelementoffset vor der Bindung und gibt ihn danach zurueck; die Indexfunktion tut dasselbe. Er bleibt getrennt vom uebergebenen Basisvertex. Eine Verbindung dieses Rueckgabewertes mit einem uebergeordneten Draw-Aufrufer ist weiterhin offen und wird hier nicht angenommen.

Requests sind fuer Modellzustand, Renderer und CPU-Ziel atomar. Ein erst nach Upload erkannter Shaderfehler erhaelt insbesondere Cachelisten und leere Poolslots. Diese sichere Porttransaktion beschreibt keine Rueckabwicklung echter GPU-/Allocatoraufrufe.

## Nachweis und Grenzen

`Generate-D3DPools.py` bindet die synthetischen Eingaben per SHA256 an den vorherigen Scratchnachweis. 48 Sequenzen mit jeweils acht Vertex-/Indexrequests wechseln zwischen neuen und vorbesetzten Pools, beiden Erzeugungsbreiten, anderen Breitengettern, geaenderten Schluesseln mit identischem Hashbucket, Scratch-/Direktmodus und weiterhin notwendigen Uploadresizes. Zwoelf Fehlerfaelle pruefen den gemeinsamen Rollback. Sechzehn neue Rusttests decken Konstruktorfelder, unveraenderte Bytes, beide Listen, Slotwiederverwendung, getrennte Indexpools, typisierte Ansichten und spaete Fehler ab.

`Record-D3DPools.py` interpretiert die originalen Lazy-Slotzweige, den Basiskonstruktor, beide dynamischen Konstruktoren, deren initiale Resizehelper, Ring-/Scratch-/Bindekoerper und die relevanten Returnwert-Ladevorgaenge. Allokationsbilder und Allocatorantworten bleiben explizite Eingaben, ebenso Getter, Fuellcallback, Shader und COM-Ergebnisse. SEH, Logging, Zeitmessung, Returns und native Fehlerpfade bleiben ausgeklammert. Geprueft werden nach jedem Schritt vollstaendige 64-/72-Byte-Bilder, alle 4096 Hashkoepfe, globale Liste, Poolslots, Device-/Deferred-/Rendererfelder und CPU-Puffer.

Ergebnis: **67221 Originalinstruktionen**, **48 Sequenzen / 384 Schritte**, **116 Poolanlagen** (32 Vertex, 84 Index), **268 Wiederverwendungen**, **20 weitere Uploadresizes**, **220 COM-Creates**, **10 Scratchwachstumsschritte**, **326 Scratchwiederverwendungen** und **12 sichere Fehlerfaelle**. Die Konstruktorinstruktionen sind in den Lazy-Instruktionszahlen bereits enthalten und werden im Gesamtergebnis nicht doppelt gezaehlt.

**16 neue Tests, insgesamt 780 Workspace-Tests**; Debug-/Releaseberichte bytegleich, Clippy mit `-D warnings`, Format- und Diffpruefung bestanden.

```powershell
python scripts/Generate-D3DPools.py
cargo run -p rc-inspect --bin rc-d3d-pools-check -- analysis/reports/d3d-pools.input.json analysis/reports/d3d-pools.json
cargo run --release -p rc-inspect --bin rc-d3d-pools-check -- analysis/reports/d3d-pools.input.json analysis/reports/d3d-pools-release.json
cargo test --workspace *> analysis/reports/d3d-pools-tests.log
python scripts/Record-D3DPools.py
```

Berichte: `analysis/reports/d3d-pools*`; neue Exporte `analysis/decompiled/d3d-pools.c/.asm`, ergaenzt durch vorhandene Ressourcen-, Indexkonstruktor- und Dynamic-Setup-Exporte. Evidenzschluessel `d3d_pools_validation`. Historische Nachweise behalten damalige Testzahlen und Quellenhashes.

Echte Allocator-/COM-/GPU-Ausfuehrung, uebergeordnete dynamische Draw-Aufrufer, Skinningcallback, Shadererzeugung und Live-Spiel bleiben offen. Die Fixtures sind keine Live-Aufnahmen. Android folgt zum Schluss; das Spiel ist weiterhin nicht spielbar.
