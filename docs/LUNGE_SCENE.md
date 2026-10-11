# Ausfallschritte M3g

Stand 2026-10-11. Neue Lieferung: Android **0.15.0 / Code17**, `analysis/milestone3g/republic-commando-0.15.0.apk` und `analysis/milestone3g/geo_01a-lunge.rcplay`. APK installieren und die passende Datei über **SZENE ÖFFNEN** laden. Drei Geonosianer ausschalten und die Exfil-Markierung erreichen. Die M3f-Lieferung bleibt erhalten.

## Spielbarer Ablauf

Die Geonosianer können nun `Lunge` und `Lunge2` ausführen. Der Körper übernimmt die Position des originalen komprimierten Move-Tracks, einschließlich des kurzen Anhebens. Der Original-Clip-Takt und die MeleeHit-Notifies bestimmen Bewegung und Trefferzeit. Eine gestartete Bewegung behält ihre Ausrichtung bei. Der Spieler kann während des Anlaufs ausweichen oder den Angriff durch einen Treffer unterbrechen.

Vor dem Start muss die ganze Bewegungskurve die statische Körperkollision und Bodenunterstützung bestehen. Während der Animation wird zusätzlich jeder tatsächlich ausgeführte Bewegungsschritt geprüft. Eine gesperrte Kurve wird nicht gewählt; ein später blockierter Schritt beendet den Angriff an der letzten sicheren Position. Tod und Trefferunterbrechung löschen die Bewegung und setzen den Körper auf eine geprüfte Bodenposition. Neustart setzt auch Bewegungszustand und Zufallsstrom zurück.

Bei der vorbereiteten Gegnerhöhe von 120 Einheiten bewegt `Lunge` den Körper um 286,80 Einheiten vorwärts und hebt ihn maximal ungefähr 62,55 Einheiten an. `Lunge2` endet bei etwa 294,29 Einheiten Vorwärtsbewegung mit einem sehr kleinen seitlichen Versatz. Diese Werte stammen aus der Originalkurve mit der bestehenden Szenenskalierung; es sind keine neuen konstanten Sprintgeschwindigkeiten.

## Originalbelege und Abgrenzung

`analysis/milestone3g/root-motion.c` exportiert die untersuchten engine.dll-Funktionen `AActor::GetAnimRootTranslation` 103ba690, `USkeletalMesh::GetAnimRootTranslation` 10501350, `AActor::physRootMotion` 1048f2a0 und `APawn::physRootMotion` 1048f3a0. Die native Endpunktabfrage liefert einen gecachten Sequenzvektor. Der StateGeonosianWarriorAttack-Pfad transformiert ihn mit der Pawn-Ausrichtung und fragt CanMoveTo ab; die vorhandene vorbereitete Manöverwahl übernimmt weiterhin Distanzgrenzen und den strikten Zufallsvergleich für Lunge.

Der neue Host berechnet die Bewegung direkt aus den bereits geladenen Original-Tracks über `skeletal_track::sample_track`. Er verwendet die portable Frame-Multiplikation und Quaternion-Mathematik der vorhandenen Animationsintegration. Er rekonstruiert weder die native Befüllung des Endpunktcaches noch die vollständige PHYS_RootMotion-/PHYS_Falling-/PlayAnim-Goal-Kette. Die untersuchten Lunge-Zweige setzen zunächst PHYS_Falling und reihen TurnTo/PlayAnim ein; deren vollständige Lebenszyklusbedingungen bleiben außerhalb dieses Hosts. Es gibt in diesem Abschnitt keinen neuen Originalinstruktions-Replay der vollständigen Root-Motion-Physik.

Die beiden ausgewählten Move-Tracks haben konstante Identitätsrotation. Der Loader verlangt diese Eigenschaft sowie den Move-Wurzelknochen und die hier verwendete unveränderte Mesh-Skalierung/-Rotation. Andere Root-Rotationen sind noch nicht unterstützt. Die Körpertranslation ist relativ zum ersten Root-Sample und wird mit der bestehenden Präsentationsskalierung gedreht. Die Skeletthierarchie übernimmt den Move-Knochen bereits separat; für diese Clips wird dessen lokale Translation zusätzlich auf die Referenzposition gesetzt, bevor die Pose gezeichnet wird. Damit entsteht keine zweite Übernahme der Körperbewegung.

Die Machbarkeitsprüfung prüft die originalen Framegrenzen: Position-Key-Dauern sind ganzzahlige Animationsframes, und die Translation zwischen ihren Grenzen ist stückweise linear. Körper-Sweeps prüfen diese Strecken vollständig; unter der Luftkurve muss auf der Ausgangshöhe ein unterstützter Gehweg vorhanden sein. Diese strengere Hostregel erlaubt keine Sprünge über Lücken, Treppen oder Hindernisse. Laufende Bewegungen verwenden zusätzlich die wirklichen festen Simulationstakte.

Beibehaltene Adapter: vorbereitete Körperbox (22,22,60), Gegnerhöhe 120, abwechselnde Lunge-Varianten nach Encounter-Seriennummer, unmittelbares Drehen vor dem Start, feste Angriffsbudgets des M3f-Hosts und sofortiges kollisionsgeprüftes Absetzen bei Unterbrechung. Ein deterministischer Encounter-Zufallsstrom verwendet die MSVCRT-artige Folge mit Seed8192; das ist nicht der Zufallszustand des laufenden PC-Originals. Der native Auswahlbaustein verbraucht Werte nur bei seinen tatsächlichen Zufallszweigen. Dynamische Actor-Kollision, vollständige GoalQueue, Squad-Koordination, weitere Sprünge, GameRules und Lunge-Effekt-/Soundnotifies sind weiterhin offen.

## Prüfung und Wiederholung

`rc-play-lunge NAVIGATION_SCENE OUTPUT_DIRECTORY` erweitert die M3f-Szene. `lunge-receipt.json` bindet Profile, Frames, Raten, Hit-Zeiten und abgetastete Originalkurven. `rc-play-lunge-check SCENE OUTPUT_DIRECTORY` prüft den echten Ablauf: Körperpositionen stimmen exakt mit der Host-Auswertung des geladenen Root-Tracks überein; jeder Bewegungsschritt besteht einen weiteren Körper-Sweep. Der erste Hit kommt in Tick38. Beide Varianten werden bis zum Ende ausgeführt. Originalkarten-Wandtests prüfen sowohl die Verweigerung vor dem Start als auch den Abbruch eines ausdrücklich erzwungenen Testpfads während der Bewegung. Tod/Hit-Unterbrechung, unterstütztes Absetzen, kompletter RNG-/Bewegungsreset, identische Zustände bei 120/30/10 Hostbildern, Niederlage und drei Projektilabschüsse bestehen.

Die bisherigen M3f-Navigationstests, der historische 1002-Kommandos-Replay sowie M2-, M3-, M3d- und M3e-Prüfungen bestehen. Der neue Szenencheck belegt Host-Bewegung und reale statische Kartenkollision, keine vollständige native Engine-Parität. Android-Ergebnisse, Signatur-/Ausrichtungsprüfung, Quellenarchiv und Artefakthashes stehen im `validation-manifest.json`. Ein persönlicher ARM64-Gerätetest dieser Version steht aus.

## Fortsetzung

Als nächster M3-Abschnitt folgen GameRules/Schwierigkeitsgrad und die Schadensmodifikation im aktiven Spielpfad. Danach die noch offenen Bot-/Goal-Felder, AimError/Zielhilfe sowie weitere Kampfklänge/-effekte. M3 bleibt offen; M4 ergänzt anschließend den zusammenhängenden Missionsablauf und Checkpoints.

Abgeschlossene Gefechtsprüfung: Beide vollständigen Android-Durchläufe über GPU/CPU bestehen, einschließlich Dateiöffnen, Touch-Zielen/Feuern/Bewegen/Sprung/Nachladen, Sieg/Niederlage, Neustart, Pause und Fehlerdatei mit Wiederherstellung. Im ersten CPU-Test lief die feste Wartezeit von 1,5 Sekunden für das Nachladen ab; das tatsächliche Android-Protokoll zeigte danach ammo60. Der Test wartet jetzt maximal acht Sekunden auf denselben geforderten Zustand und wiederholt keine Eingabe. Der anschließende vollständige CPU-Lauf besteht. Der fehlgeschlagene Versuch und die Diagnose bleiben unter android-cpu-attempt1 erhalten. 970 Bibliothekstests, Format, Clippy und die Desktop-ABI-Prüfung bestehen.

Emulator-Hostintervalle während aktiver Frames: GPU Median/p95 17,62/20,85 ms, CPU 66,56/83,48 ms; maximal verworfene Simulationszeit GPU0,0333 s/CPU0,0833 s. Das sind Host-Intervalle mit Software-GPU-Treiber, keine Display-FPS- oder ARM64-Leistungsabnahme. Die Profile und Auswertung sind unter performance-summary.json gebunden.

Abschließende Grafikprüfung: Echter Kontextneubau stellt Bild und Pose exakt wieder her; Backendwechsel erhält den Zustand. CPU/GPU-Startbilder: mittlerer Kanalfehler0,550/255, 99,139 % der Kanäle innerhalb zwei Stufen. Drei Wiederladungen bestehen die kurze Speichergrenzenprüfung; keine Langzeit-Leakfreiheitsbehauptung. APK-Signatur, ZIP-Ausrichtung und ELF-16-KiB-Ausrichtung beider ABIs sind geprüft. Quellen und Dateien sind im Liefermanifest und Quellenarchiv gebunden.
