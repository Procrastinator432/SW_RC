# geo_01a: Ereigniskette zur Absturzstelle

Die hier beschriebene Grundlage ist inzwischen in die spielbare Lieferung 0.20.0 integriert. [Aktuelle Szene, APK und Umfang](ORIGINAL_CRASH_MISSION.md). Die folgenden Befunde dokumentieren den vorausgehenden Grundlagenstand.

Entwicklungsstand 2026-10-11. Die startbare Lieferung bleibt **0.19.0** mit dem Bacta-Abschnitt. Dieser Schritt liefert die geprüfte Grundlage für Originalziel1, **„Secure area around crash“**; er enthält noch keine neue APK oder spielbare Absturzstelle.

## Originalbefund

| Originalexport | Bedeutung |
|---|---|
| Script739, Aktionen2899–2911 | BactaUsed, Sperre1122 freigeben, Ziel0 abschließen, Advisor abwarten, Rückzug auslösen, Barrikade617 mit50000 Schaden treffen und TrackerTarget01 zeigen |
| Trigger128 | SpawnGunshipArea; Radius600, geerbte Höhe40 |
| Fabriken133,134,277,441,836 | Je ein `Properties.BattleDroidTM`, PawnTag `GunshipDroids`, Todesereignis `DeadGunshipDroid` |
| Script129, Aktionen1537–1544 | Droids nach Spawn unverwundbar machen, auf DialogThereIsTooMany warten, Unverwundbarkeit entfernen, auf CrashSiteSecured warten, Ziel1 abschließen |
| Trigger608 | DialogThereIsTooMany; Radius500, geerbte Höhe40 |
| Counter620 | Fünf DeadGunshipDroid-Meldungen lösen CrashSiteSecured aus |
| Script911 / Aktion3293 | Nach CrashSiteSecured Kollision der Ausgangssperre915 abschalten |

Der Default der Droiden ist **75 Gesundheit**, das Originalmodell heißt `Droid.BattleDroid`. Das sind fünf Battle Droids und keine Übernahme der drei Geonosianer aus der eigenen Kampf-Testszene. `ACTION_ObjectiveUpdate`1544 hat `ChangeToObjNum=1`, verwendet aber den geerbten UpdateAction0: Ziel1 wird abgeschlossen; der bloß gesetzte ChangeToObjNum-Wert schaltet diesen Aktionstyp nicht um.

Die Wartezeit vor der Barrikadenzerstörung enthält eine Sekunde, die Dauer der blockierenden Advisor-Zeile `CA1G1791` und den originalen Float-Wert des anschließenden Timers (ungefähr1,89 Sekunden). Die PCM-Dauer beträgt ungefähr3,202 Sekunden. Eine unabhängige Python-WAV-Auswertung prüft den Rust-Extraktor.

## Umsetzung und Abnahme

`rc-game::crash` stellt einen begrenzten Ereignishost bereit. Er nimmt einen Bacta-Abschluss, Zeitfortschritt, Spielerposition/Körperausdehnung und bestätigte Todesmeldungen entgegen. Er gibt geordnete Effekte für Kollision, Geräusche, Ereignisse, Fabriken, Unverwundbarkeit und Zielabschluss zurück. Der aktuelle `Game`-Host ruft dieses Modul noch nicht auf; die Effekte verändern deshalb noch keine Szene und erzeugen keine sichtbaren Gegner.

Der Extraktor `rc-crash-assets` liest die Originalkarte und aufgelöste Klassendefaults. Er prüft die Originalaktionsreihenfolge, UpdateAction-/bMakeGod-Defaults, Kollisionsflags und Fabrikbindungen. Die Quelle jeder Fabrik-Health-/Mesh-/Kollisionsangabe bleibt im Receipt enthalten. Artefakte liegen unter `analysis/mission2`.

`rc-crash-check` prüft alle120 Todesreihenfolgen, vier Taktraten, zeitliche Reihenfolge, unabhängige Bacta-/Spawn-Skriptzweige, Unverwundbarkeit vor Trigger608, genau fünf Spawn-Effekte, einmaligen Abschluss und ungültige Eingaben ohne Zustandsänderung. Wiederholte Meldungen derselben Fabrik werden im Host abgefangen: Das ist eine Schutzmaßnahme des Ports; der originale Counter selbst dedupliziert keine beliebigen Trigger-Ereignisse. Ein früher Ortskontakt wird nicht als wartendes Skriptereignis gespeichert. Die Zylinder-Kontaktprüfung und die geordnete gemeinsame Tick-Auswertung sind Adapter, keine belegte native Trigger-/Scheduler-Parität.

`Verify-CrashSiteFlow.py` bildet die Originalaktionsdaten separat auf die erwartete Effektfolge ab und vergleicht sie mit allen Timer-Traces. Sie prüft die fünf Fabriken, das Counter-Ereignis und die Advisor-Dauer gegen die ursprünglichen Soundbytes. 986 Bibliothekstests, Clippy, Format, Whitespace-Prüfung, das historische1002-Kommandos-Replay und der vollständige physische Bacta-Wegtest sichern die bestehende Lieferung ab.

## Vorbereitete Originaldarstellung

`rc-crash-visual-check` prüft das ursprüngliche Battle-Droid-Modell und die DC-15s mit neun Originalsequenzen an insgesamt36 Animationsständen. Skinning, endliche Dreieckskoordinaten und Basismaterialien sind geprüft; die serialisierten Meshdaten und Bounds liegen unter `analysis/mission2/visuals`. Die Droiden besitzen die Treffersequenz **Impact**, während der bisherige Geonosian-Host **hit** erwartet. Diese Auswahl muss bei der Integration ausdrücklich nach Gegnertyp erfolgen; Originalsequenzen wurden nicht umbenannt. Eine ursprüngliche Trefferzustandsmaschine oder vollständige Shader-Parität ist damit nicht belegt.

## Noch erforderliche Integration

1. Effekte an dynamische Barrikaden-/Kollisionszustände und sichtbare Geometrie anbinden. CPU/GPU müssen denselben Zustand sehen; Neustart muss Sperren, Fabriken und Zähler wiederherstellen.
2. Den physischen Weg von der Bacta-Station über die Originaltracker zur Absturzstelle prüfen. Tracker02/03 sowie die weiteren Dialog-/Rückzugsaktionen sind inventarisiert, aber noch nicht ausgeführt.
3. Fünf echte Battle-Droid-Instanzen mit Originalmodell und passenden Animationen erzeugen. Schaden, Unverwundbarkeit und Todesmeldungen mit dem Kampfhost verbinden; bisherige Geonosian-Nahkampfregeln sind kein Ersatz für deren Schuss-KI.
4. Beide Originalziele zusammenhängend spielen, danach erst eine neue APK/Szene liefern und Android GPU/CPU prüfen.

Keine allgemeine Script-VM, vollständige PawnFactory-/KI-Parität, physische ARM64-Abnahme oder neue spielbare Missionsabnahme wird mit diesem Grundlagenstand behauptet.
