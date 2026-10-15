# GEO_01C: Sun Facs Flucht durch die Wachtür

Lieferung **0.39.0 / Code43**, `analysis/mission22-map3-escape/delivery`:
`republic-commando-0.39.0.apk` und passende `geo_01c.rcplay`.
Die APK installieren und die neue Szene über „Szene öffnen“ laden. Der gespeicherte
B→C-Übergang übernimmt wie bisher die Ressourcen. Nach Abseilen/Sev und dem Kampf
im Planungsraum löst die zerstörte Scheibe jetzt Sun Facs sichtbare Flucht aus.
Anschließend zur Wachtür gehen; der große Tisch muss seitlich umgangen werden.

## Umgesetzte Originalfolge

Der bisherige Einstieg und die fünf Planungsraumgegner bleiben erhalten. Sun Fac
bleibt unverwundbar. Sein Originalskript193 wartet auf `WindowDestroyed`, läuft
mit Aktion1010 zum ursprünglichen `SunFacThroughDoor`/PathNode198 und verriegelt
mit1011 die Tür139. Aktion1012 wartet drei Sekunden,1013 aktiviert den
Näheauslöser144. Aktion1014 teleportiert Sun Fac ausdrücklich im Original zu
`SunFacInsideShield`/PathNode32;1015 richtet ihn dort aus. Der Host wartet danach
an der Grenze vor1016 auf `SunFacRoomDoorOpened`.

Der Laufweg ist mit Sun Facs Originalmaßen50/50/115 gegen die importierte Welt
geprüft. Originalgeschwindigkeit450 und originaler `MoveTimes[0]`-Wert0,5Sekunden
sind aufgelöst. Ein beim Bau geprüfter86Punkte-Gehweg verbindet die unterstützten
Start-/Zielpositionen; im Abnahmelauf tritt kein blockierter Schritt auf.
Die Aufstellung wird vor dem Laufen an den importierten Boden angepasst. Originale
und unterstützte Positionen, Navigation und Originalaktionen stehen im Buildbeleg.
Es handelt sich um einen begrenzten Gehcontroller mit gewöhnlicher Körperphysik,
keinen vollständigen Port der nativen AI-/MoveToPoint-/ReachSpec-Ausführung.

Sun Fac verwendet den ursprünglichen RunForward-Clip des Geonosian-Modells,
seine Bewegungsrichtung bestimmt die Ausrichtung. Die Darstellung schleift den
Clip zeitabhängig; native Lauf-Notifies/Schritt-Audio und RootMotion-Parität sind
noch nicht angebunden. Für den späteren Teleport werden Originalposition und
-rotation des Zielactors verwendet. Er ist kein Ersatz für den vorherigen Laufweg.

Die beiden Flügel134/135 werden aus dem statischen Mesh gelöst. Ihre originalen
KeyPos-Werte `[192,0,0]` und `[-176,0,0]` verschieben Darstellung, Hüllengrenzen und
Kollisionsflächen gemeinsam. Die geöffneten Flügel bleiben seitliche Hindernisse.
Die Tür öffnet sich bei Sun Facs Annäherung, schließt hinter ihm und bleibt
verriegelt. Der Schließhost wartet bei einer Überlappung mit Spieler oder Squad.
Die Annäherungsschwelle, lineare Interpolation und Überlappungssicherung sind
Portregeln; native Autodoor-/Encroachment-/Crush-, Sound- und Antiportal-Parität
wird nicht behauptet. Neustart stellt die geschlossene Originalgeometrie wieder her.

Trigger144 ist anfangs deaktiviert und wird durch `ActivateSBDIntro` freigegeben.
Sein ursprünglicher Radius1000, die aufgelöste Höhe und CloneCommando-Klasse
begrenzen den Überlappungshost. Spieler und beigetretene Squad-Mitglieder werden
berücksichtigt; horizontaler und vertikaler Abstand müssen passen. Das Ereignis
`StartSBDIntro` wird einmalig ausgelöst, wenn ein passender Körper den Bereich
erreicht. Der Host prüft auch nachträglich aktivierte Überlappungen. Dies ist keine
vollständige native Touch-/Event-Verteilung.

## Abnahme und Grenzen

`playthrough/validation.json` enthält3.558normale C-Eingaben nach einem tatsächlichen
B-Abschluss: bisheriger Einstieg, Glas und fünf Gegner, Sun Facs Weg, vollständige
Türöffnung/-schließung, Verriegelung,180Ticks Wartezeit, Originalteleport und
anschließender Vorstoß bis zum Näheauslöser. Es werden keine laufenden Positionen,
Ressourcen oder Ereignisse im Abnahmelauf gesetzt. Ergebnis weiterhin10HP,
1/141Munition,19Spielerschüsse, fünf besiegte Gegner und sieben Squadtreffer.
Die geschlossene Tür blockiert einen Körperstrahl durch den Durchgang. Neustart
stellt Zustand und Grafikframe exakt wieder her; der gespeicherte Eingabelauf
reproduziert den vollständigen Endzustand. Bei20/60/144Eingabeframes proSekunde
ist der gesamte Zustand nach dem Glas-Schuss-Präfix plus20Sekunden identisch.

Windows- und Android-Native-Prüfung spielen davor18.730A- und16.414B-Eingaben durch,
wechseln tatsächlich nachC und prüfen den neuen Lauf. Die APK-Prüfung öffnet C
mit der tatsächlich unterAndroid erzeugten gespeicherten Übergabe und prüft
Einstieg, GPU, Touch, Waffenton und Pause. Kein vollständiger manueller Touch-
Durchlauf und kein physischer ARM64-Test. Java-Dateileser werden mit bekannter
und unbekannter Länge unter96MiBHeap geprüft. Die Liefersicherung prüft frühere
Belege unverändert; Details stehen im Liefermanifest.

Die erste Gesamtmission bleibt unvollständig. **Der sichtbare Superkampfdroiden-
Auftritt und der Wachraumkampf sind noch nicht ausgeführt.** Der neue Abschnitt
endet an deren ursprünglichem Eingangssignal `StartSBDIntro`. Originalziel0
„Eliminate Sun Fac“ und Ziele3–6 bleiben offen. Die vorhandene Dialogfolge nach
der Glaszerstörung läuft weiterhin unabhängig von der Laufzeit der Flucht.

Als Nächstes Skript147 anbinden: drei Sekunden nach `StartSBDIntro` folgen
`SpawnGuardRoom`, `SBDIntro` und die Türentriegelung. Dazu gehören die originalen
Figuren321–323 und ihre IntroSBD-/RightPush-/LeftSmash-Clips, acht Wachraumfabriken,
SBD-Kampf, `SBDDestroyed` und Originalziel3. Danach folgen Türbrech-/Scharfschützen-
manöver und der Aufzug bis zum Ausgang nachGEO_03A.
