# DC-15s: originaler Feuermodus und gespeicherte Eingaben

## Originalbefund und Umfang

`rc-dc15-assets` exportiert jetzt auch `CTInventory.DC15s.FireMode`. Die lokale Originalinstallation löst diesen Wert über `engine.Weapon` als Byte **0** auf (Nullinitialisierung, kein expliziter Property-Payload). `Weapon.EFireMode` definiert 0 als `FM_SemiAuto`, 2 als `FM_Automatic`. Der Originalkommentar verlangt für SemiAuto einen neuen Feuerdruck pro Schuss. `Weapon.ServerFire` und `NormalFire.Tick` aktivieren beziehungsweise wiederholen automatisches Feuer nur über `bAutoFire`; `DC15s.Fire` ergänzt die Ladefreigabe.

Der Szenenbuilder übernimmt den aufgelösten Modus samt Herkunft. Burst und animationsgesteuertes Feuer werden als ununterstützt zurückgewiesen. Es wird keine vollständige Weapon-Zustandsmaschine behauptet: AnimEnd-/Idle-/ClientThink-Wechsel, erzwungene Schüsse, Zielhilfe, Streuung, Rückstoß und ballistische Originaltreue bleiben eigene Arbeiten.

## Verhalten

- Neue DC-15s-Szenen verlangen Loslassen und erneutes Drücken. Halten löst einen Schuss aus.
- Zu frühe Eingaben während Schussfrist, leerer Energie oder einer Waffensperre werden nicht bis zur nächsten Feuerfreigabe aufbewahrt. Danach ist erneut zu drücken.
- Physische Eingabeflanken werden bei jedem `advance` erfasst, auch ohne ausgeführten Simulationstakt. Ein kurzer Druck bleibt bis zum nächsten Takt erhalten. Mehrere Eingaben innerhalb eines Takts werden wie bisher zu höchstens einer ausstehenden Anfrage zusammengefasst.
- Ein langsamer Renderframe mit mehreren Simulationstakten vervielfacht einen halbautomatischen Druck nicht. Ein explizites automatisches Profil wiederholt dagegen gehaltenes Feuer.
- Haltezustand und ausstehende Anfrage werden im Checkpoint gespeichert; Kartenübergaben erhalten den Haltezustand. Fortsetzen oder Übergang erzeugen keinen zusätzlichen Druck. Neustart setzt ihn zurück.
- Alte Szenen ohne `fire_mode` behalten ihren bisherigen gehaltenen Feuerpfad. Deren Standardzustand serialisiert kein zusätzliches Haltefeld.

Die Replay-Vorbereitung erzeugt für die neue Halbautomatik gewöhnliche Loslass-/Druckeingaben an sichtbaren Kampfzielen. Sie setzt weder Lebenspunkte noch Gegner, Energie oder Missionsziele. Die fertigen Aufzeichnungen müssen anschließend unabhängig durch die Missions- und Checkpointprüfungen laufen.

## Prüfung

Gezielte Rustregressionen prüfen Halten/Loslassen, abgewiesene frühe und leere Eingaben, automatische Profile, 20/60/144 FPS, eine zwischen zwei Simulationstakten liegende Loslass-/Druckfolge mit exaktem Checkpoint-Fortlauf, Neustart und Übergabe bei gehaltener Taste.

Der zentrale Gesamtbau besteht mit je **1.178 Bibliothekstests in Debug und Release**, Clippy, Java-Prüfungen, Android ARM64/x86_64 und 30 Szenenstufen. Der A/B/C→Fabrik-Lauf besteht mit 25 Checkpointproben; die Präsentationsprüfung erzeugt 39 Spieleransichten. Die erweiterte Intro-Sperrregression wurde anschließend separat nochmals in Debug und Release geprüft. Acht historische 0.52-/0.53-Spielstände werden durch die neue produktive DLL exakt wiederhergestellt.

Das normale APK besteht im Android-x86_64-Emulator echten Paketimport, Prozessverlust/Fortsetzen, kompakte Touch-Steuerung, Energieanzeige und alle drei automatischen Kartenwechsel. Der zusätzliche Touch-Test im Emulator bestätigt: 1,1 Sekunden Halten feuert einmal; erneutes Antippen feuert genau einmal zusätzlich. Die Übergangstests verwenden echte Replay-Endspielstände als Einstieg und ersetzen keinen frei gespielten vollständigen Missionslauf.

Neue zusammengehörige Prüflieferung **0.54.0-dc15-trigger**:

- [APK](../build/releases/0.54.0/republic-commando-0.54.0-dc15-trigger.apk)
- [Szenenpaket](../build/releases/0.54.0/mission1-scenes.rcpack.zip)
- [Prüfbericht](../build/releases/0.54.0/verification-summary.json) und [Quell-/Dateizuordnung](../build/releases/0.54.0/source-and-delivery.json)

APK und Szenenpaket gemeinsam aktualisieren, Paket importieren und eine neue Mission starten. Alte Szene-/Spielstandpaare behalten ihren damaligen Feuermodus. Die eingefrorene Lieferung 0.53.0 bleibt unverändert.

Reproduzieren: zentraler `build.ps1 -Target All` mit den deklarierten Szeneneingaben; anschließend `scripts/Test-CampaignAndroid.py --apk APK --package ZIP --checkpoints build/dc15-trigger/complete/scenes/runtime-validation --output build/dc15-trigger/campaign-android --expect-semi-auto`. Gesamtnachweise liegen unter `build/dc15-trigger`.

Der NDK-Builder verwendet jetzt ein beschreibbares Compiler-Tempverzeichnis im Projekt. Das ursprüngliche Sandbox-Problem beim Verbinden zum Gradle-Prozess wurde durch Ausführung des lokalen Builds außerhalb der Sandbox gelöst; es wurde dafür keine Gradle-Prüfung entfernt.

## Nächster Handytest

Mit zusammengehörigem neuem APK und Szenenpaket eine neue Mission starten. Nach dem Intro einmal lange Feuer halten (ein Schuss), loslassen und erneut drücken (zweiter Schuss). Danach Energie entleeren und beim Wiederaufladen die Taste halten: kein verspäteter Schuss ohne erneuten Druck. Speichern/Fortsetzen sowie Kartenwechsel auf zusätzliche ungewollte Schüsse prüfen. Anschließend Intro → Bacta → Barrieren → Bunker/Tür und A/B/C bis zum Fabrikeinstieg frei auf Bild, Ton, Touch und Leistung abnehmen.

Dieser Gerätetest ist offen. Kompilierung und aufgezeichnete Eingaben ersetzen ihn nicht.
