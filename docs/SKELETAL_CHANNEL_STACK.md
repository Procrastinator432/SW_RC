# Aktive Kanalfolge und gemeinsame Pose-Übernahme

Original: `USkeletalMeshInstance::ApplyAnimation` **10509dc0**, begrenzte Stage
**1050a615..1050a719** mit Referenzfallback **1050ab6f..1050abe4**.
Rust: `skeletal_channel_stack.rs`.

Diese Stage beginnt nach Instanzpuffer-/Inverse-Bind-Matrix-Vorbereitung,
MeshToWorld und Linkup-Refresh. Der separate Root-only-Zweig ist bereits zuvor
abgezweigt. Sie endet vor Hierarchiematrizen, Directors und Bounds.

## Verhalten

- Byte +61 ungleich 0 überspringt außerhalb des Editors die Stage vollständig.
  Editorbetrieb wertet trotz dieses Cachewerts aus.
- Wort +11c ungleich 0 sperrt Kanalaufrufe und wählt die Referenzpose.
- Sonst werden Kanäle in aufsteigender Arrayreihenfolge geprüft. Das bestehende
  `IsChannelActive` bestimmt Aktivität und vollständige Verdrängung durch spätere
  Kanäle. Ein verdrängter Kanal bekommt weder Lookup noch History-Update.
- Jeder aktive Kanal erhält dieselbe **unveränderte vorherige Instanzpose** und
  den gemeinsamen, durch vorherige Kanalaufrufe bereits geänderten Scratch.
- Liefert mindestens ein ApplyAnimChannel true, werden danach alle Instanz-
  Rotationen und Positionen aus dem Scratch übernommen. Das gilt auch für einen
  gültigen Kanal mit leerem Knochenintervall. Liefert keiner true, wird die
  Referenzpose kopiert. Ein späterer verdrängender Kanal ohne gültige Sequenz
  kann deshalb zum Referenzfallback führen.
- Bei einem Rust-Hostfehler bleiben lokale Instanzpose und Cachebyte unverändert;
  vorherige Scratch- und Kanalwortänderungen bleiben bestehen. Dieser sichere
  Fehlerpfad ergänzt die nativen Speicherzugriffe.

Der native globale Scratch wird hier **nicht** automatisch mit Referenzwerten
initialisiert. Er ist ein expliziter, persistent übergebener Puffer; auch
unberührte Knochenwerte werden bei erfolgreicher Pose-Übernahme kopiert.
`PreparedChannelPose` verlangt vorbereitete lokale Puffer und genügend Scratch.
Die native Größen-/Allokationsvorbereitung bleibt außerhalb dieser Stage.
Cache-Abschlussflags werden hier nicht gesetzt: Sie liegen im Original erst
hinter der nachfolgenden Matrix-/Director-/Bounds-Verarbeitung.

## Originaldaten über aufeinanderfolgende Schritte

`rc-original-channel-stack-check` prüft 225 aufgelöste Original-Linkups mit jeweils
vier vom Diagnoseaufrufer vorgegebenen Schritten und drei Kanälen:

1. Grundkanal auf dem ganzen Skelett, erste Originalsequenz, Gewicht 1.
2. Teilkanal ab Knochen 1, zweite Originalsequenz soweit vorhanden, Gewicht .5;
   Blendwerte .25/.5/.75/1 über die vier Schritte.
3. Vollständiger Ersatzkanal, erste Sequenz, Gewicht 0 in den ersten drei und 1
   im vierten Schritt. Dadurch verdrängt er im vierten Schritt beide Vorgänger.

Normalisierte Frames sind 0/.25/.5/.75. Instanzpose und Scratch beginnen je
Linkup bei der Referenzpose und bleiben danach über die Schritte erhalten.
Es entstehen **900 Posen mit 29.912 Matrizen**, **1.575 Kanalaufrufe**, 51.583
zugeordnete Knochenverarbeitungen und 88 Referenzfallbacks. Winkelbegrenzung
greift 12.965-mal, Positionsbegrenzung 20.922-mal.

`Record-OriginalChannelStacks.py` berechnet aktive Kanalfolge, Original-Track-
Posen, Übergänge und Scratch-Übernahme unabhängig. Alle Vorposen, Scratchwerte,
Kanal-Historywörter und Matrixwerte werden verglichen. Im aktuellen Korpus
stimmen alle portablen lokalen Ergebnisse bitgenau überein. Die Matrizen werden
über den bereits belegten Original-SSE-Ausdrucksinterpreter unabhängig geprüft.
Alle Komponenten sind endlich. Das bestätigt die portable Policy und die
untersuchte Stage, keine Ausführungsgleichheit mit der vollständigen Originalengine.

Fünf gezielte Tests prüfen unveränderte Vorpose/gemeinsamen Scratch, Verdrängung
auch bei fehlender Sequenz, Cache-/Disablepfade, Teiländerungen bei Fehlern und
Scratch-Übernahme bei leerem erfolgreichen Kanal. Insgesamt **276 Tests**,
Clippy mit `-D warnings` und Formatprüfung bestanden.

Offen: natürliche Tick-/Sequenzauswahl, automatische Cacheinvalidierung und
Abschlussflags, native Scratchallokation, x87-Präzision, Directors/Bounds,
Skinning und vollständige Spiel-/Renderintegration. Androidprüfung am Schluss.


Fortsetzung 2026-10-08: [Verbundene vorbereitete Vollpose](SKELETAL_FULL_POSE.md) verbindet jetzt Kanal-Auswertung, Director-Hierarchie, Bounds und das gemeinsame Cachebyte. Die vorgelagerte Puffer-/Transform-/Inverse-/Linkup-Vorbereitung bleibt vorausgesetzt; 347 Workspace-Tests bestanden.
