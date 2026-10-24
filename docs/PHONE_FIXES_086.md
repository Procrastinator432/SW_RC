# Handytest: Scorchs Bunkerausrichtung und Ruckler in Karte 2

Stand: 29.10.2026 · **0.86.0-phone-fixes**, Android-Code 90.

Die Handy-Rückmeldung zu 0.85 bestätigt den vorgesehenen musiklosen A-Abschnitt, funktionierende Musik auf B und keine bemerkten Texturlücken in A sowie am Anfang von B. Das ist eine positive Abnahme dieser tatsächlich gespielten Bereiche, keine Abnahme der ganzen B-/C-Karte oder der gesamten ersten Mission.

## Korrekturen

Scorchs A-Bunkerpfad verwendete die Rotation der **montierten Ladung** zugleich für den Körper. Deren nach außen gerichtete Achse zeigt genau entgegen der Arbeitsrichtung. Die Körperpose und die an seiner Hand befestigte Ladung verwenden jetzt die um 180 Grad gedrehte Arbeitsrichtung; das bereits montierte Objekt behält seine eigene korrekte Rotation. Laufweg, Kollisionskörper, Arbeitstimer und Missionslogik bleiben erhalten. Die Korrektur wird beim Rendern abgeleitet und wirkt auch in vorhandenen Teilspielständen. Nach Unterbrechung richtet sich eine tatsächlich laufende Figur weiterhin nach ihrer Bewegung. Der getrennte, am Gerät richtig beobachtete B-Arbeitsmarkerpfad wird nicht gedreht.

Für B ließ sich eine konkrete Ursache der Ruckler reproduzieren: Wegsuchen prüfen viele unterstützte Korridore. Jede ihrer Kollisionsabfragen validierte erneut sämtliche Ebenen auch weit entfernter BSP-Hüllen. Das summierte sich zu langen Simulationsschritten. Für unveränderlich geteilte `Arc<HullSet>` wird die vollständige Gültigkeitsprüfung jetzt einmal je Objektidentität erfasst. Danach schließt die bereits vorhandene exakte AABB-Grenze entfernte Hüllen aus, bevor deren Ebenen erneut abgefragt werden. Mögliche Treffer durchlaufen weiter denselben Solver in derselben Reihenfolge. Fehlerhafte, unvollständige und vollständig solide Sonderwelten behalten den bisherigen Fehler-/Abfragepfad.

Der Cache ist pro Thread auf 4.096 Einträge begrenzt. Schwache Besitzer verhindern die Wiederverwendung einer alten Speicheradresse unter einem gültigen Cacheeintrag. Ausgetauschte Tor-/Kollisionsgeometrie erhält ihre eigene Identität; neue oder kalt geladene Welten werden neu geprüft. Rohe veränderliche HullSets benutzen weiterhin den bisherigen Pfad. Es werden weder Kollisionskörper verkleinert noch Gegner, Materialien oder Audiokanäle abgeschaltet. Der Cache ist kein Spielzustand und wird nicht gespeichert.

## Messung und Prüfungen

Der kontrollierte Windows-Test vergleicht dieselben ersten 3.600 Original-Replayeingaben je Karte ohne Zeichnen, Dateispeicherung oder Audiowiedergabe innerhalb der gemessenen Simulationszeit. Er misst die CPU-Ursache, keine Handy-Bildrate.

| B-Simulation | 0.85 | Optimierter Vergleichslauf |
|---|---:|---:|
| Median pro Schritt | 2,36 ms | 0,30 ms |
| 95. Perzentil | 15,22 ms | 1,80 ms |
| Maximum | 871,71 ms | 113,04 ms |
| Schritte über 15 ms | 192 | 1, beim ersten Schritt |

Alle **7.200 Checkpointzustände** aus A und B bleiben im direkten Vergleich bytegleich. Die erste Vorbereitung kann weiterhin eine Spitze verursachen; der Test belegt keine vollständige Beseitigung sämtlicher Geräte-Ruckler. Ein Vorversuch mit Cache für abgeleitete Trennebenen verbesserte B nicht und wurde verworfen.

Die Lieferungsnachweise ergänzen den finalen Vergleichslauf, vier A-Arbeitsphasen einschließlich kalter Bild-/Zustandswiederherstellung, die bisherigen B-Ladungsfälle und den vollständigen A→B→C→Fabrik-Lauf. Drei neue Kollisionsprüfungen vergleichen beschleunigte und rohe Abfragen einschließlich tangentialer Kontakte, Überlappungen, Gleichständen, falscher Eingaben, entfernter ungültiger Hüllen und ausgetauschter Geometrie. Der zentrale Check, beide Android-ABIs und die Emulatorfälle werden in der [Prüfzusammenfassung](../build/phone-fixes/verification-summary.json) gebunden.

## Installation und nächster Test

[APK 0.86 installieren](../build/phone-fixes/android/artifacts/republic-commando-0.86.0-phone-fixes.apk). **Das Szenenpaket 0.85 bleibt unverändert verwendbar; kein erneuter Import erforderlich.** Vorhandene Spielstände bleiben verwendbar.

Am Handy insbesondere Scorchs Hände/Körper am A-Bunker sowie mehrere Minuten Bewegung und Squad-Folgen auf B prüfen. Für die bisher nur pauschal beschriebene geringere Ausarbeitung von B ist eine systematische Abnahme in Abschnitten vorgesehen: Einstieg und erste Aufträge, weitere Kämpfe und Squadwege, danach Türen/Ziele bis C mit Speichern/Fortsetzen. Weitere Inhalte und Inszenierung sind damit noch nicht als abgeschlossen erklärt. Musik und Texturen der positiv getesteten Bereiche werden ohne neuen Fehlerbefund beibehalten.
