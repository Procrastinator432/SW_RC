# Deckende Materialien nach dem Handytest

Stand: 28.10.2026 · Materialpaket für **APK 0.74.0-music-groups**.

Die bestätigten Alpha-Fehler an Tür/Umrandung zum zweiten Level und zahlreichen Flächen auf GEO_01B sind korrigiert. Derselbe Materialfehler ist auch auf C und am Fabrikeinstieg behoben. Die Egoansicht mit Händen/Rüstung beim Abseilen bleibt ein gesonderter, noch offener Darstellungspunkt.

[Neues Szenenpaket](../build/material-coverage/package/mission1-scenes.rcpack.zip) · [Prüfbericht](../build/material-coverage/verification-summary.json) · [Vorher](../build/material-coverage/final-views/b-0-before.png) · [Nachher](../build/material-coverage/final-views/b-0-after.png)

**Installation:** Die vorhandene APK **0.74** kann bleiben. Das neue Paket importieren und **Neue Mission starten** wählen. Alte Checkpoints verwenden weiterhin ihre alten, unveränderlichen Szenen; Fortsetzen oder Kartenneustart aktualisiert deren Materialien nicht.

## Ursache und Änderung

`HsBumpDiffSpec`, `HsBumpIllumSpecMask` und verwandte Hardware-Wrapper besitzen kein `OutputBlending`. Die bisherige Abfrage dieses Shader-Feldes scheiterte und ließ die Textur-Alphawerte unverändert. Der vereinfachte Missionsrenderer behandelte diese Werte danach als Deckung und blendete Teile deckender Originalflächen aus. Betroffen waren unter anderem `Door2`, `Door3`, `DoorBreach`, `DoorFrame`, `HallwayFloorFinal`, `doortest` und `SpirePileShade`.

Die Auflösung folgt jetzt der tatsächlich referenzierten **ShaderImplementation**. Nur wenn die originalen, aufgelösten Hardware-Zustände **AlphaBlending=false und AlphaTest=false** ergeben, wird die abgeleitete Deckung auf 255 gesetzt. Der ursprüngliche Textur-Alpha wird damit nicht pauschal als Transparenz interpretiert. Dessen übrige Verwendungen im vollständigen Shader, etwa Materialmasken, werden durch diesen Diffuse-Adapter nicht nachgebildet.

Der gemeinsame Materiallader und die Reparatur vorhandener Pakete verwenden diese Regel. Der Paketpfad verarbeitet die Welt, eingebettete RCSC-Objektsnapshots und deklarierte Visual-Materialien. Lokale CTM-Materialreferenzen können über das Maps-Verzeichnis aufgelöst werden. Maskierte, geblendete, unbekannte und widersprüchlich zugeordnete Flächen bleiben erhalten. Bei gemischter Verwendung gibt es eine private Texturkopie für die belegte deckende Fläche. RCPLAY15-Musikheader, Partitur und Bank werden bytegleich durchgereicht.

| Karte | Korrigierte Weltbelegungen |
|---|---:|
| GEO_01A | 4.603 |
| GEO_01B | 150.469 |
| GEO_01C | 27.441 |
| GEO_03A | 62.608 |

Zusätzlich werden 8.334 Belegungen in eingebetteten B-Tür-/Objektsnapshots korrigiert. Das sind korrigierte Dreieckbelegungen über mehrere Szenen, keine Behauptung gleichzeitig sichtbarer Flächen. Geometrie, RGB-Farben, UVs, Startanker, Kollision und Missionsprogramme werden nicht geändert.

## Prüfung

- Je **1.518 Debug-/Release-Bibliothekstests**, zwei Materialimport-Testgruppen, **82 Java-Audiogruppen**, Clippy, Buildvertrag und übrige zentrale Prüfungen bestehen.
- Strukturvergleich aller vier Szenen: RGB unverändert, Alpha ausschließlich unverändert oder auf 255 gesetzt; Geometrie, UVs, Anker, Kollision und sonstige Metadaten bleiben erhalten. Die Originalzustände jeder korrigierten Weltmaterialklasse sind im Audit gespeichert. Musikheader, Partitur und Ressourcen sind exakt gleich.
- Zweiter Reparaturdurchlauf: null weitere Änderungen; alle ZIP-Nutzdaten bleiben bytegleich. Die privaten Kopien wachsen nicht bei Wiederholung.
- A-/B-Vergleich mit normalen Replayeingaben: **36.865 Eingaben und 161 Zustandsvergleiche**, identische Gameplayzustände. Kontrollansichten umfassen den Auftakt, A-Ausgang und B-Einstieg. Am B-Einstieg ändern sich 112.140 von 256.000 Bildpixeln; die vorher unsichtbare Tür und Umrandung werden sichtbar.
- Kompletter A/B/C→Fabrik-Lauf mit **49.795 Eingaben, 1.230 WAV-Ereignissen, 39.836.000 Stereo-Frames**, drei Übergängen und vier kalten Restores besteht mit dem neuen Paket.
- Android-35-x86_64-Emulator mit unveränderter APK 0.74: produktiver Import, GPU-Ansichten auf B/C, nativer Ton, Pausensidecar und kaltes Fortsetzen bestehen. Das ist keine freie physische Handy-/Hörabnahme.

## Abseilstelle: eigener offener Punkt

Der Nutzer präzisierte die Lücke auf **Seil, Hände oder Rüstung**. Die vorhandene Seiltextur `MiscAnimPropTextures.Rope.Rope` ist im bisherigen Paket bereits deckend; ihre Originalflags `bMasked=false`, `bAlphaTexture=false` sind aufgelöst. Die Texturkorrektur ändert die ersten A-Ansichten daher nicht.

Im aktuellen Abseilhost wird `EntranceRope600` des Spielermodells für die Bewegung ausgewertet. Eine eigene Ego-Hand-/Körperdarstellung ist dort nicht angeschlossen. Das ganze Körpermodell einzublenden ist keine ausreichende Reparatur: Ein lokaler Versuch verdeckte die Sicht mit Kopf-/Innengeometrie und wurde vollständig verworfen. Der Originaldefault `Pawn.bOwnerNoSee=true` unterstreicht, dass die Egoansicht nicht mit der vollständigen Fremdansicht gleichgesetzt werden darf. `RopeSet.CloneEntrance600` existiert, sein tatsächlicher Receiver wird weiterhin nicht ohne Beleg angenommen.

Die nächste gezielte Arbeit ist deshalb die originale Ego-/Kamera-/Meshbindung dieser Sequenz. **Hände und Rüstung werden mit dieser Lieferung nicht als repariert ausgewiesen.** Unbekannte Combiner-, Falloff- und sonstige Spezialmaterialien sind ebenfalls keine pauschal gelöste Materialparität.

## Weitere Handy-Rückmeldungen

- Auf A fiel keine Musik auf. Das entspricht dem bisher belegten frischen Einstieg ohne zugewiesenen Titel; die vollständige Menü-/Filmübernahme bleibt separat offen.
- Teilweise gleichzeitige Sprache, eventuell nach schnellem Erreichen späterer Orte. **Fehlerverdacht offen**, weder als normales Verhalten noch als nachgewiesener Audiotreiberfehler eingestuft. Reproduktion über schnelle Triggerfolgen und Abgleich der originalen Dialog-/Ownerregeln erforderlich. Dieser Materialblock verändert keine Sprachplanung.

## Wiederholen

`Build-MaterialCoverage.py` benötigt explizit Builder, eigene GameData, Ausgangspaket und einen neuen Ausgabeordner. `Test-MaterialCoverage.py` prüft das alte/neue Paket und den gebundenen Audit; `Test-MaterialViews.py` vergleicht normale Replays. Die zentrale Prüfung bleibt `./build.ps1 -Target Check`. Ausgangspaket, frühere APK und eingefrorene 0.74-Lieferung bleiben unverändert. Es wurde weder committet noch getaggt oder gepusht.
