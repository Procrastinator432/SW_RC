# Karma-Impulse und Interaktionen – 0.52.0

Stand: 20.10.2026, versionCode 56. Dieser Abschnitt erweitert die vier importierten Barrieren in GEO_01A. Die fünf Arbeitsbereiche sind Originalarithmetik, KarmaProp-Regeln, Geschosstreffer, Explosionen und Spielerberührung. Allgemeine Karma-Akteure und die vollständige MathEngine bleiben eigene Aufgaben.

## Originalbefunde und Umsetzung

**1. Einheiten und Impulsarithmetik.** `AActor::execKAddImpulse` bei `10591ab0` in der ursprünglichen `engine.dll` multipliziert Impuls und Angriffspunkt mit der Float-Konstanten bei `1067eacc`: `0x3ca3d70a`, entsprechend 0,02. Damit entsprechen 50 Welteinheiten einer Karma-Einheit. Der bisherige Import in 0.51.0 verwendete irrtümlich 100: Die Kollisionskörper waren ungefähr doppelt so groß wie ihre sichtbaren Originalmeshes. Der neue Import korrigiert Hüllen, Schwerpunkt und Trägheitsmatrix gemeinsam. Eine Größenprüfung gegen die sichtbaren Meshgrenzen verhindert diesen Fehler bei den ausgewählten vier Körpern künftig.

Der Helfer `10596570` addiert linearen Impuls und das Kreuzprodukt aus Hebelarm und Impuls. Die ersten beiden Drehimpulskomponenten werden vor der Addition in Float zwischengespeichert; die dritte bleibt bis zur Addition im x87-Register. `rc-package::karma_impulse` bildet diese Speicherstellen nach. `scripts/Record-KarmaImpulse.py` liest die exportierten Originalinstruktionen und erzeugt 256 begrenzte Vergleichsfälle. Die Tests vergleichen Float-Bitmuster. Dies ist ein Replay der exportierten Arithmetik mit vorbereiteten Körperdaten; es führt weder die Original-DLL noch deren vollständige VM-, Modell- oder FPU-Prozessumgebung aus.

**2. KarmaProp-Regeln.** Der Builder liest `bKTakeShot`, `DamageThreshold`, anfängliches `Physics`, `LifeTime`, `Event` und `CollisionRadius` aus den ursprünglichen Klassen-/Instanzdaten. Die vier ausgewählten Akteure 909/910/912/913 beginnen mit `PHYS_None`, aktiviertem `bKTakeShot`, Schwelle 0, Lebenszeit 0 und ohne Folgeereignis. Nicht unterstützte Lebenszeiten, Ereignisse und anfängliche Physikvarianten werden beim Import abgewiesen. Wie in `KarmaProp.AffectKarma` sinkt die ganzzahlige Schwelle bei Schaden; ein Treffer allein aktiviert einen zunächst festen Körper nicht. Erst das bestehende `KnockOverWall`-Ereignis schaltet diese Barrieren frei. Das ursprüngliche Trigger-Verhalten erlaubt anschließend auch eine Schussreaktion.

**3. Geschosse.** Die DC-15s übernimmt `KDamageImpulse=200` aus ihrem originalen DamageType. Der erste tatsächliche Welttreffer bestimmt Körper und Angriffspunkt; Treffer hinter einem Hindernis werden nicht weitergereicht. Neben dem Geschosslauf ist der von GEO_01A verwendete bestehende Hitscan-Pfad angeschlossen. Der Impuls verändert Geschwindigkeit und Rotation entsprechend Masse, Trägheit und Hebelarm und weckt einen ruhenden freigegebenen Körper.

**4. Explosionen.** Der Thermaldetonator verwendet die Originalwerte Impuls 800 und Aufwärtsanteil 700. Der ausgewählte `Actor.HurtRadius`-Pfad berücksichtigt Entfernung und Kollisionsradius; der Aufwärtsanteil wird vor dem gemeinsamen Entfernungsfaktor addiert. Sichtprüfungen verhindern Wirkung durch vorgeschaltete Hindernisse. Angriffspunkt ist die bewegte Actor-Position, wie im ursprünglichen Skript. Eine Explosion ohne positiven Schaden beziehungsweise Radius löst diese Reaktion nicht aus. Bei exakt identischen Quell-/Zielpositionen verwendet der Port eine sichere Nullrichtung und den Aufwärtsanteil.

**5. Spielerberührung.** Geh- und Fallkontakte können freigegebene, schlafende Barrieren wieder aufwecken und schieben. Die Originalmasse des Spielers und die importierte Körpermasse/-trägheit begrenzen die Impulsübertragung; voneinander weg bewegte Kontakte erzeugen keinen neuen Stoß. Aufliegende Bodenkontakte bleiben davon getrennt. Diese Kopplung ist ein eigener Kontaktadapter des Ports, keine Rekonstruktion des ursprünglichen nativen Pawn/Karma-Solvers. Die neue Variante indiziert die vollständige statische Szenengeometrie, damit weggeschobene Körper auch außerhalb ihres ursprünglichen Umfelds Weltkontakte finden.

## Spielstände und Verwendung

Die verbleibenden Schadensschwellen gehören zusammen mit Pose, Geschwindigkeiten und Schlafzustand zum Checkpoint. Ein fehlendes Modul oder eine falsche Anzahl von Schwellen wird vor der Zustandsübernahme abgewiesen. Die neuen Szenenbindungen sind optional: Alte Szenen und ihre Spielstände behalten ihre bisherigen Körpermaße und Abläufe. Für die korrigierte Geometrie und die neuen Interaktionen daher das zusammengehörige APK und Szenenpaket installieren/importieren und eine neue Mission starten.

## Reproduzierbare Prüfung

```powershell
./build.ps1 -Target All -SceneInputs local-assets/mission1/scene-inputs.json `
  -GameData 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' `
  -OutputDirectory build/karma-interactions/complete

./scripts/Test-PresentationAndroid.ps1 `
  -Inputs build/karma-interactions/complete/scenes/resolved-inputs.json `
  -PresentationReport build/karma-interactions/complete/scenes/player-presentation/validation.json `
  -OutputDirectory build/karma-interactions/android

python -X utf8 scripts/Test-CampaignAndroid.py `
  --apk build/karma-interactions/complete/artifacts/republic-commando-0.52.0-karma-interactions.apk `
  --package build/karma-interactions/complete/artifacts/mission1-scenes.rcpack.zip `
  --checkpoints build/karma-interactions/complete/scenes/runtime-validation `
  --output build/karma-interactions/campaign-android
```

Der zentrale Szenenbau führt zusätzlich `rc-karma-check` aus: In echten aufgezeichneten Missionszwischenständen werden vorbereitete eingehende Schuss-/Zünderereignisse durch den gemeinsamen Trefferverteiler beziehungsweise den regulären Granatenlauf geleitet. Zusätzliche synthetische Hindernisse prüfen die negativen Sichtfälle. Körperpose, Spielerposition und Gesundheit werden dabei nicht gesetzt. Zwei exakte Checkpoint-Fortsetzungen und vier atomar zurückgewiesene beschädigte Schwellenzustände gehören zu dieser Prüfung. Das ist ein gezielter Integrationstest; es ersetzt keinen manuellen Durchlauf aus Spielerperspektive.

Die ursprüngliche DLL hat SHA-256 `f8b3fc9d62414828a1c31d8b5f3b2bcb951f7c0f142fdf2a9846c98d7b5d35a5`. Decompiler-Ausgaben und Instruktionsexport liegen unter `build/karma-interactions/original`. Die versionierte Fixture enthält DLL- und Instruktionshash. Vollständige Bewegungsparität mit MathEngine, subjektive Hörprüfung und ein neuer Lauf auf einem physischen ARM64-Handy sind damit nicht belegt.

## Abschlussnachweise

Der zentrale Offline-Gesamtbau besteht mit je **1.167 Bibliothekstests** in Debug und Release, Clippy, Java-Prüfungen, beiden Android-ABIs, **23 Szenenstufen** und **25 Missions-Checkpointproben** durch A/B/C bis zur Fabrik. Die Präsentationsprüfung erreicht A→B nach **19.811 regulären Eingaben** mit **39 Spieleransichten**, neun exakten Wiederherstellungsfortläufen und drei atomaren Ablehnungen beschädigter Körperzustände. Die vier Barrieren sind am Ende freigegeben und schlafen. Hinzu kommen die zwei Fortsetzungen und vier korrupten Schwellenzustände der neuen Impulsprüfung. Der tatsächliche Java-GPU-Parser liest alle 39 Frames und das 43.475.920-Byte-Weltpaket unter 96 MiB Heap.

Der Android-x86_64-Nativlauf erreicht A→B mit 20 HP, acht exakt neu geladenen Zwischenständen und 60 weiteren Schritten in B. Acht unabhängige Abschnitte laufen anschließend über die produktiven JNI-, GLES- und SoundPool-Klassen: Intro, Verwundung, Bacta, Barrieren, deren Fallbewegung, Geschützstellung, Scorchs Sprengung und Tür. **16 echte PNGs**, 34 gestartete Einmalwiedergaben und 13 Ambient-Streamstarts sind geprüft. Das getrennte Prüf-APK gehört nicht zur Lieferung. Der gezielte Schuss-/Granatentest verwendet die oben beschriebenen vorbereiteten Ereignisse auf dem Desktop; dieser Android-Lauf prüft den zusammenhängenden Missionsablauf und die Darstellung.

Bacta-Abbruch/erneutes Benutzen, alle sechs Original-Notifies, Dauerton, Partikel und gespeicherte Phasen bestehen mit der endgültigen nativen DLL. 41 Cue-Referenzen auf 29 Originalcues sowie die 7.610.358-Byte-Soundbank sind geprüft. Am endgültigen ZIP bestehen Original-Gegner-PCM, Rampenalpha und vier historische Spielstände. Vier zusätzliche Szenen-/Checkpointpaare aus 0.51.0 werden einschließlich ihres vollständigen gespeicherten Zustands unverändert wiederhergestellt.

Die maßgeblichen Berichte liegen unter `build/karma-interactions/complete/validation.json`, `complete/scenes/karma-interactions/validation.json`, `android/validation.json`, `legacy-physics.json`, `bacta-lifecycle.json` und `package-validation.json`. Das zentrale Buildprotokoll nennt den Ausgangscommit und den damals noch uncommitteten Arbeitsstand; die abschließende Quellzuordnung erfolgt separat in `source-and-delivery.json`.

Am normalen Liefer-APK besteht zusätzlich die Kampagnenbedienung: echter Paketdialog und nativer Import, Erhalt vorhandener Spielstände, atomare Ablehnung eines unvollständigen Pakets, Kaltstart sowie exakte Wiederaufnahme nach Android-Hintergrundkill. Bei 640×360 dp sind Touch-Ziele mindestens 48 dp groß; Bewegung, Schuss, Sprung, beide Blickachsen und gespeicherte Empfindlichkeit bestehen. Die horizontale Drehung beträgt 0,750 gegenüber 0,375 Radiant bei 200 gegenüber 100 Prozent. Alle drei automatischen Übergänge A→B→C→Fabrik übernehmen den echten abgeschlossenen Zustand. Für diese Übergänge verwendet der UI-Test echte Replay-Endspielstände in privatem Testspeicher; es ist kein vollständiger manueller Touch-Durchlauf.

Die zusammengehörige Lieferung liegt unter `build/releases/0.52.0`:

- `republic-commando-0.52.0-karma-interactions.apk`, SHA-256 `a5361d150e6b71fb9b0eb86ef02228c0842a6410983803b92ee5484e99717630`.
- `mission1-scenes.rcpack.zip`, SHA-256 `df9499437b0ac9a1eb8793c5ad068c046738023fcccc99ad970418d4726cd9fb`.
- `SHA256SUMS.txt`, `verification-summary.json` und `source-and-delivery.json` ordnen Dateien, Prüfungen und Quellstand zu. Das normale APK enthält beide ABIs und keine Prüf-Activity beziehungsweise deren DEX-Klasse; seine Einträge stimmen mit dem nach der Probe wiederhergestellten normalen Build überein.
