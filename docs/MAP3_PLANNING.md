# GEO_01C: Planungsraum und Strategieraumglas

Lieferung **0.38.0 / Code42**, `analysis/mission21-map3-planning/delivery`.
Die passende APK heißt `republic-commando-0.38.0.apk`, die Szene `geo_01c.rcplay`.
APK installieren und die neue Szene über „Szene öffnen“ laden. Der gespeicherte
B→C-Übergang übernimmt Spielerressourcen, Scorch und Fixer. Danach laufen der
Abseil-Einstieg und Sevs Beitritt. Auf die große Scheibe zum Strategieraum schießen,
anschließend die fünf Gegner bekämpfen. Neustart setzt auch Glas und Gegner zurück.

## Integrierter Abschnitt

Das originale Ereignis `SpawnPlanningRoom` aktiviert einmalig die Fabriken164/165
(zwei BattleDroids mit je75HP) und190/191/192 (drei Warriors mit je100HP).
Die Modelle und Atem-/Laufclips stammen aus den Originalpaketen. Kollisionsmaße
35/80 beziehungsweise50/115 stammen aus den jeweiligen Prototypen. Der begrenzte
AABB-Host benötigt bei Fabrik164 eine106Einheiten höhere Aufstellung und bei165
eine5Einheiten höhere Aufstellung, um den statischen Import zu räumen. Die
ursprünglichen Positionen, Anpassungen und Maße stehen in `build-receipt.json`.
Das ist keine Rekonstruktion des nativen zylindrischen Spawn-Solvers.

Warriors nähern sich bei Sichtkontakt zum Nahkampf, Droiden greifen aus der
Distanz an. Bewegung wird gegen den statischen Import geprüft. Die anwesenden
Squad-Mitglieder unterstützen bei freier Schusslinie. Diese Kampfentscheidung,
Angriffsintervalle, Schadenswerte und gemeinsame Unterstützungstaktung sind
begrenzte Portregeln. Native Goal-Ausführung, Fernkampfprojektile, Melee-Notifies,
Waffenanbauten der Gegner und Ragdolls sind hier noch nicht vollständig angebunden.

Die originale Scheibe263 `WindowShatter_Test1` hat150HP. Ihre252Dreiecke werden
als dynamische Darstellung aus dem statischen Szenenmesh herausgenommen. Der
Treffer wird gegen ihre ursprüngliche Kollisionshülle geprüft; eine nähere Wand
oder ein näherer Gegner hat Vorrang. Fünf Treffer mit dem vorhandenen30Schaden-
Hitscan zerstören sie. Darstellung und Kollisionshülle werden dann entfernt.
Dieser Zustand wird beim Neustart wiederhergestellt, auch nach einer Niederlage.

Der dekodierte originale `DestroyedState` nennt `WindowDestroyed`, das Effektobjekt
`CTEffects.WindowShattering_doubleSized`, kein Folgemesh und keinen direkten
TransitionSound. Das originale Feld `bDisableCollision` ist **false**; die
explizite Entfernung der alten Hülle ist eine Portregel für den Mesh-None-Zustand,
keine bestätigte native Prop-Kollisionsparität. Splittereffekt, dessen Audio und
vollständige Prop-Schadensmodifikatoren fehlen noch.

`WindowDestroyed` setzt die originale Skriptfolge356 fort: Ziel2 wird abgeschlossen,
danach laufen die drei Originalaufnahmen1492–1494 mit Wartezeiten aus ihren WAV-Daten.
Die Soundbank enthält18Samples. Sun Fac bleibt in dieser Lieferung zunächst am
ursprünglichen Ort und unverwundbar; seine Fluchtbewegung ist noch nicht ausgeführt.
Die Dialogfolge kündigt diese Flucht bereits an. Ziel0 wird nicht abgeschlossen.

## Prüfung

`playthrough/validation.json` enthält3.357normale C-Eingaben ab dem tatsächlichen
B-Abschluss. Kein Versetzen des laufenden Spielers, kein Auffüllen von Ressourcen
und kein Setzen von Missionsereignissen im Abnahmelauf. Ergebnis: Ziele1/2, fünf
besiegte Gegner,19Spielerschüsse, sieben Squad-Unterstützungstreffer,10HP und1/141
Munition. Der Lauf erzeugt57Audioereignisse. Vollständiger Zustand und Grafikframe
sind nach Neustart identisch; der gespeicherte Eingabelauf reproduziert den Zustand.

Der gesamte Einstieg bleibt nach25Sekunden bei20/60/144Eingabeframes proSekunde
identisch. `occlusion.json` prüft einen normalen Schuss auf die Scheibe hinter
einem tatsächlichen Welt-Hindernis: Munitionsverbrauch, aber kein Glas-/Gegnerschaden.
Bis zur Glaszerstörung erleidet keiner der fünf Gegner Schussschaden durch die
Scheibe. `failure-path.json` prüft den ungeschützten Vorstoß mit normalen Eingaben:
Die Gegner verursachen Schaden und die Niederlage ist erreichbar; Neustart stellt
den ursprünglichen Zustand wieder her.

Windows- und Android-Native-Prüfung spielen davor18.730A- und16.414B-Eingaben durch,
wechseln tatsächlich nachC und spielen den neuen Abschnitt über die Produktions-
Schnittstellen. APK-Prüfung: gespeicherter tatsächlicher Android-B→C-Übergang,
Einstieg bis Sevs Beitritt, GPU, Touch-Bewegung/Schießen/Ducken, Waffenton und Pause.
Das ist kein vollständiger manueller Touch-Durchlauf und kein physischer ARM64-Test.
Die Java-Szenenleser werden mit bekannter/unbekannter Länge unter96MiBHeap geprüft.
Frühere Lieferungen bleiben unverändert; Details stehen im Liefermanifest.

## Weiterarbeit

Sun Facs Originalskript193 wartet auf `WindowDestroyed`, führt ihn zu
`SunFacThroughDoor` (PathNode198), verriegelt die GuardRoomDoor139 und aktiviert
nach drei Sekunden `ActivateSBDIntro`. Danach folgt der ausdrücklich originale
Teleport nach `SunFacInsideShield` (PathNode32). Die beiden Türflügel134/135 sind
mit ihren originalen KeyPos-Werten bereits im Inventar aufgelöst. Diese Bewegung,
Türzustände, der aktivierbare Näheauslöser144 und der SBD-Auftritt sind der nächste
zusammenhängende Abschnitt. Anschließend folgen Türbrech-/Scharfschützenmanöver
und der Aufzug bis zum ursprünglichen Ausgang nachGEO_03A.
