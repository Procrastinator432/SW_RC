# Windows-Abnahme des Linux-Quellstands

Stand: 25.10.2026 · übernommener Commit `e6a2438`.

Die Commits `d76b158` und `e6a2438` wurden auf Windows per Fast-forward übernommen und neu geprüft. Die zuvor wegen fehlender Linux-Missionseingaben zurückgestellte Prüfung der Audio-Geometrieanbindung ist damit abgeschlossen. Am Rust-Spielcode war keine Korrektur erforderlich; zwei Prüfskripte sind an die neue Quellenlebensdauer angepasst.

## Bestandene Prüfungen

- Zentraler Windowscheck: je **1.256 Debug-/Release-Bibliothekstests**, Format, Materialimport-/Doc-Tests, Clippy aller Targets, Native-/Inspectorbau, Java und Buildvertrag.
- Sieben frische Originalvergleichsskripte für Welt-Audio, Controller, Quellen, Befehle, Aktivierung, Upload und FALSource-Treibersteuerung. Die jeweiligen eingefrorenen Fixtures stimmen vollständig überein. OpenAL-Aufrufe und Treiberantworten bleiben die in den Fachtexten dokumentierten kontrollierten Grenzen.
- Original-Geometrie und Zonendistanzen: vier Karten, je 4.096 Tabellenbytes, 820 Linien-/Regions-Abfragepaare sowie die synthetischen Originalfälle. Actor-/Geometrieimport und Serialisierung bestehen.
- Neu erzeugte 30 Szenenstufen: regulärer Ablauf A→B→C→Fabrik, 25 Checkpointproben mit Zustand, Darstellung, Fortlauf und Neustart, Karma-Interaktionen, Waffenanimation und vier Foley-Fixtures.
- 36 ältere Spielstände aus neun Lieferständen 0.52.0–0.60.0 mit ihren unveränderten zugehörigen Szenen exakt wiederhergestellt; neun alte A-Einstiegsbilder bleiben identisch.
- Normaler A-Quellenlauf mit 19.319 Eingaben: acht echte Aktivierungen, alle fünf Feuer-Freigaben, Bacta, Ausblendung, Radiuszustände, kaltes Laden und atomare Abweisung beschädigter Geometriezustände. Eine Ausgabe kehrt mit derselben Aktivierung nach Radiusunterdrückung zurück.
- Native Soundbanken mit unverändertem PCM, unveränderliche Einmalgeräuschparameter und Audio-ABI-Abfragen. Der tatsächliche Android-Frameparser liest 39 Spielerframes und das Weltpaket bei 96 MiB Heap.
- Androidbau für ARM64 und x86_64. Die zusätzliche Android-Präsentationsprüfung auf x86_64 besteht mit acht GLES-/SoundPool-Abschnitten und 16 Bildern: Intro, Verwundung, Bacta, Barrierenbeginn und -bewegung, Scorchs Bunker, Sprengung und Tür. Der native Android-A-Lauf erreicht den Wechsel nach B.
- Normale APK: echter Paketimport einschließlich Abweisung eines defekten Pakets, Erhaltung vorhandener Checkpoints, Hintergrund-Prozessverlust, kompakte Touch-Ziele, beide Blickachsen, Empfindlichkeit, Halbautomatik, Waffenkanäle/Foley und Originalparameter. Räumlicher Ton, Bacta per Touch, kaltes Fortsetzen, tatsächlicher Streamstopp nach Fade und alle drei automatischen Kartenwechsel bestehen.

## Korrekturen an den Prüfern

`Test-SpatialAmbient.py` leitete neue Aktivierungen bisher aus den ausgegebenen Quellen ab. Der neue Geometriehost kann die Ausgabe bei Radius ≤ 0 unterdrücken, während die gespeicherte Stimme weiterlebt. Im echten Lauf erschien Owner 950 bei Tick 10531 wieder; seine Aktivierung stammte noch von Tick 10060. Die gespeicherte Priorität 0,01 war daher korrekt, der Prüfer verglich sie mit der späteren Spielerposition. Er verfolgt nun die persistenten Stimmen, prüft die Priorität am tatsächlichen Aktivierungszeitpunkt und erfasst auch lautlose Starts. Bei diesem Lauf wurden acht echte Starts und eine Rückkehr der Ausgabe ohne Neuaktivierung geprüft.

`Test-CampaignAndroid.py` unterscheidet beim Blicktest die unveränderlichen Auswahl-/Aktivierungswerte von den fortlaufenden Radius- und Regionsdaten. Seed, Owner, Sound, Aufnahmezeit, Priorität und Playbackparameter müssen gleich bleiben. Kaltes Fortsetzen prüft weiterhin den vollständigen gespeicherten Audiozustand. Zusätzlich muss die gemessene Blickgeste ihren erwarteten Winkel erreichen; die Touch-Nutzen-Probe wartet auf die laufende Ansicht. Sollwerte und Toleranzen sind nicht erweitert.

## Nachweise und Prüfbau

[Gesamtbericht](../build/linux-sync-20261025/verification-summary.json) · [Quell- und Artefaktzuordnung](../build/linux-sync-20261025/source-and-verification.json)

- [Normale APK](../build/linux-sync-20261025/android/artifacts/republic-commando-0.60.0-ambient-lifecycle.apk)
- [Passendes Szenenpaket](../build/linux-sync-20261025/package/artifacts/mission1-scenes.rcpack.zip)

Dies ist ein Prüfbau des übernommenen Quellstands mit dem noch unveränderten Versionsnamen 0.60.0. Er ist anhand seiner Hashes dem neuen Quellenhost zugeordnet; die früher eingefrorene Lieferung unter `build/releases/0.60.0` wurde nicht ersetzt. Die Prüfkorrekturen und dieser Bericht liegen lokal zusätzlich zum übernommenen Commit vor.

Diese Abnahme wurde unter Windows durchgeführt. Das Linux-Setup wurde hier nicht erneut ausgeführt; Python und PowerShell wurden syntaktisch geprüft und der Windows-Buildpfad vollständig ausgeführt. Ein optionaler Bash-Syntaxlauf war auf diesem Host wegen des Git-Bash/MSYS-Starts nicht nutzbar. Eine freie Hör-/Bild-/Touch-Abnahme auf einem physischen ARM64-Handy steht weiterhin aus. Die neuen allgemeinen Audio-Kerne werden durch diese Prüfung nicht zu einem angeschlossenen allgemeinen OpenAL-/Android-Mixer erklärt.
