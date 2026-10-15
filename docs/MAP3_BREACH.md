# GEO_01C: Bacta, Wachraum und Türbrechmanöver

Lieferung **0.41.0 / Code45**: `analysis/mission24-map3-breach/delivery`.
APK `republic-commando-0.41.0.apk` installieren und die passende `geo_01c.rcplay`
über „Szene öffnen“ laden. Der gespeicherte B→C-Übergang bleibt verwendbar.
Die bisherigen Karten A/B bleiben in ihren früheren Lieferungen unverändert.

## Spielen

Nach dem SBD-Auftritt und Originalziel3 lassen sich die ursprünglichen
Bactastationen im Eingangsbereich und im Wachraum benutzen. In der Nähe
**BENUTZEN halten**, bis die Gesundheit aufgefüllt ist. Der Hinweis erscheint
im HUD. Während der Heilung stehen Bewegung und Waffenbedienung still;
Gegner können weiter angreifen. Die Heilung gibt keine Munition.

Durch den Wachraum zu Sun Facs Tür gehen und Gegner bekämpfen. An der Tür
**BENUTZEN** drücken. Scorch, Fixer und Sev laufen zu ihren drei Positionen
und führen gemeinsam das Türbrechmanöver aus. Die Tür öffnet sich beim
originalen Schalter-Notify; Originalziel4 wird abgeschlossen. Dieser Stand
endet nach der Türöffnung. Die weitere Sun-Fac-Verfolgung wird als Nächstes
angebunden und ist noch kein vollständiger Missionsabschluss.

## Originalbindungen und begrenzte Portregeln

Bacta310/261 stammen aus `MarkerProperties.BactaDispenserGEO`, einschließlich
Originalmodell, Holster-/Load-Clips, Positionen und Drehungen. Originaldefaults:
PlayerRange256, bLockPlayerLocation=true und BactaRate4.0. Die native Anwendung
von BactaRate ist noch nicht rekonstruiert: Der Host interpretiert4.0 als vier
Sekunden für100HP, also25HP/s. Freie Sicht und lebender Spieler sind nötig;
bei100HP endet die Heilung. Das Modell zeigt jeweils Frame0 des gewählten
Clips. Partikel, Körper-Blitze, vollständige Aktivierungsanimation und Bacta-
Audio fehlen. Die beiden Originaltexturen werden für das Szenenbudget auf
höchstens128×128Pixel verkleinert; es werden keine Ersatztexturen erzeugt.

SquadMarker122/Controller125 verwenden die Originalpositionen183/184/185.
Die Rollenverteilung Scorch/Türmann, Fixer/Granatenwerfer, Sev/Sicherung ist
Portlogik. Die drei Originalclips `DoorBreachLoudSM1Doorman`,
`DoorBreachLoudSM2Grenademan`, `DoorBreachLoudSM3Coverman` laufen mit den
nativen Kanal-/AnimEnd-/Notify-Rechnungen:277/248/275Frames bei30FPS.
Die vollständigen Notifyzahlen sind5/3/0. MDBDestroySwitch309 öffnet die
Tür und löst LandingPadDoorBreached aus; MDBtossGrenade306 wird protokolliert.
Die Aufnahmen zum Ladungssetzen312, zur Bewegung307 und die ursprüngliche
DoorDamagedState-Explosion werden abgespielt. Speech-SoundCues308/310/311/305,
Ladungs-/Granatenobjekte, tatsächlicher Granatenwurf/-schaden, Explosionseffekt,
DoorDamagedState-Flächenschaden200/Radius256 und beschädigter Schlossmesh fehlen.
Die sichtbaren Marker-/Schlossdarstellungen sind noch nicht vollständig an den
originalen Aktivierungs-/Beschädigungszustand gekoppelt.

Türflügel120/121 verschieben Geometrie und Kollisionshüllen um die Originalwerte
[0,192,0]/[0,-192,0] in0.5Sekunden. SunFacRoomDoorOpening/Opened werden getrennt
verarbeitet. Originalskript140 schließt Ziel4 ab und emittiert ContinueToAARoom,
SpawnAARoom und den Tracker. Die AA-Raum-Fabriken und Sun Facs Folgehandlung
sind noch nicht ausgeführt. Ein emittiertes Ereignis allein bedeutet daher
keine vollständige Umsetzung des Folgeabschnitts.

Die Laufwege verwenden120 ursprüngliche Navigationspositionen mit1127
geometrisch ermittelten, physisch geprüften Verbindungen. Das ist ein eigener
Dijkstra-/Bodenkontakt-Adapter, keine native ReachSpec-/CTBot-/Squad-Parität.
Nur die getrennte Planungswelt behandelt Glas und Türen als offen. Jeder
Spieler-/Squad-Schritt prüft die tatsächliche laufende Welt. Die Rootbewegung
während des Manövers wird mit Boden-, Rampen-, Stufen- und Wandkontakt ausgeführt;
es gibt keine Positionssprünge zu den Formationspunkten. Kollisionsbedingte
Abweichungen von den ursprünglichen Rootkurven werden protokolliert.

## Nachweise

Der Referenzlauf setzt die tatsächliche B-Übergabe fort und verwendet7081 normale
C-Eingaben. Er umfasst Einstieg, Planungsraum, Sun Facs Flucht, SBD-Auftritt,
beide Heilungen, Wachraumkämpfe, Formation und Türöffnung. Ergebnis: Ziele1/2/3/4,
95HP,12/103Munition,46Spielerschüsse,12Abschüsse, zwei verbleibende Gegner und
120Audioereignisse bei26Originalsamples. Keine laufenden Positionen, Ressourcen
oder Missionsereignisse werden für diesen Durchlauf gesetzt.

`playthrough/validation.json` prüft exakten Neustart von Zustand und Grafik,
identische Wiederholung des gespeicherten Eingabelaufs, Heilung nur mit
BENUTZEN, keine Bewegung/Schüsse während der Heilung und die100HP-Grenze.
Der vollständige Zustand nach20Sekunden ab tatsächlichem Türbefehl ist bei
20/60/144FPS identisch. Die Produktionsbibliotheken spielen davor18730A- und
16414B-Eingaben samt tatsächlichen Kartenübergängen. Java-Dateileser werden mit
bekannter/unbekannter Dateigröße unter96MiBHeap geprüft. Android-Native prüft
C bis zur Tür; der gesonderte APK-Test prüft Wiederaufnahme, Einstieg, GPU,
Touch, Waffenton und Pause. Das ersetzt keinen vollständigen manuellen Touch-
Durchlauf und keine physische ARM64-Abnahme. Details stehen in den jeweiligen
Validierungsdateien und im Liefermanifest.

## Nächster Meilenstein

Sun Facs anschließende Bewegung nach SunFacRoomDoorOpened, die AA-Raum-Fabriken,
Scharfschützenmanöver und Ziele5/6 umsetzen. Danach Originalziel0, Missionsende
und Aufzug/Übergang nachGEO_03A verbinden und den vollständigen A/B/C-Lauf
abnehmen. Kampf-, Effekt-, Marker- und Animationsparität bleiben sichtbar
getrennte Arbeiten. Die erste Mission ist weiterhin nicht abgeschlossen.
