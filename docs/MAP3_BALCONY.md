# GEO_01C: AA-Raum, Schild und Sevs Scharfschützenposition

Lieferung **0.42.0 / Code46**: `analysis/mission25-map3-balcony/delivery`.
APK `republic-commando-0.42.0.apk` installieren und die passende `geo_01c.rcplay`
über „Szene öffnen“ laden. Der bestehende gespeicherte B→C-Übergang bleibt verwendbar.
A/B sowie frühere C-Lieferungen bleiben unverändert.

## Spielen

Nach beiden Bactastationen und dem Türbrechbefehl an Sun Facs Tür die Öffnung
decken und die Droiden im nächsten Raum bekämpfen. Sichtbare Gegner anvisieren
und mit BENUTZEN das Squad-Feuer konzentrieren. Danach über die Rampen auf den
Balkon vorgehen. Sun Fac reagiert auf das Squad, passiert den Schildbereich
und aktiviert das Schild. Die Originaldialoge fordern Sevs Zuweisung an.
Die markierte Kiste anvisieren und **BENUTZEN** drücken. Sev läuft zuerst
in Bereitschaft und nach dem Befehl zu seiner Position an der Kiste.
Der HUD-Text bestätigt die besetzte Scharfschützenposition.

Dieser Stand endet an dieser Vorbereitung. Sun Facs weiterer Lauf zum Jäger,
Start, Beschuss, Absturz und Missionsabschluss sind noch nicht ausgeführt.
Insbesondere werden die Originalziele5/6 und das Gesamtziel0 noch nicht
abgeschlossen. Die Scharfschützenwaffe und die eigentliche Schusssequenz sind
noch nicht angebunden; Sev verwendet weiterhin das vorhandene Waffenmodell
und die stehende Standarddarstellung. Es wird keine abgeschlossene erste
Mission oder vollständige Original-KI-/Squad-Parität behauptet.

## Originalbindungen und Portregeln

Alle acht ursprünglichen SpawnAARoom-Fabriken151/152/153/474/475/476/481/486
sind an das bereits vorhandene Ereignis aus Originalskript140 gebunden.
Jede hat Kapazität1 und erzeugt einen BattleDroid mit75HP und Maßen35/35/80.
Unterstützte Spawnpunkte werden begrenzt um den Originalpunkt gesucht;
Original-/Laufzeitpositionen stehen im Buildbeleg. Ursprüngliche PatrolRoutes,
StartStates und native Factory-/CTBot-Ausführung werden nicht vollständig
portiert. Die vorhandenen begrenzten Bewegungs-, Sicht-, Hitscan-, Fokus-
und Squad-Unterstützungsregeln gelten auch für diese Gegner.

Aus Sun Facs Originalskript193 sind Aktionen1016–1022 angeschlossen:
SunFacRoomDoorOpened,2sWartezeit,EmoteSeeEnemy,5sWartezeit, unterstützter Lauf
zu SunFacOutSideShield33 mit450Einheiten/s,ActivateShield undDeathTaunt.
Die ursprünglichen WarriorSet-Clips haben45/65Frames bei30FPS und insgesamt
achtNotifies. Kanal-, AnimEnd- und relativeRoot-Rechnungen verwenden die
nativen Funktionen; die Körperbewegung folgt dem begrenzten Bodenkontakt-
Host. Die Notifyzeitpunkte werden protokolliert, ihre vollständigen Audio-/FX-
Seiteneffekte sind noch nicht angebunden. Am Ziel übernimmt Sun Fac die
ursprüngliche Drehung33 und richtet seine Geste zum Squad.

Der vorherige native Teleport auf Punkt32 würde mit der verwendeten originalen
115Einheiten-Körperhöhe rund20Einheiten in den Boden führen. Der neue Export
projiziert dieses Teleportziel auf den unterstützten Boden (-739.886 statt
-759.872 inZ); X/Y bleiben original. Diese begrenzte Platzierungskorrektur ist
keine exakte native Teleport-/Kapsel-Parität. Anschließend wird körperlich
bewegt. Die frühere Teleportaktion selbst stammt aus dem Originalskript.

Schild412/Prop3 war bisher unsichtbar, aber ständig kollidierbar. Jetzt ist
seine vorhandene Originalhülle zu Beginn deaktiviert. ActivateShield schaltet
die Kollision sofort und die sichtbare Geometrie nach2s ein, entsprechend
Skript413. Die Darstellung nutzt den ursprünglichen GateShield-Mesh und die
in dessen HsFalloff-Material ausdrücklich hinterlegte FallbackMaterial-Textur
Arena_Textures.Shader.SpotRaysPCX. FalloffGradient,PanRate,Tint und die native
Blend-/Effektkette sind nicht vollständig gerendert; der Originalfallback wird
statisch verwendet. Es wird keine vollständige Schild-Shader-Parität behauptet.

Die ausgewählten Aktionen aus145/482 spielen die beiden Originalaufnahmen
D38G1513/CA1G1746, senden WalkToSnipeMarker, zeigen Kisten-/Markerhinweise und
aktivieren nach0.8+0.5s die Zuweisung. Es gibt keinen modalen Original-Hinweisdialog.
Sev läuft physisch zu Bereitschaftspunkt34 und nach sichtbarer Kisten-Zielwahl
innerhalb2500Einheiten zu Originalpunkt485. Erst bei seiner tatsächlichen
Ankunft entstehen InSnipePosition undSnipeStarted. Native Marker-Mitgliederwahl,
Abbruch-/Inkapazitätsregeln, Scharfschützenverhalten und die Folgeketten415/416
bleiben offen; die Ereignisse allein führen den Schiffskampf noch nicht aus.

Die Navigation umfasst147Originalpositionen und1336 geometrisch erzeugte
Verbindungen. Der getrennte Planer darf Türen/Glas offen behandeln; jeder
laufende Körper prüft die tatsächliche Welt. OriginalReachSpecs und deren
sämtliche Bewegungsarten sind nicht portiert. Die Bactatexturen werden im
neuen Export auf höchstens64×64 verkleinert. Unbenutzte Heil-/Sprengclips der
Sev-Kopie und unnötige Clips der nur im Einstieg sichtbaren Scorch-/Fixer-
Kopien werden ausgelassen. Alle in diesem Stand tatsächlich abgespielten
Clips bleiben enthalten. Das32MiB-Metadatenlimit bleibt bestehen.

## Nachweise

7.768normale C-Eingaben nach der tatsächlichen B-Übergabe erreichen den neuen
Endpunkt. Der Lauf übernimmt die ersten6480Eingaben des vorigen Ablaufs und
bekämpft die neuen Gegner bereits während der Türöffnung. Kein laufender
Spieler-/Squad-Zustand, keine Ressourcen und keine Missionsereignisse werden
als Abkürzung gesetzt. Ergebnis: Ziele1/2/3/4,80HP,9/84Munition,68Spielerschüsse,
20Abschüsse, zwei verbleibende frühere Gegner und150Audioereignisse aus28Samples.
Alle acht AA-Raum-Droiden sind im Referenzlauf besiegt.

Die Abnahme prüft die acht Bindungen, Sun Facs achtNotifies, Sevs tatsächliche
Ankunft, anfänglich freie Schildkollision, sofortige Aktivierung vor der
Sichtbarkeit, die120TicksVerzögerung und blockierte Strahlen durch das aktive
Schild. Zustand/Grafik werden bei Neustart exakt wiederhergestellt; das normale
Eingabereplay reproduziert den vollständigen Endzustand.15Sekunden nach dem
Zuweisungspräfix ergeben bei20/60/144FPS denselben vollständigen Zustand.

Windows-/Android-Produktionsbibliotheken spielen davor18730A- und16414B-
Eingaben mit tatsächlichen Kartenübergängen und danach den vollständigen
C-Lauf. Der APK-Test prüft die echte gespeicherte Android-B→C-Übergabe,
Einstieg, GPU, Touch-Bewegung/Schießen/Ducken, Waffenton und Pause.
Bekannte/unbekannte Dateigrößen werden unter96MiBJavaHeap geprüft.
Das ist kein vollständiger manueller Touch-Durchlauf und kein physischer
ARM64-Test. Details und SHA256-Belege stehen im Liefermanifest.

## Nächster Meilenstein

Sun Facs Aktionen1023–1029, SnipeReminder/StartSunFacRunning, Sev-Schusssequenz
und den Original-Jägerablauf bis TakeOff/Absturz mit Zielen5/6 verbinden.
Anschließend Gesamtziel0, Aufzug und Ausgang39 nachGEO_03A umsetzen und die
erste Mission zusammenhängend abnehmen. Bisherige Effekt-, Marker-, Kampf-,
Animations- und nativeKI-/Karma-Grenzen bleiben bestehen.
