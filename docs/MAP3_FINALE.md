# GEO_01C: Sevs Beschuss, Sun Facs Jäger und Absturz

Lieferung **0.43.0 / Code47**: `analysis/mission26-map3-finale/delivery`.
Die neue APK `republic-commando-0.43.0.apk` installieren und die passende
`geo_01c.rcplay` über „Szene öffnen“ laden. Der gespeicherte B→C-Übergang
bleibt verwendbar. Frühere APKs verstehen die neue kompakte Audiodarstellung
noch nicht; APK und Szene dieser Lieferung zusammen verwenden.

## Spielen

Den bisherigen Weg durch Planungs- und Wachraum samt Bactastationen und
Türbrechmanöver spielen. Die acht Droiden im AA-Raum bekämpfen und über die
Rampen auf den Balkon gehen. Sobald Sun Fac das Schild aktiviert hat und die
Dialoge die Position freigeben, die Kiste anvisieren und **BENUTZEN** drücken.
Sev muss seine Position tatsächlich erreichen. Er trägt dort jetzt das
ursprüngliche DC17m-Scharfschützenmodell und nimmt die gedeckte Haltung ein.

Sun Fac läuft zum Jäger und steigt nach seinen beiden Originalgesten ein.
Den Jägerstart beobachten und Sev decken. Sev spielt seine Schussanimation
und den Originalwaffenton. Bei besetzter Position folgt die Absturzsequenz,
das Wrack erscheint, die Abschlussdialoge beginnen und der Tracker zeigt
auf die Aufzugstür. Ohne rechtzeitigen Befehl erinnern die Originaldialoge
an die Zuweisung; danach kann der Jäger entkommen und die Mission scheitert.

Der erfolgreiche Stand endet am **Aufzugstracker**. Die Aufzugsinteraktion,
Originalziel6 und der abschließende Übergang nachGEO_03A sind noch offen.
Die gesamte erste Mission ist daher weiterhin nicht vollständig abgeschlossen.

## Originalablauf und begrenzte Portregeln

Sun Facs Aktionen1023–1029 sind angeschlossen: Lauf zu133, Warten auf
SunFacRunToShip, Stunned171Frames/30FPS, EmoteHearEnemy40/30, Gehen zu512,
TakeOff und Entfernen des Sun-Fac-Darstellers. Die Gehgeschwindigkeit180 ist
hier eine Portregel; eine vollständige native WalkToPoint-Ausführung wird
nicht behauptet. Die Originalkörpermaße50/50/115 und Laufgeschwindigkeit450
bleiben erhalten. Unterstützte Wege241/38Punkte verwenden tatsächliche
Boden-/Rampen-/Wandprüfungen. Im Referenzlauf werden keine Livepositionen
oder Ressourcen zum Überspringen dieser Wege gesetzt.

Stunned besitzt im Original0.2Animationsjitter. Dessen vorbereitete native
LOD-Jitterrechnung ist jetzt mit einem expliziten, im Animationszustand
speicherbaren lokalen CRT-Zufallsstrom verbunden (Seed1,214013/2531011).
Neustart und Bildraten reproduzieren ihn. Der globale Zufallsstrom des
Originalspiels und vollständige Jitter-Pose-/Kapselparität sind nicht
rekonstruiert. Andere bisherige Animationskanäle bleiben ohne Zufallsstrom,
wenn ihre Originalsequenz keinen Jitter benötigt.

Die ausgewählte Erinnerungskette415 prüft nach den ursprünglichen
Zwei-Sekunden-Pausen InSnipePosition. Sonst werden nacheinander die vier
Originalaufnahmen1627/1632/1637/1642 mit ihren tatsächlichen WAV-Dauern
abgespielt. Der Endabschnitt sendet StartSunFacRunning,SunFacRunToShip und
SpawnClimbOverWall. Aus416 folgen CA1G1592, Abschluss des Scharfschützenziels5
nach der Aufnahme und beim Start D40G1530. Der Auftritt141 erzeugt einmalig
einen GeonosianWarrior über Original-Spawnpunkt488 an einem unterstützten
Platz. Originales Klettern, Spawnpunktauswahl, Wiederholungs-/Patrouillen-
verhalten und Angriffe auf Sev sind noch nicht vollständig portiert; der
bestehende begrenzte Gegnerhost bleibt maßgeblich.

GeoFighter320 nutzt den Originalmesh Vehicles.GeoFighter und den Satz
cGEO01GeoFighterSet mit vier Clips: SevKillSunFacIntro247/30,
SevKillSunFacIdle54/30, SevKillSunFacDeath124/30 und
SevKillSunFacFailMission71/30. Die nativen Kanal-/Frame-/AnimEnd-/Root-Matrix-
Rechnungen steuern die Darstellung. Die Rootkurven der Clips setzen die
Positionen der vorherigen Clips fort und bleiben am Originalanker320.
Flugfiguren sind hier geskriptete, unverwundbare Darsteller; native Flug-
physik, Kollision, Karma und sämtliche Partikel sind nicht umgesetzt.

Nach Start und Warteclip sendet319 SnipeSunFac und wählt den Ausgang anhand
von Sevs tatsächlicher besetzter Position. Der Erfolg ist entsprechend dem
Originalskript eine Inszenierungsentscheidung und hängt hier nicht von
Projektilschaden am unverwundbaren Jäger ab. Erfolg spielt Death und die
ursprüngliche Trigger-Notify130:KillMe. Erst diese Notify verbirgt den Jäger,
zeigt Prop382/VehicleMeshes.Confederacy.GeoFighterBroken und sendet
SunFacTerminated. Die neun Originalnotifies des Erfolgswegs werden erfasst;
Effekt-/Rauch-/Explosionsnotifies sind noch nicht vollständig angebunden.

Sev verwendet den Originalclip FireSniper16/30, den StaticMesh
Weapons.CloneWeapons.DC17mSniper am Waffenbone und die Aufnahme
GEN_Sound.Weapons.wep_GEN_sniperFire_01. Die Originalaktion98/ShootTarget
liefert den Fünf-Sekunden-Zeitraum abTakeOff; der Host spielt darin sieben
Schüsse mit0.8sPort-Taktung. Native Zielauswahl, Projektil-/Ballistik-/Schadens-
logik, vollständige Marker-/Abbruch-/Inkapazitätsregeln und präzise native
Feuerkadenz bleiben offen. Das Zielfernrohr verwendet seine ursprüngliche
SelfIllumination-/Opacity-Textur, da der Shader keine Diffuse-Textur besitzt.
Die Portdarstellung verwendet CrouchBreathe; die Kollisionshülle bleibt die
bisherige stehende Begleiterhülle.

Die ausgewählten Originalaktionen287 warten aufSunFacTerminated, senden
ElevatorTrigger/ConditionSunFacDead, deaktivieren die Scharfschützenbindung
nach0.2s und spielen die anschließenden Originaldialoge. Danach folgen
ContinueToElevator/TrackElevatorDoor und die Umauftragungsdialoge. Der
Referenzlauf endet beim Tracker; später begonnene Dialoge dürfen weiterlaufen.
Die GrievousFlyAway-/Musik-/Effektketten sind weiterhin unvollständig.
Originalaktion1297 wiederholt ObjectiveNum1. Zusätzlich schließt der Port das
inhaltliche Gesamtziel0 „Eliminate Sun Fac“ beim erfolgreichen
SunFacTerminated ab und hält Ziel6 als nächstes aktives Ziel. Diese explizite
Zielzuordnung ist keine behauptete Parität sämtlicher nativer Objective-
Manager-Seiteneffekte. Die ausgesendeten Aufzugsereignisse führen dessen
Bewegung/Interaktion in diesem Stand noch nicht aus.

Beim Fehlschlag läuft der ursprüngliche FailMission-Clip, dann MissionFailed
und die beiden OriginalaufnahmenCA1Y1535/1536. Anschließend endet der Host
mit0Spieler-HP/PhaseLost. Die vollständige native289-Inkapazitäts-/HurtActor-
Ausführung für alle vier Squadmitglieder wird nicht behauptet.

## Dateien, Speicher und Nachweise

Die Szene bleibt RCPLAY14 und enthält47Originalsamples. Sound.wave wird beim
neuen Export als verlustfreier Base64-String statt JSON-Zahlenliste gespeichert;
der Decoder akzeptiert weiterhin beide Formen mit32MiB-Grenze proWAV.
Die eigentlichen WAV-Dateien und das native RCSND001-Audioformat ändern sich
nicht. Die ersten28Samples werden bytegenau mit der bisherigen Lieferung
verglichen. Daten-/Paddingfehler und alte Arrays sind gezielt getestet.
Die32MiB-Grenze für die gesamten Metadaten bleibt bestehen. Neue Jäger-,
Wrack- und Waffentexturen werden auf höchstens128×128Pixel vorbereitet;
bisherige Kartentexturen bleiben im statischen Szenenexport unverändert.

Der erfolgreiche Referenzlauf verwendet10841normale C-Eingaben nach echter
B-Übergabe: Ziele0/1/2/3/4/5,80HP,9/84Munition,68Spielerschüsse,20Abschüsse,
siebenSev-Scharfschützenschüsse und166Audioereignisse. Drei Gegner verbleiben:
zwei frühere optionale Gegner und der begrenzt aufgestellte Warrior.
Der gesonderte10526Eingaben-Lauf gibt Sev keine Position: vier Erinnerungen,
Entkommen,0Sev-Schüsse, PhaseLost und kein abgeschlossenes Sun-Fac-Gesamtziel.
Beide normalen Eingabereplays werden exakt wiederholt. Neustart stellt Zustand
und Grafik wieder her.35Sekunden ab dem tatsächlichen TakeOff-Präfix sind
bei20/60/144FPS vollständig zustandsgleich.

Windows-/Android-Produktionsbibliotheken spielen zuerst18730A- und16414B-
Eingaben samt echten Übergängen, dann den erfolgreichen C-Lauf und nach
Neustart den C-Fehlschlag. Der zusätzliche APK-Test prüft die tatsächlich unter
Android erzeugte gespeicherte B→C-Übergabe, Einstieg, GPU, Touch, Audio und
Pause. Java-Dateileser mit bekannter/unbekannter Länge bestehen unter96MiBHeap.
1119Bibliothekstests, Clippy, Format, SHA256-Liefersicherung und frühere
unveränderte Belege sind dokumentiert. Ein vollständiger manueller Touch-
Durchlauf oder ein physischer ARM64-Test ist damit nicht ersetzt.

## Nächster Meilenstein

Aufzugstür, Zugang, ursprüngliche Auslöser und Ausgang39 nachGEO_03A verbinden.
Originalziel6 erst durch die tatsächliche Aufzugsbenutzung abschließen und
dann den vollständigen A/B/C-Missionsabschluss samt Zustandsübergabe prüfen.
Die sichtbaren Grenzen bei Klettern/Gegner-/Squad-KI, Partikeln, Musik,
Animationen, Markern und früheren Bacta-/Tür-/Granateneffekten bleiben bestehen.
