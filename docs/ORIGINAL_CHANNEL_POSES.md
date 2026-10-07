# Originaltracks mit Kanalblending

`TrackChannelHost` verbindet `ApplyAnimChannel` mit geladenen `AnimationTrack`s,
dem vorhandenen `GetRotPos`-Sampler und Quaternion-Decoding/Slerp. Die
Quaternion-Math-Policy bleibt über alle Aufrufe erhalten; Trackfehler werden
weitergegeben. Ein fehlerhafter lokaler Track verändert den Scratch-Knochen
nicht, zuvor fertig geschriebene Knochen bleiben bestehen.

`ChannelAngularMath` kapselt die Winkelbegrenzung. Die mitgelieferte
`PortableChannelAngularMath` benutzt f64 für AngleDiffFast: Produktsumme in
Reihenfolge w,z,y,x, Betrag, obere Begrenzung auf 1, 2*acos. Der erste Winkel
wird nach f32 gerundet. Für den Vergleich wird der zweite Winkel plus
ersterWinkel*Fortschritt in f64 berechnet; erst für den Quotienten wird die Summe
nach f32 gerundet. Das entspricht der belegten Speicher-/Vergleichsstruktur,
emuliert aber keine x87-Registerpräzision, Kontrollwörter oder ursprüngliche
acos-Implementierung. Auch RSQRTSS bleibt die bekannte portable Näherung.

## Diagnose mit Originaldaten

`rc-original-channel-check` lädt die erste Originalsequenz jedes der 225 zuvor
aufgelösten Mesh-Linkups erneut aus dem jeweiligen `.ukx`-Paket. Pro Linkup
werden vier unabhängige Szenarien auf dem gesamten Knochenintervall gerechnet:

| Szenario | Kanalindex | Blend | Gewicht |
| --- | ---: | ---: | ---: |
| Replace | 0 | 1 | .5, durch Index 0 effektiv 1 |
| Layer | 1 | 1 | .5 |
| Transition | 0 | .25 | .5, durch Index 0 effektiv 1 |
| LayerTransition | 1 | .25 | .5 |

Aktueller normalisierter Frame ist .75, vorheriger .25, vorheriger Blend 0.
Referenzpose, vorherige Instanzpose und anfänglicher Scratch werden aus denselben
Original-Referenzknochen vorbereitet. Das sind bewusst kontrollierte Snapshots;
es wird kein natürlicher Spielablauf, kein vollständiger Kanalstapel und keine
Animationszeitsteuerung behauptet.

Ergebnis: **900 Kanalposen, 29.912 Knochenmatrizen** auf 130 Meshes. 29.860
zugeordnete Knochen werden verarbeitet, 52 verwenden Referenzfallback. In diesen
Szenarien greifen 9.372 Winkel- und 13.916 Positionsbegrenzungen.

`Record-OriginalChannelPoses.py` liest die Original-Trackbytes unabhängig,
berechnet Decoding, Trackzeit, Slerp und Kanalübergänge mit expliziten
f32-Rundungen nach und vergleicht alle lokalen Posen und Historywörter.
Akzeptanzgrenzen sind 2e-6 je Quaternionkomponente und 2e-5 Positionsfehler
geteilt durch max(1, abs(erwarteter Wert)). Im aktuellen Korpus sind beide
maximalen Fehler **0**. Das bestätigt die implementierte portable Policy,
keine Bitgleichheit mit dem laufenden Originalspiel.

Alle Matrizen werden zusätzlich anhand der bereits unabhängig interpretierten
Original-SSE-Ausdrücke bitgenau geprüft. Alle Komponenten sind endlich.
Originalpaket-, Quell- und Reporthashes stehen in
`analysis/reports/original-channel-poses-validation.json`.

Zwei neue Tests prüfen die echte Track-Anbindung samt fehlenden Keys sowie die
Winkelbegrenzung mit Vorzeichenwechsel, übergroßem Dotprodukt, identischen
Rotationen und NaN-Fortschritt. Insgesamt 271 Workspace-Tests bestanden;
Clippy mit `-D warnings` und Formatprüfung ebenfalls.

Die aktive Kanalfolge und gemeinsame Pose-Übernahme sind anschließend in
[SKELETAL_CHANNEL_STACK.md](SKELETAL_CHANNEL_STACK.md) ergänzt und mit
aufeinanderfolgenden Originaltrack-Schritten geprüft.

Offen bleiben natürliche Vorpose-/Scratch-Vorbereitung,
Animations-Ticking, Directors/Bounds, Skinning und der vollständige Renderpfad.
Die Androidprüfung folgt erst am Projektende.
