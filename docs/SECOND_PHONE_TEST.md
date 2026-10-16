# Zweiter Handytest – Korrekturblock in Arbeit

Stand: 16.10.2026. Grundlage ist der Gerätebericht zu 0.50.0. Der gesamte angeforderte Inszenierungsblock ist **noch nicht abgeschlossen**. Die veröffentlichte 0.50.0 bleibt als Vergleich eingefroren; dieser Zwischenstand ist keine neue Gerätefreigabe.

## Umgesetzte Korrekturen

### Soundidentitäten

Der zusätzliche Droidenton wird bei GEO_01A in die äußere Opening-Bank eingefügt. Die innere RCPLAY04-Bank bleibt unverändert. Dadurch verschieben sich die absoluten Intro-/Abseilrollen nicht mehr. GEO_01B behält seine eigenständige Bank.

Der finale A-Builder schreibt zusätzlich für alle Skript-, Intro- und Abseil-Notify-Referenzen eine Bindung aus ursprünglicher Rolle, Originalquelle und PCM-Fingerabdruck. Der Loader löst diese Identitäten auf, auch wenn eine spätere Importstufe die innere Bank erweitert. Fehlende oder widersprüchliche Bindungen werden abgewiesen. Historische Pakete ohne Manifest bleiben lesbar. Bereits fehlerhaft mit einem inneren Droidenton erweiterte A-Eingaben werden beim Neubau abgewiesen; Ausgangspunkt sind die gepinnten Eingaben.

`scripts/Test-OpeningSoundBindings.py` prüft den tatsächlichen nativen Audiopuffer eines RCPLAY oder des finalen Kampagnen-ZIP. Die erste korrigierte Szene besteht mit **24 Referenzen / 23 unterschiedlichen Cues** und passenden PCM-Laufzeiten. Mit `--ticks 1800` wird zusätzlich die reale native Ereignisfolge protokolliert.

**Noch offen:** Im korrigierten Ablauf starten die Originalaktionen2502/2503 weiterhin im selben Tick. Aktion2502 hat kein `bWaitForFinish`, setzt aber `bDontOverrideOtherSounds`. `Gameplay.ACTION_PlaySound` benutzt `PlayOwnedSound`, SoundSource/Slot und bei wartenden Sounds `GetSoundDuration + 0.1`. Die bisherige Portabbildung berücksichtigt diese Semantik nicht vollständig. Richtige Dateien und Laufzeiten allein sind deshalb keine abgeschlossene Dialogabnahme; eine pauschale SoundPool-Verzögerung wäre ebenfalls unzureichend.

### Gemeinsame Materialentscheidung und vorhandene Szenen

`assets::Assets::opaque` entscheidet anhand aufgelöster Originaldefaults für Texture, Shader, FinalBlend und unterstützte Hs-Materialien sowie Material-/UV-Wrapper. Diese Entscheidung gilt für statische und animierte Importe. Adapter, die eine Basisschicht selbst auswählen, reichen die Transparenzentscheidung des umgebenden Materials weiter. Der bisherige separate Skeletal-Override entfällt.

Vier zusätzliche deklarierte Stufen in `config/scene-builds.json` bearbeiten auch die bereits vorbereiteten A/B/C/Fabrik-Szenen. `rc-scene-material-build` ordnet Weltflächen und eingebettete Snapshots über exakte originale Weltkoordinaten zu und berücksichtigt Actor-Skins. Historische Farb-Surface-Nummern sind keine Materialidentität. Widersprüchliche Flächenzuordnungen und ungeklärte, maskierte oder transparente Materialien bleiben erhalten. Geteilte Texturen erhalten private Kopien, sobald nicht alle Benutzer einen Opazitätsnachweis haben. Ausschließlich opake Texturen werden ohne zusätzliche Kopie normalisiert, damit C innerhalb des Metadatenlimits bleibt.

Erster kompletter Neubau:

| Karte | Korrigierte Weltflächen | Korrigierte Flächen in eingebetteten Snapshots |
|---|---:|---:|
| GEO_01A | 4.656 | 638 |
| GEO_01B | 32.248 | 12.124 |
| GEO_01C | 18.650 | 984 |
| GEO_03A | 36.024 | 0 |

Die638 A-Prop-Flächen umfassen die nachgewiesenen52 unsichtbaren Flächen am alten Geschützbunker und586 an dessen Luke. Dieser Bunker ist nicht Scorchs spätere Sprengstelle.

Die Ausgabe `material-refresh.json` hält Entscheidungen, ungeklärte Klassen und veränderte Flächen getrennt fest. Dies ist eine Opazitätskorrektur; vollständige Combiner-Auswertung, sämtliche UV-/Shadereffekte und ein einheitliches höheres Texturprofil für beide Wracks sind weiterhin offen.

### Intro und Spieleransichten

Die laufende Intro-Animation verbraucht ihre Root-Translation bereits beim Actor-Schritt mit Kollisionsprüfung. Für die anschließende Geometrieausgabe wird diese Translation jetzt gesperrt, wie bei anderen Root-Motion-Pfaden. Die Originalposition im Gunship wird nicht pauschal auf den Boden versetzt.

`RC_CAPTURE_VIEWS=1` erweitert den bestehenden durchgehenden Checkpoint-Replay um normale Spieleransichten, Kamerabasis und Zustände an festen Introzeitpunkten, Abschnittswechseln und Checkpointproben. Diese Ansichten verwenden aufgezeichnete reguläre Eingaben; Position, Blickrichtung und Gesundheit werden dafür nicht gesetzt. Eine vollständige Abnahme von Füßen, Mesh-Ursprung, Abseilkamera und Übergängen ist damit noch nicht behauptet.

## Geprüfter Zwischenstand

Der zentrale Prüfbefehl besteht mit jeweils1.128 Debug-/Release-Bibliothekstests, zwei zusätzlichen Materialimport-Regressionen, Doc-Tests, Clippy aller Targets, Java-Prüfungen und Buildvertrag. Der vollständige Neubau umfasst18 Stufen. Der reguläre A/B/C-Lauf bis zur Fabrik und25 Zustands-/Bild-/Fortlauf-/Neustartproben bestehen. Zusätzlich wurden51 Spieleransichten aufgezeichnet; diese sind Diagnosebelege und keine vollständige Geräteabnahme.

Das Arbeits-ZIP besteht die native Sound-/PCM-/Laufzeitprüfung aller24 Referenzen sowie die bestehende Prüfung beider Droidentöne und der Rampentextur. Nachweise: `checks-final/validation.json`, `pipeline/scenes/validation.json`, `pipeline/scenes/runtime-validation/validation.json`, `final-package-sound.json` und `final-package-regression.json`, jeweils unter `build/sequence-corrections`.

Das Arbeits-ZIP benötigt den dazugehörigen neuen nativen Loader; es ist **kein Testpaket für die unveränderte0.50.0-APK**. Android-Build, Audio-Hörprüfung und physische Abnahme folgen nach Abschluss der noch offenen Inszenierung.

## Nächster notwendiger Abschnitt

1. Die fehlende PlayOwnedSound-/Slot-/Wartesemantik anhand der Originalquellen umsetzen und die komplette Dialogfolge erneut hören und zeitlich prüfen.
2. Introverankerung und Abseilkamera an festen Zeitpunkten prüfen; Wrack-Mips und Speicherbudget gemeinsam festlegen.
3. Originale Verwundungskette ab `PlayerExplosionFactory` sichtbar und hörbar anbinden; der gegenwärtige generische Sprite am Spieler reicht nicht aus.
4. Bacta: `Load → FinishAnim → Eye-Attachment/PawnEffect → Heilung → Holster/Entfernen/FadeOut` implementieren und Sounds aus Defaults/Notifies beziehen. Die vorhandene Heilungsformel ist hierfür nur die Grundlage.
5. Barrieren einzeln nach Originalakteur behandeln. Die Bacta-/Exit-Blocking-Brushes haben im gegenwärtigen Import leere Render-Snapshots; ihr Kollisionsabschalten erklärt keine verschwindende sichtbare Wand. Sichtbare Pfadbarriere, Tunnel964 und Karma-Wände benötigen jeweils ihren tatsächlichen Zerstörungs-/Bewegungsablauf.
6. Anschließend Scorchs Sprengbunker, Tür und A→B-Übergang aus normaler Spieleransicht abnehmen, neue APK und Kampagnenpaket gemeinsam bauen und auf Android prüfen. Erst danach den gesamten Korrekturblock als erreicht markieren.

Arbeitsnachweise liegen unter `build/sequence-corrections`; alte Release-Artefakte werden nicht überschrieben. Ein vorhandener Spielstand verwendet seine archivierte Szene weiter; ein neuer Paketimport ersetzt diese Checkpoint-Szene nicht stillschweigend.
