# Director-Rotationsanwendung und relative Korrektur

Stand: 2026-10-07. `skeletal_director_apply::apply_director_prepared` verbindet
die belegte Director-Auswahl, Weltraum-Umrechnung, Rotationsvorbereitung mit
History und Winkelgrenzen sowie die nachfolgenden Schreibzugriffe aus
**engine 10501bb0, ApplyDirector**. Alle Szenenwerte werden weiterhin als
Snapshots übergeben; es besteht noch keine Bindung an laufende Original-Actors.

## Verbundener Ablauf

`build_mutable_directed_pose` berechnet zunächst die lokale Knochenmatrix und
ihre Elternverknüpfung. Danach wählt es wie zuvor den ersten aktiven passenden
Director anhand seines signed31-Knochenindex. Der ausgewählte Director ist jetzt
veränderbar, damit History und Budget-Zustand über Folgeaufrufe erhalten bleiben.

Die Director-Matrix wird bei fehlendem Lokal-Bit **+4c Bit0** mit der originalen
inversen MeshToWorld-Matrix in den lokalen Raum umgerechnet. Die ursprünglichen
Matrixworte im Director bleiben dabei unverändert. Bei aktiver Rotation folgt
die bereits geprüfte History-/Winkelvorbereitung.

Ist **+4c Bit1** gesetzt, wird anschließend die aktuelle Knochenmatrix von rechts
mit der vorbereiteten Director-Matrix multipliziert: `current * prepared`.
Der Originalabschnitt **105022fb–1050276f** benutzt unterschiedliche
Additionsreihenfolgen für die einzelnen Matrixelemente. Sie werden durch
`Generate-DirectorProducts.py` aus den Original-SSE-Ausdrücken erzeugt und in
`skeletal_director_products.rs` als getrennte f32-Operationen bewahrt.

Die ersten drei Zeilen der resultierenden Matrix ersetzen die Rotationszeilen
des ausgewählten Knochens, jeweils komponentenweise multipliziert mit
`(actor_scale.x, actor_scale.y, actor_scale.z, 0)`. Die Skalierung wirkt damit
auf Spaltenkomponenten, nicht als ein einheitlicher Faktor je Zeile. Der
Originalhelfer **core 101149b0, FPlane::operator*** lädt die Skalierungsplane
zuerst; auch diese Operandenreihenfolge wird bewahrt. W wird mit +0 multipliziert,
was bei nichtendlichen Eingaben nicht dasselbe wie ein bedingungsloses Löschen ist.

Nach der relativen Anwendung kann eine Korrekturschleife frühere Knochenmatrizen
ändern. Erst danach folgen Translation und abschließende Plane-Skalierung:

- Bei aktivem Byte **+48** wird die gesamte letzte Zeile der Arbeitsmatrix
  einschließlich W in den ausgewählten Knochen kopiert.
- Bei aktivem Byte **+4a** werden dessen erste drei Zeilen nochmals
  komponentenweise mit den entsprechenden Arbeitsmatrixzeilen multipliziert.
  Bei gleichzeitiger Rotation ist dies ein zusätzlicher Schritt nach der
  Rotationsanwendung.

## Korrekturschleife für frühere Knochenmatrizen

Der signed31-Index aus Director **+4** ist der Anfang des Korrekturbereichs.
Bei gesetztem relativem Bit und `first < selected_bone` läuft das Original
**selected_bone-1, selected_bone-2, …, first** abwärts. Es handelt sich um
einen zusammenhängenden Indexbereich; die Schleife folgt nicht den Parent-Pointern
des Skeletts. Der Name „Vorfahrenkorrektur“ allein beschreibt dies daher ungenau.

Für jeden früheren Index wird das endgültige vorbereitete Quaternion potenziert:

`exponent = f32(previous - first + 1) / f32(selected_bone - first + 1)`

Aus diesem Quaternion entsteht eine Korrekturmatrix mit Nulltranslation.
Der Abschnitt **10502921–10502c84** multipliziert die ersten drei Zeilen der
früheren Knochenmatrix mit ihr. Auch hier bleiben die je Komponente verschiedenen
Original-Additionsfolgen erhalten. Die letzte Zeile der früheren Matrix bleibt
unverändert. Später berechnete Kinder verwenden die gegebenenfalls bereits
korrigierten Elternmatrizen.

Ein Fehler der Potenz-/Math-Schnittstelle erhält die bereits geschriebene
Director-History, die Rotationszeilen des ausgewählten Knochens und alle zuvor
erfolgreich korrigierten früheren Knochenmatrizen. Die abschließende Translation
und Plane-Skalierung des ausgewählten Knochens werden dann nicht ausgeführt.
Negative oder fehlende Korrekturindizes liefern in Rust ausdrückliche Fehler;
ungültige Original-Speicherzugriffe werden nicht emuliert. Die Diagnoseeingaben
verwenden gültige Indizes und kleine Skelettgrößen ohne Integerüberläufe.

## Unabhängige Prüfung

`rc-directed-rotation-pose-check` verwendet die 900 bereits geprüften
Originaltrack-/Channel-Stack-Spielposen und vier kontrollierte Director-
Konfigurationen: absolute lokale Rotation, relative lokale Rotation mit beiden
Winkelgrenzen, relative Weltraum-Rotation mit zeitlicher Grenze und absolute
Weltraum-Rotation mit absoluter Grenze. Die Konfigurationen decken auch die
Kombination mit Translation und Plane-Skalierung ab.

Jede Konfiguration wird zweimal mit derselben veränderbaren History ausgewertet;
vor dem zweiten Aufruf wird ein neues diagnostisches Budget von 0.125 vorgegeben.
Ein inaktiver erster Treffer und ein weiterer aktiver späterer Treffer prüfen
die Auswahl des ersten aktiven Directors. Die vorgegebenen Actor-Skalen sind
(2,0.5,3); MeshToWorld ist eine nichtuniforme Skalierung mit Translation.

`Record-DirectedRotationPoses.py` prüft unabhängig sämtliche Posen und mutierten
Director-Snapshots. Für die beiden neuen Matrixprodukte interpretiert es die
Originalbefehle numerisch mit Registern, Stack und veränderbarem Speicher,
anstatt die generierten Rust-Ausdrücke auszuwerten. Die Quaternion-Helfer nutzen
die separat geprüfte portable Math-Policy und den Original-ASM-Interpreter der
Matrixkonversion. Ergebnisse:

- **7.200 Posen / 239.296 Knochenmatrizen**, bitgenau unter der portablen Policy.
- 3.600 History-Initialisierungen und 3.600 relative Matrixverknüpfungen.
- 9.312 Korrekturen früherer Knochenmatrizen.
- 2.015 zeitliche und 2.886 absolute Begrenzungen.
- **64 allgemeine Produktproben** zusätzlich gegen den numerischen ASM-
  Interpreter und das mathematische f64-Matrixprodukt geprüft. Die maximale
  Produktabweichung beträgt `1.431e-6` für diese Eingaben.

Sechs neue Tests prüfen Rotationszeilen und Spaltenskalierung, letzte
Translationszeile, doppelte Plane-Skalierung, relative Verknüpfung, absteigende
Exponenten, Teilzustände bei Fehlern sowie Auswahl und Kindvererbung.
**318 Workspace-Tests**, Clippy mit `-D warnings` und Formatprüfung bestanden.
Nachweise und Hashes: `analysis/reports/directed-rotation-poses-validation.json`
und `analysis/evidence.json`.

## Noch offene Integration

Die früheren lokalen/transformativen Director-Helfer bleiben bewusst auf ihren
unveränderbaren Translations-/Skalierungspfad beschränkt. Rotation wird über den
neuen veränderbaren Einstieg ausgewertet. Original-Director-Datenbindung,
vorgelagerte Budget-Akkumulation, Actor-/MeshToWorld-Snapshots aus dem laufenden
Spiel, vollständige Pose-Vorbereitung,
inverse Bind-Posen und Skinning sind weiterhin offen. Die portable Math-Policy
ist keine bitgenaue x87-/RSQRTSS-Emulation. Es gibt damit noch keine animierte
Spielfigur oder spielbare native Portierung. Android und Emulator am Schluss.

Die Bounds-Publikation ist über den
[verbundenen Pose-/Bounds-Einstieg](SKELETAL_DIRECTED_BOUNDS.md) integriert und
geprüft. Auch der [SetBonePlace-Einstieg](SKELETAL_SET_BONE_PLACE.md) für Anlage
und Aktualisierung ist inzwischen rekonstruiert; seine globale Namens-/Alias-
und Script-/Actor-Bindung bleibt ausdrücklich offen.
