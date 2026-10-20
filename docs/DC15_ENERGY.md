# DC-15s: Energie, Wiederaufladen und Speicherung – 0.53.0

Die fünf Arbeitsbereiche sind originale Waffenwerte, Ladungsverbrauch, Kühlung/Wiederaufladen, Anzeige/Bedienung und persistenter Zustand einschließlich Kartenübergaben. Neue Kampagnenszenen enthalten ein optionales DC-15s-Profil im Basismanifest. Alte Szenen ohne Profil behalten ihre bisherigen Magazin-/Reservewerte und Abläufe.

## Originaldaten und Rechnung

`rc-dc15-assets` und `rc-dc15-build` lesen die tatsächlich installierten Klassen-/Properties-Daten einschließlich Herkunft und Payloadposition. Die ausgewählten Werte sind:

| Eigenschaft | Originalwert |
| --- | --- |
| `DC15Ammo.MaxAmmo` | 5 |
| `RechargeRate` | 1,5 pro Sekunde |
| `CoolDownTime` | 0 Sekunden |
| `bWaitForRecharge` | false |
| `DC15Ammo.Damage` / `DC15Proj.Damage` | jeweils 55 |
| `DC15s.ShotDelay` | 0,2 Sekunden als ursprünglicher Float |
| `DC15s.bCanReload` | false |

`CTAmmoRecharging.PostBeginPlay` setzt die Ladung auf `MaxAmmo`; der geerbte Default `Charge=0` ist deshalb kein leerer Startzustand. `UseCharge(1)` zieht eine Einheit ab, begrenzt den Wert und aktiviert den Tick wieder. Unter einer Einheit verhindert `HasAmmo` das Feuern. Die verbleibende Ladung wird als Float gespeichert; eine ganzzahlige Anzeige entscheidet nicht über deren genaue Fortsetzung.

Der Port führt die ausgewählten Skriptzweige in Originalreihenfolge aus: zuerst `DC15Ammo.Tick`, dann `CTAmmoRecharging.Tick`. Kühlbeginn, `bNoRecharge`, `bCoolAndRecharge`, Wartefreigabe und Tickabschaltung werden berücksichtigt. Die Zeit läuft bei abgeschaltetem Ammo-Tick weiter; bei voller Ladung wird die Wiederaufladerechnung deaktiviert. Die tatsächlichen ausgewählten Defaults haben keine zusätzliche Kühlverzögerung und verlangen kein vollständiges Auffüllen vor dem nächsten Schuss. Es wird keine künstliche Überhitzungspause ergänzt.

Der importierte Basisschaden gilt im vorhandenen Trefferpfad auch für die ausgewählten Warrior-, Glas- und Karma-Schadensübergaben. Der Schussabstand ersetzt die bisher festen 0,15 Sekunden. Zielen, Halten der Feuertaste, Hitscan, Hitboxen und die bisherigen Gegner-/Schadensadapter bleiben eigene Grenzen: Dies ist keine vollständige Rekonstruktion von `Weapon.NormalFire`, Halbautomatik, Geschossflug oder sämtlichen DamageType-/Schwierigkeitsregeln.

## Anzeige und Bedienung

Das native HUD zeigt `DC-15s ENERGIE` in Prozent und bei weniger als einer verfügbaren Ladung `ENERGIE LAEDT`. Der Integer-Munitionswert der bestehenden Status-ABI bleibt die Zahl momentan verfügbarer ganzer Schüsse; Reserve und Reloadtimer sind bei dieser Waffe null. Manuelles und automatisches Magazinnachladen sind deaktiviert.

Die additive native Abfrage `rc_game_energy_status` liefert den Füllstand in Promille; alte Waffen liefern -1. JNI übergibt ihn an die produktive Android-Activity. Die bisherige Nachladefläche zeigt dort `E …%` als passive Energieanzeige, die Nachladetaste löst keine Aktion aus. Größe und Lage der übrigen Touchflächen bleiben bestehen; die Bedienbeschreibung wird beim Wechsel des Waffentyps angepasst.

## Spielstände und Kartenwechsel

Ladung, lokale Waffenzeit, Kühlbeginn, beide Freigabeflags und Tickaktivität gehören zum vollständigen Checkpoint. Beim Laden werden Modulpräsenz, Wertebereiche, Zeitfolge sowie die Übereinstimmung mit Integer-Munition, Reserve und Reloadtimer vor jeder Zustandsübernahme geprüft. Der Schusstimer muss zum Profil passen. Beschädigte Zustände ändern das laufende Spiel nicht.

Die Übergaben A→B→C→Fabrik übernehmen den genauen Energiezustand und den verbleibenden Schussabstand. Dadurch entsteht am Kartenwechsel weder eine kostenlose volle Batterie noch ein zurückgesetzter Schussabstand. Historische Übergaben ohne Energiezustand können in eine neue Szene übernommen werden: ihre sofort verfügbaren Schüsse werden auf die neue Kapazität begrenzt, danach gilt die automatische Wiederaufladung. Eine neue Energieübergabe an eine alte Zielszene wird als unpassendes Szenenpaket abgewiesen. Archivierte alte Szene-/Checkpointpaare bleiben unverändert verwendbar.

## Aufzeichnungen und Prüfungen

Die bisherigen Aufzeichnungen wurden für 20 Magazin-/200 Reserveschüsse und kürzere Feuerintervalle erstellt. `rc-dc15-replay-build` erzeugt reguläre Blick-/Feuereingaben für sichtbare Gegner an den vorhandenen stehenden Kampfstellen und kehrt anschließend zur aufgezeichneten Blickrichtung zurück. Für B/C liefert ein Gegenlauf mit der bisherigen Magazinwaffe ausschließlich Routen- und Blickziele; begrenzte Bewegungseingaben gleichen Abweichungen von diesen Zielen aus. Benutzen- und Granateneingaben bleiben erhalten. Eingefügte Kampfzeit verändert den Zeitpunkt der Barrierenfreigabe; der anschließende Barrierenbuilder gleicht den Weg durch gewöhnliche Steuerungseingaben ab. Bei einem bereits nahen, frei erreichbaren Anschlussziel geht er direkt dorthin, statt unnötig die gesamte Zwischenroute erneut abzulaufen.

Jede vorbereitete Energieaufzeichnung wird unabhängig von Beginn an erneut gespielt und mit ihrem vollständigen Zustand verglichen. Ihr `preparation.json` unterscheidet ausdrücklich Vorbereitung und erreichten Missionsabschluss. Gesundheit, Position, Energie, Inventar, Gegner und Ziele werden bei der Generierung nicht gesetzt. Die vollständige Missionsabnahme erfolgt danach im zentralen Checkpoint- und Präsentationslauf.

B und C beginnen über echte abgeschlossene Vorläuferaufzeichnungen und die produktive Kartenübergabe. Ein isolierter Szenenstart ist kein Ersatz für diesen Einstieg: Er enthält beispielsweise die getragenen Squadmitglieder noch nicht. Für B erhält der Gegenlauf zur Routenführung dieselbe Übergabe ohne Energiemodul, mit den bisherigen Magazin-/Reserve-Startwerten. Für C benötigt der alte Eingabelauf seinen damaligen Eintrittszustand: Schon eine andere Eingangsgesundheit verändert die Dauer und damit die Bewegung während des Heilens. Deshalb bindet `map_c_reference_handoff` eine versionierte, per SHA-256 fixierte Übergabe aus dem bestandenen 0.52.0-Lauf ein; ihre Herkunft steht in `fixtures/dc15/map-c-legacy-entry.provenance.json`. Das beeinflusst ausschließlich den Gegenlauf. Die ausgelieferte Aufzeichnung verwendet den tatsächlich erreichten neuen A/B-Zustand und wird nochmals mit dieser unveränderten Übergabe abgespielt.

Acht neue Rustprüfungen decken Entleerung, Bruchteile der Ladung, Kühlzeit/Zweigreihenfolge, Schaden/Schussabstand, ignorierte Reload-Eingaben, Aufladung und Schussfrist während gesperrter Intros, exaktes Fortsetzen mit gepufferten Eingaben, Neustart, Übergabe und sechs korrekt neu gehashte beschädigte Zustände ab.

Der zusätzliche Desktop-GPU-Parserlauf verwendet bei 96 MiB ausdrücklich SerialGC. Mit dem JVM-Standardkollektor trat beim großen Weltpaket einmal ein Heapfehler auf; ein unveränderter Wiederholungslauf bestand. Die feste Kollektorwahl macht die Testbedingung explizit. Parservertrag, alle 39 Frames und das Weltpaket bestehen damit bei derselben Heapgrenze. Androids ART und die tatsächliche GLES-Ausführung werden separat geprüft.

```powershell
./build.ps1 -Target All -SceneInputs local-assets/mission1/scene-inputs.json `
  -GameData 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' `
  -OutputDirectory build/dc15-energy/complete
```

Die Szenenpipeline bindet das Profil für A/B/C und den Fabrikeinstieg. Für die neue Logik das zusammengehörige APK und Szenenpaket verwenden, importieren und eine neue Mission starten. Ein Emulatorlauf belegt keine subjektive Hör-/Touch-Abnahme oder physische ARM64-Ausführung.

## Abschlussnachweise

Der zentrale Offline-Gesamtbau besteht mit je **1.175 Bibliothekstests** in Debug und Release, Clippy, Java-Prüfungen und beiden Android-Architekturen. **30 Szenenstufen** erzeugen A/B/C und den Fabrikeinstieg. **25 Checkpointproben** prüfen die vollständige Kampagne einschließlich Übergaben, atomarer Ablehnung beschädigter Daten, Neustart und exakter Fortsetzung. Die neuen Aufzeichnungen erreichen A nach 19.319 Eingaben mit 15 HP, B nach 17.843 mit 20 HP und C nach 12.707 mit 80 HP. C verwendet den tatsächlichen Energie-/Squadzustand aus B; die alte fixierte Eintrittsübergabe dient ausschließlich der Routenreferenz.

Die Präsentationsprüfung besteht mit **39 Spieleransichten**, neun exakten Checkpoint-Fortsetzungen und drei zurückgewiesenen beschädigten Körperzuständen. Der echte Java-GPU-Parser liest alle Frames und das 43.475.920-Byte-Weltpaket unter 96 MiB Heap. Die neue Prozentanzeige ist auch im kleinen nativen Bitmapzeichensatz vollständig sichtbar.

Android x86_64 erreicht A→B nach **19319 Eingaben** mit 15 HP, acht exakt neu geladenen Zwischenständen und 60 weiteren B-Schritten. Die native Energieabfrage erreicht tatsächlich null Promille; die Reserve bleibt null. Acht produktive JNI-/GLES-/SoundPool-Abschnitte liefern **16 echte PNGs**, 37 Einmalwiedergaben und 13 Ambient-Streamstarts. Das normale APK besteht Paketimport, Fortsetzen nach Hintergrundkill, 640×360-dp-Steuerung, gespeicherte Empfindlichkeit, passive Energiefläche/ignorierten Reload sowie alle drei automatischen Übergaben einschließlich exakten Energie- und Schussfristzustands. Für die Übergänge verwendet der UI-Test echte Replay-Endspielstände; dies ist kein vollständiger manueller Touch-Durchlauf.

Bacta-Abbruch/erneutes Benutzen, Notifies, Partikel und Dauerton bestehen mit der endgültigen DLL. 41 Cue-Referenzen auf 29 Originalcues und die 7.610.358-Byte-Soundbank bleiben geprüft. Am endgültigen ZIP bestehen Gegner-PCM, Rampenalpha und vier historische Restores. Vier zusätzliche 0.52.0-Szene-/Checkpointpaare werden unverändert wiederhergestellt.

Die zusammengehörige Lieferung liegt unter `build/releases/0.53.0`:

- `republic-commando-0.53.0-dc15-energy.apk`, SHA-256 `da2165ef238a629fab4a4165a8ec7bd46d56779abc3c7b72b4e877d50c940b10`.
- `mission1-scenes.rcpack.zip`, SHA-256 `de109c850fda038bb7f6ea1e63679a1de924de3f53b4dda721426ee8f921a95a`.
- `SHA256SUMS.txt`, `verification-summary.json` und `source-and-delivery.json` ordnen Dateien, Prüfungen und Quellstand zu. Das Liefer-APK enthält beide ABIs und keine optionale Prüf-Activity; seine Einträge stimmen mit dem normalen Neubau nach der Android-Probe überein.

Maßgebliche Belege: `build/dc15-energy/complete/validation.json`, `complete/scenes/runtime-validation/validation.json`, `complete/scenes/player-presentation/validation.json`, `android/validation.json`, `campaign-android/validation.json`, `legacy-weapon.json` und `verification-summary.json`. Das Gesamtprotokoll nennt den Ausgangscommit und den noch uncommitteten Buildstand; die abschließende Zuordnung steht separat in `source-and-delivery.json`.
