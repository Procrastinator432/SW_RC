# GEO_01C: Fabrikaufzug und Missionsabschluss – 0.44.0

Die dritte Karte lässt sich nach Sun Facs Abschuss bis zum Fabrikaufzug abschließen. Die beiden Originaltürflügel bewegen sich sichtbar und mit ihrer Kollisionshülle. Beim Betreten des freigegebenen Ausgangs wird Ziel 6 erfüllt, die Sitzung angehalten und die Übergabe für `GEO_03A.ctm#playerstart` erzeugt.

## Ausprobieren

- APK: `analysis/mission27-map3-elevator/delivery/republic-commando-0.44.0.apk`.
- Dazu passende Szene: `analysis/mission27-map3-elevator/delivery/geo_01c.rcplay` (46.877.914 Bytes).
- Die APK aktualisieren und die neue Szene über „Szene öffnen“ auswählen. Eine gespeicherte GEO_01B-Übergabe wird wie bisher beim Öffnen von C übernommen. Die bisherigen A/B-Szenen bleiben verwendbar.
- Den bisherigen C-Ablauf spielen, Sev die Scharfschützenposition zuweisen und den Abschuss abwarten. Danach durch den AA-Raum und den Wachraum zum Aufzug zurückgehen. Restgegner bleiben aktiv. Die Aufzugstür öffnet bei Annäherung; durch die Tür gehen. Der Eintritt in den Ausgangsbereich schließt die Mission ab, eine zusätzliche Benutzen-Taste ist nicht erforderlich.
- „Mission abgeschlossen“ erscheint; Neustart setzt die gesamte C-Szene einschließlich Tür und übernommener Einstiegsvorräte zurück. Die App speichert die neue Übergabe atomar als `geo_01c-handoff.json`.

**Die Zielkarte GEO_03A ist noch nicht als ladbare Szene eingebunden.** Diese Lieferung erreicht den Abschluss am Originalausgang und speichert die Reiseanforderung. Ein tatsächlicher Wechsel in die Droidenfabrik folgt als nächster Meilenstein.

## Originalbindung

Tür 465/466 verwendet originale KeyPos ±192 auf X, Originalflächen/Texturen, Originalhüllen und die aufgelöste MoveTimes-Vorgabe. Hack-/Breach-Komponenten 467/469 sind im Original bereits „Unlocked“. Trigger 468 hat Radius 200/Höhe 100; Clone-Annäherung öffnet, der Rückweg schließt. Beim Schließen verhindert eine Körperprüfung das Schließen durch Spieler oder Squad. Der Aufzug selbst besteht aus statischen Originalmeshes; es wird keine zusätzliche Plattformfahrt erfunden.

Trigger 227 wird durch ElevatorTrigger freigegeben und meldet ContinueIntoElevator. Die ConditionSunFacDead-Freigabe aus Skript 318 steuert den Originalausgang 39 mit URL GEO_03A, Radius 900 und ursprünglicher Standardhöhe. Ziel 6 braucht erfolgreiche Sun-Fac-Sequenz, Ziele 0–5, eine vollständig offene Tür, Spielerüberlappung und freien Weg zum Teleporter. Türannäherung allein beendet die Mission nicht.

Schema 3 enthält Spielerzustand und Vorräte, Scorch, Fixer, Sev sowie die sieben C-Ziele. Frühere Schema-1/2-Übergaben bleiben kompatibel. Der Zielkartenimport und das Anwenden dieser neuen Übergabe auf deren PlayerStart fehlen noch; Versuche, erneut A/B/C als Ziel einzusetzen, werden ohne Änderung der abgeschlossenen Sitzung abgelehnt.

## Nachweise

Der normale Eingabelauf umfasst 18.730 A-, 16.414 B- und 11.800 C-Eingaben. A→B→C wird über die produktive Reise-/Wiederaufnahme-Schnittstelle ausgeführt. Der neue C-Abschnitt setzt den eingefrorenen Sun-Fac-Erfolgslauf fort und läuft über die originale Welt zurück; keine Spieler-, Gegner-, Ressourcen- oder Ereignisabkürzung im erfolgreichen Lauf. Eine getrennte Planungswelt ohne bereits bekannte Tore dient nur der Wegberechnung; der Spielkörper bleibt kollisionsgeprüft.

Ergebnis: alle sieben Ziele, 65 HP, 14/65 Munition, vier Granaten, 22 Abschüsse, ein verbleibender Gegner, 192 Audioereignisse. Alle drei Squad-Mitglieder sind beigetreten und werden mit Zustand übergeben. Exakte Wiederholung und Neustart vergleichen Zustand und Grafikpaket; die Tür-/Squad-Taktung stimmt bei 20/60/144 FPS überein. Nach Abschluss ändern weitere gültige Eingaben die Sitzung nicht.

Windows- und Android-x86_64-Schnittstelle prüfen den gesamten A/B/C-Lauf, Exit-39-Übergabe, alle Statuszahlen, falsche/kaputte Zielkarten, gespeicherte Wiederaufnahme sowie Neustart. Der frühere Lauf ohne Sev wird unverändert auf der neuen Szene als Niederlagenregression ausgeführt. Die APK-Prüfung deckt C-Einstieg, GPU, Originalaudio, Touch-Bewegung/Schießen/Ducken und Pause ab; sie ersetzt keinen manuellen Missionslauf. Java-Dateileser mit bekanntem/unbekanntem Umfang besteht bei 96 MiB Heap; 1.120 Bibliothekstests, Clippy und Formatprüfung bestehen.

## Verbleibende Grenzen

Die erste Mission besitzt jetzt einen durchgehenden spielbaren Abschluss über A/B/C bis zur Reiseanforderung. Originalparität bei allgemeiner KI/Squad, Markern, Projektilen, Granaten-/Türschaden, Bacta, Shadern, Musik/Partikeln, Grievous und weiteren Inszenierungsdetails bleibt unvollständig. Türsteuerung, Überlappungen und Einklemmschutz sind begrenzte Hosts; die vollständige native Mover-/Encroachment-/Antiportal-/Endsymbol-Ausführung fehlt. Physischer ARM64-Gesamtlauf und vollständiger manueller Touch-Durchlauf stehen aus.

Prüfer: `rc-map3-elevator-build`, `rc-map3-elevator-check`, `scripts/Validate-Map3ElevatorAbi.py`, `scripts/Test-Map3ElevatorAndroid.py`. Replays, Übergaben, Berichte, Originalbindungen und Quellstand liegen in der Lieferung; SHA-256-Zuordnung in `validation-manifest.json`.
