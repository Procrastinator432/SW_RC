# Weltgrenzen der ausgewerteten Pose

Original: ApplyAnimation **1050af1e..1050b276**. Rust:
`skeletal_world_bounds.rs`. Eingaben sind lokale Box/Kugel und die bereits
aufgelöste Matrix am Actor-Primitiv +8; dessen Existenz entspricht Actor+48!=null.

Alle acht Boxecken werden in nativer X/Y/Z-Schleifenreihenfolge transformiert.
Die Addition entspricht dem Original-SSE-Code, einschließlich abweichender
Reihenfolgen je Achse. Es gibt keine projektive Division. Die FBox-Helfer sind
zusätzlich aus core.dll exportiert: Konstruktor 1011f9e0, Punktaddition
1011fa30, GetExtrema 10115e60. Erstes Einfügen setzt min=max=Punkt und valid=1.
Spätere ungeordnete Vergleiche ersetzen den bisherigen Endpunkt durch den neuen
Punkt; Gleichheit behält den bisherigen Wert samt Rohbits. Dies unterscheidet
sich von den lokalen Knochenpunkt-Bounds und von f32::min/max bei NaN.

Die Weltbox wird vor der Weltkugel veröffentlicht. Deren Zentrum verwendet
eigene native Summenreihenfolgen, die sich von den Boxeck-Reihenfolgen unterscheiden.
Der Radius wird mit der größten Matrixzeilenlänge skaliert. Dabei werden die
Zeilenquadrat-Summen in Reihenfolge Zeile2, Zeile1, Zeile0 per **unsigned
Float-Rohbits** verglichen, mit Ersetzung bei Gleichheit. Das ist die belegte
Engine-Regel, keine Berechnung der größten Singulärzahl bei Scherung.
RSQRTSS/Newtonkorrektur und Nullmaskierung entsprechen der lokalen Bounds-Stage.

`WorldPoseBoundsHost` verbindet diese Veröffentlichung mit
`finish_prepared_bounds`. Bei fehlendem Actor-Primitiv bleibt sie ein No-op.
Mit Actor wird zuerst die Weltbox, dann die Weltkugel geschrieben; erst nach
Erfolg setzt der lokale Abschluss die Bytes +60/+61=1 und +179=0.
Ein Seedfehler erhält die neue Weltbox und alte Weltkugel. Die vorhandene
lokale Fehlergrenze verhindert dann vorzeitiges Setzen der Abschlussflags.
Native Objekt-/Pointerbindung und Aktualisierung der Actor-Matrix bleiben
außerhalb dieses Snapshot-Adapters.

## Nachweise

`rc-world-bounds-check` transformiert die 900 zuvor geprüften lokalen
Originaltrack-Diagnosebounds mit vier explizit vorgegebenen Matrizen: Identität,
Rotation/Spiegelung/ungleiche Skalierung mit Translation, Scherung und
Nullskalierung mit Translation. Das sind **3.600 Weltbounds-Fälle**. Die lokalen
Paddingwerte und Transformationsmatrizen sind Diagnoseeingaben, keine geladenen
Original-Runtime-Actorwerte.

64 allgemeine endliche Matrixfälle benutzen einen festen Seed .125; 16 davon
erzeugen einen Seedfehler. `Record-WorldPoseBounds.py` interpretiert die
Originalinstruktionen für alle Boxeck- und Kugelzentrum-Ausdrücke direkt und
vergleicht alle Bounds-/Kugelergebnisse bitgenau unter der jeweiligen
Math-Policy. Es prüft außerdem Zeilenauswahl und Veröffentlichungsreihenfolge.
Sechs Tests sichern Spiegelung/Skalierung, unterscheidbare Summenreihenfolgen,
Fehlergrenze, Nullskalierung, NaN/Ties in der Box und integrierten lokalen Abschluss.
**287 Tests**, Clippy und Formatprüfung bestanden.

Offen: natürliche Actor-/Matrixbindung, originale Padding-/kOne-Runtimewerte,
Directors, native RSQRTSS-Präzision und Skinning. Androidprüfung am Schluss.
