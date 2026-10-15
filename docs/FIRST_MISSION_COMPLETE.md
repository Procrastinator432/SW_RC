# Mission 1 durchspielbar bis zum Kartenwechsel – 0.45.0

Der spielbare Missionsablauf führt von GEO_01A über GEO_01B und GEO_01C bis zum tatsächlichen Wechsel nach GEO_03A. Alle sieben C-Ziele einschließlich Sun Facs Abschuss und Fabrikaufzug werden erfüllt. Der Wechsel lädt den originalen Fabrikeinstieg, übernimmt Gesundheit, Munition, Granaten sowie Scorch, Fixer und Sev und setzt die neue Sitzung am Original-PlayerStart fort.

## Starten

Das Paket `mission1-playable-0.45.0.zip` enthält APK und alle benötigten Szenen. APK `republic-commando-0.45.0.apk` installieren; die vier Szenen auf das Gerät kopieren. Anschließend:

1. Im Spielabschnitt `geo_01a-first-mission.rcplay` öffnen und bis zum Kartenende spielen.
2. **Weiter: GEO_01B** wählen und `geo_01b.rcplay` öffnen.
3. Nach dem zweiten Abschnitt **Weiter: GEO_01C** wählen und `geo_01c.rcplay` öffnen.
4. In C Sev treffen, den Strategieraum, den Wachraum, das Türbrech- und Scharfschützenmanöver abschließen. Nach Sun Facs Abschuss zum Fabrikaufzug zurückkehren und durch die automatisch öffnende Tür gehen.
5. **Weiter: GEO_03A** wählen und `geo_03a.rcplay` öffnen. Am Fabrikeinstieg erscheint „Mission 1 abgeschlossen“; der Spieler und alle drei Squad-Mitglieder sind übernommen.

Die Szenenwahl ist weiterhin ein Android-Dateidialog. Die passende Datei wird nach Betätigen von „Weiter“ ausgewählt; native Quelle, Ziel und abgeschlossene Ziele werden vor dem Austausch geprüft. Falsche oder beschädigte Dateien ersetzen die abgeschlossene Sitzung nicht. Ihr Bild und Audiozustand bleiben bei einem fehlgeschlagenen Reiseversuch erhalten.

Alternativ kann eine Karte direkt geöffnet werden. B/C/Fabrik stellen dann eine vorhandene passende gespeicherte Übergabe wieder her. Die Dateien bleiben getrennt: `geo_01a-handoff.json`, `geo_01b-handoff.json`, `geo_01c-handoff.json`. Ohne Übergabe startet die Zielszene eigenständig mit Standardwerten und ohne übernommenen Squad; das ist kein nachgewiesener Missionsabschluss. Neustart auf GEO_03A stellt den übernommenen Fabrikeinstieg wieder her.

## Inhalt und Originalbindung

A und B sowie C entsprechen den bereits geprüften Szenen; die C-Datei ist bytegleich zu 0.44.0. GEO_03A ist neu: 24.985.911 Bytes, 118.736 sichtbare Dreiecke, 184 statische Kollisionsquellen und Original-PlayerStart1/Actor8 bei `[-2555.739501953125, 2654.17138671875, 6920.81640625]` mit Originalblickrichtung. Alle übernommenen BSP-/Mesh-Flächen sind texturiert; der Oberflächenbeleg meldet null ungelöste Bindungen.

4.715 Hologramm-/Falloff-/Konstantfarb-/volumetrische Effektflächen sind als Effekte ausgelassen. Der originale Sniperscope-Shader ohne Diffuse verwendet seine SelfIllumination-Textur. Der statische Kollisionsimport lässt ausschließlich den verborgenen DefaultProp-Hilfsträger `Mover1`/Actor740 mit Tag `ElevatorLightMover` aus; seine native dynamische Hilfskollision ist nicht modelliert. Keine pauschale Abschaltung der Fabrikwelt-Kollision. `import-scope.json` und `original-bindings.zip` dokumentieren Originalwerte und Grenzen.

Schema3 gilt ausschließlich für C→GEO_03A über Originalteleporter39 mit allen sieben Zielen und beigetretenem Scorch/Fixer/Sev. Ressourcen und Mitgliedschaft/Gesundheit/Schilde/Kampfunfähigkeit werden validiert, bevor ein Zustand übernommen wird. Alte Positionen, Gegner und Wege gelangen nicht in die neue Karte. Die drei Mitglieder erhalten getrennte, boden- und kollisionsgeprüfte Einstiegspunkte, ihre bisherigen Originalskins und Waffen sowie einen begrenzten physischen Folgehost. Fehler beim letzten Mitglied lassen auch Spieler und vorherige Mitglieder unverändert.

## Abnahme

- Vollständiger normaler Eingabestrom: 18.730 A + 16.414 B + 11.800 C, danach produktiver Reiseaufruf nach GEO_03A. Keine Zustands-/Positions-/Ressourcenabkürzung im Missionslauf.
- Am Missionsende und Fabrikeintritt: 65 HP, 14/65 Munition, vier Granaten; Scorch/Fixer/Sev jeweils 100 HP/100 Schilde. Alle sieben C-Ziele sind erfüllt.
- 360 Fabrikeingaben prüfen Bewegung, einen Sprung, zwei Schüsse, Nachladen und Ducken. Danach 65 HP/20/57 und weiterhin vier Granaten; alle drei Mitglieder folgen im Eintrittsbereich.
- Frühe/falsche/beschädigte Reiseziele werden abgewiesen. Zwölf fehlerhafte gespeicherte Übergaben einschließlich Sev-Daten werden atomar abgewiesen. Gespeicherte Wiederaufnahme sowie Neustart/Wiederholung vergleichen Zustand und Grafikpaket exakt.
- Windows- und Android-x86_64-C-Schnittstelle spielen den ganzen A/B/C-Strom und tatsächlichen Fabrikwechsel ab. Die APK prüft separat gespeicherte Wiederaufnahme mit dem tatsächlich auf Android erzeugten C-Handoff, GPU, Touch-Bewegung/Schießen/Ducken, Originalaudio und Pause.
- 1.120 Bibliothekstests, Clippy/Format, drei Java-Kampagnenverknüpfungen und Java-Dateileser mit bekanntem/unbekanntem Umfang bei 96 MiB Heap bestehen.

Replays, Zustände, Bilder, Originalbelege, Quellstand und SHA-256 stehen in `delivery/validation-manifest.json`. Die 604 bisherigen eingefrorenen Nachweisdateien bleiben unverändert.

## Bedeutung des Abschlusses

**Mission 1 ist als zusammenhängender Port-Spielablauf einschließlich ihres tatsächlichen Kartenwechsels abgeschlossen.** Das ist keine vollständige 1:1-Rekonstruktion der Originalengine. Die dokumentierten Grenzen bei allgemeiner KI/Squad/Markern, Kampfphysik, Shadern, Bacta, Tür-/Granatenschaden, Musik/Partikeln und Teilen der Inszenierung bestehen weiter. Ein vollständiger manueller Touch-Lauf und die physische ARM64-Abnahme dieser Version fehlen noch; ARM64 ist gebaut, der automatische Android-Lauf verwendet x86_64.

GEO_03A ist der Eintritt in den anschließenden Fabrikabschnitt. Dessen Aufgaben, Gegner, Aufzugsskripte und Inszenierung sind nicht Bestandteil des abgeschlossenen Mission-1-Ablaufs und noch nicht integriert. Weitere Arbeit kann nun auf Geräteabnahme und Qualität der ersten Mission oder den Fabrikablauf ausgerichtet werden.
