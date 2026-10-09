# Gepackte Passzustaende bis zum Direct3D-8-Aufrufplan

Drei Aufgaben erweitern die rohe Materialuebergabe: gemeinsamer Renderpraefix, nichtmatrizielle Textur-/Buehnenuebersetzung und ein begrenzter Soll-/Ausgabecache fuer geordnete Direct3D-8-Aufrufplaene. `rc-package::d3d_state` erzeugt die Originalwerte und die benoetigten numerischen Methodenargumente. Es ruft keine Grafik-API auf.

## 1. Gemeinsamer Renderpraefix

`Cache::render_prefix` entspricht `1001efb7..1001f165` aus `D3DDrv!1001ed70`. Es uebernimmt die Passbits, Alpha-Referenz und Blendwoerter in die nativen Sollregister; nicht betroffene Register bleiben erhalten. Der Fill-Selektor bei Pass+0c kennt die Zweige 0, 1 und 2; andere Werte lassen die bisherigen Fill-/ColorOp-Werte stehen. Passbit 20 waehlt Cullwert 1, ansonsten wird das ausdrueckliche Cull-Argument verwendet. Die Funktion markiert dieselben Render-/Buehnengruppen als geaendert.

Der Praefix setzt diese numerischen Render-IDs:

| Passquelle | Render-ID / Wert |
| --- | --- |
| Flagbit 0 | 27 / 0 oder 1 |
| Flagbit 1 | 15 / 0 oder 1 |
| Flagbit 2 | 29 / 0 oder 1 |
| Flagbit 3 | 23 / 4 oder 8 |
| Flagbit 4 | 14 / 0 oder 1 |
| Flagbit 5 / Cull-Argument | 22 / 1 oder Hostwert |
| Byte +8 | 24 / Alpha-Referenz |
| Festwert | 25 / 5 |
| Woerter +14 / +18 / +10 | 19 / 20 / 60 |
| Selektorwort +0c | 8 / 2 oder 3; dazu ColorOp 3 oder 4 |

Microsoft dokumentiert diese historischen numerischen [Renderzustands-IDs](https://learn.microsoft.com/en-us/previous-versions/windows/embedded/ms886344(v=msdn.10)). Der Code behaelt Zahlen, damit deren Ursprung nachpruefbar bleibt. Shaderauswahl vor dem Praefix, die Passidentitaets-Abkuerzung, der ColorWrite-Passabschluss und die Deaktivierung unbenutzter Buehnen danach sind noch nicht Teil dieser API.

## 2. Nichtmatrizielle Buehnenuebersetzung

`Cache::stage` rekonstruiert `1001f176..1001f58e` fuer genau eine Buehne. Ein Nichtnullressourcenwort +3c wird als Texturhandle gewaehlt, sonst Wort +38; Nullressource ergibt Nullbindung. Sieben feste Funktionsfelder bleiben bei vorhandenem HardwareShader unveraendert, wie es der Originalzweig vorsieht. Die anderen gepackten Farb-/Alpha-/Adress-/Koordinatenfelder werden in 21 native Sollwoerter entpackt.

Der rohe LOD-Bias in Buehnenwort 1 wird im Pass selbst auf den Geraetewert +4140 gesetzt, falls er strikt kleiner als -100 ist. Die Konstante stammt nachweislich von DLL-Adresse 100726c4 (c2c80000). Gleichheit, NaN, signierte Null und positive Unendlichkeit erhalten die bisherigen Woerter; negative Unendlichkeit nimmt den Ersatzpfad. Es werden keine Floatwerte fuer den Upload neu gerundet.

Die Verbraucher klaeren zwei fruehere Arbeitsbezeichnungen: Die niedrigen drei Nibbles des Buehnenworts 2 landen bei den API-IDs 13, 14 und 25, also den Adressierungswerten. Das historische Artefakt-/Variablenlabel `filter` fuer Geraetewort +466c bezeichnet hier einen rohen Adressierungswert, keinen belegten Min-/Mag-/Mip-Filter. Die zuvor allgemein als Transformationswoerter bezeichneten sechs Woerter 22..27 landen bei IDs 7..10, 22 und 23 (Bump-Umgebungsparameter), nicht in einer allgemeinen UV-Matrix. Die historischen Rohwertberichte bleiben unveraendert gueltig. Die [archivierte Microsoft-DirectX-8.1-Referenz](https://alpyne.me/archive/dx8/directx_cpp/graphics/reference/cpp/d3d/enums/d3dtexturestagestatetype.htm) nennt diese TextureStage-IDs.

Echte Matrixuebernahme bei Buehnenwort 4, Bit 40 ist ausgeschlossen: die API meldet diesen Fall vor jeder Mutation. Fehlende Felder einer Nichtnullressource und Buehnenindices ab 8 werden ebenfalls vorher abgewiesen. Filter-IDs 16..18, Matrixdaten/-Dirtybits, Ressourcenbeschaffung und Lebensdauer bleiben offen. Die alten CPU-Meshbilder behalten ihre explizite Nearest/Repeat-Fixture.

## 3. Soll-/Ausgabecache und Aufrufplanung

`Cache::flush` rekonstruiert die Render-, Buehnen- und Texturgruppen aus `10028f10`, Dirtymaske 13 (Bits 1/2/10). Es vergleicht mit den zuletzt im nativen Cache vermerkten Werten und erzeugt nur Aenderungen, in Originalreihenfolge:

| VTable-Offset | Argumente ohne Geraetezeiger |
| --- | --- |
| c8 | Render-ID, Rohwert |
| fc | Buehne, TextureStage-ID, Rohwert |
| f4 | Buehne, Texturhandle |

Die Nebelparameter IDs 34/36/37 bleiben im Cache zurueckgestellt, solange Soll-ID 28 null ist. Die IDs 52..59 werden nur beim originalen Gate `device+54 != 0 || GIsEditor != 0` ausgegeben. Diese Gruppe sind Stencilzustaende; sie ist nicht die Beleuchtungsfreigabe. Ein wiederholter Texturfaktorvergleich fuer ID 60 wird wie im Original durch den bereits aktualisierten Cache unterdrueckt. Kapazitaeten 0..8 begrenzen Buehnen und Texturen. Nicht unterstuetzte Dirtybits oder groessere Kapazitaeten liefern einen Fehler vor Cachewrites.

`applied` bezeichnet den optimistisch aktualisierten nativen Ausgabecache, keinen bestaetigten GPU-Zustand. Das Original schreibt ihn vor dem Methodenaufruf und ignoriert den HRESULT. Die Gegenprobe gibt fuer die Methoden absichtlich HRESULT 80004005 zurueck; die Originalanweisungen aktualisieren den Cache trotzdem. Der Rust-Code plant Aufrufe und schreibt denselben Cache, ohne sie auszufuehren. Eine spaetere Backend-Anbindung muss Ausfuehrung und Wiederherstellung selbst behandeln.

## Originalbefehls-Gegenprobe

`Generate-D3DState.py` kombiniert 256 systematische Praefixfaelle (64 Flagmuster und vier Fill-Selektoren) mit allen 96 erfolgreichen Materialuebergaben des vorherigen SHA256-geprueften Berichts. Buehnenrohwerte kommen aus den vorherigen Samplerproben; Matrixbit 40 wird fuer diesen begrenzten Pfad ausdruecklich entfernt. Anfangscache, Cull-Argument, Stencil-Gate, Kapazitaet, zusaetzliches Ressourcenwort +38 und LOD-Bias bleiben ausgewiesene Fixtures.

`rc-d3d-state-check` zeichnet nach dem Renderpraefix, nach der Buehnenuebersetzung, nach der Ausgabe und nach einer erweiterten Ausgabe die kompletten unterstuetzten Cachefelder auf. Eine erneut markierte identische Uebergabe erzeugt keinen Aufruf. Anschliessend zeigt Kapazitaet 8 mit freigegebenen Stencilzustaenden die zuvor zurueckgestellten Aenderungen.

`Record-D3DState.py` wiederholt separat die exportierten x86-Anweisungen ueber ein Byteadress-Woerterbuch. Der Praefix, der nichtmatrizielle Buehnenzweig und die Render-/Buehnen-/Textur-Cacheschleifen werden ausgefuehrt; Profiling, Matrix-/Shader-/Buffergruppen sind ausgeschlossen. Die geordneten nativen Methodenargumente, Soll-/Ausgabewoerter, Dirtybits und der mutierte LOD-Bias werden gegen Rust verglichen.

Ergebnis: **352 Uebergaben**, **981509 Originalanweisungen** (90427 Uebersetzung, 891082 Ausgabecache), **31269 geplante Methodenaufrufe**, **587136 Cachewoerter** und **9856 Buehnenwoerter** geprueft. 352 wiederholte Ausgaben ohne Aufruf. Debug/Release bytegleich. Sechs neue Tests; **608 Workspace-Tests**, Clippy `-D warnings`, Format- und Diffpruefung bestanden.

Artefakte: `analysis/decompiled/d3d-{state-consumers.txt,state-cache.c,state-cache.asm,state-cache-callers.txt,pass-consumer.c,pass-consumer.asm,pass-translation.c,pass-translation.asm}`, `analysis/reports/d3d-state{.input,,-release,-validation}.json` und `d3d-state-tests.log`. Die Referenz verifiziert die DLL-Konstante direkt aus dem Original-PE und protokolliert den DLL-Hash.

```powershell
python scripts/Generate-D3DState.py
cargo run -p rc-inspect --bin rc-d3d-state-check -- analysis/reports/d3d-state.input.json analysis/reports/d3d-state.json
cargo run --release -p rc-inspect --bin rc-d3d-state-check -- analysis/reports/d3d-state.input.json analysis/reports/d3d-state-release.json
cargo test --workspace *> analysis/reports/d3d-state-tests.log
python scripts/Record-D3DState.py
```

Naechste offene Verbindungen: kompletter Passabschluss/Shaderauswahl, Matrix- und Filterpfade, Property-/Klassenvorgaben zu nativen Feldern, echte Ressourcen und Grafikbackend. Die neuen Aufrufplaene sind keine GPU-, Bild- oder Live-Spielpruefung. Diffuse-Abdeckung bleibt 271/289. Spielbare Engine weiterhin offen; Android-Pruefung zum Schluss.


## Fortsetzung: Passabschluss

Der nachfolgende Stand in `D3D_PASS_COMPLETION.md` implementiert Reststufen, ColorWrite/Identitaet und den begrenzten mehrstufigen Hardwarezweig. Die hier geprueften 96 Materialpraefixe enthalten 32 kuenstliche Setupwerte -2 (Stufenzaehlbyte 254); diese waren fuer die damalige Praefixpruefung gueltig, werden aber von der neuen maximal achtstufigen Pipeline abgewiesen. 64 gueltige Materialpraefixe sind in deren Originalbefehls-Gegenprobe enthalten. Matrixpfad und feste Shaderauswahl bleiben offen.


Fortsetzung `D3D_TRANSFORMS.md`: Eine zusaetzliche API uebernimmt jetzt rohe Matrixstufen, elf Transformslots und native Transformmasken im geordneten Render-/Buehnen-/Transform-/Texturplan. Der bisherige nichtmatrizielle API-Vertrag bleibt erhalten. Feste Shaderauswahl, Ressourcen-/Transformquellen und Backend bleiben offen.
