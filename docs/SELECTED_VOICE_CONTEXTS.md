# Vollständige Sprachzuordnung im ausgewählten A-Ablauf

Stand: 28.10.2026 · Neues Szenenpaket für die **unveränderte APK 0.75.0-dialog-routing**.

Die elf bisher offenen Sprachzuordnungen auf GEO_01A sind für den vorhandenen Missionshost ergänzt. Alle **42 Sprachclips seiner Soundbank** besitzen jetzt eine Kategorie und einen belegten Besitzer. Sieben dieser Clips gehören Scorch; seine zuvor getrennt ausgegebenen Zeilen verwenden nun dieselbe Sprachidentität und damit die vorhandene Ersetzungsregel.

[Neues Szenenpaket](../build/voice-context/package/mission1-scenes.rcpack.zip) · [APK 0.75.0](../build/rappel-dialog/android/artifacts/republic-commando-0.75.0-dialog-routing.apk) · [Gesamtnachweis](../build/voice-context/verification-summary.json)

APK 0.75 kann installiert bleiben. Das neue Paket importieren und **Neue Mission starten** wählen. Es enthält weiterhin die vorherigen Materialkorrekturen. Alte Checkpoints behalten ihre ursprünglichen Szenen; sie erhalten die neuen Tonbindungen nicht durch Fortsetzen.

## Was geklärt wurde

| Bisher offene Bindung | Jetzt übernommener Kontext |
|---|---|
| Sieben Scorch-Zeilen | AudioParams.ActorTag `Commando62Delta` → ursprüngliche PawnFactory 457, Prototype `CTCharacters.Commando62Delta`, PawnTagFromFactory `Commando62Delta`, Capacity 1 |
| CA1G1760 | Der vorhandene Granatenhost führt Aktion 2956 / Skript 745 aus; der ebenfalls vorhandene Original-Reminder 2984 / Skript 748 gehört nicht zu diesem Host |
| CA1G1763 | Der Rendezvoushost enthält Aktion 3541 / Skript 1128; Aktion 3012 ist im Original deaktiviert und gehört nicht zum geladenen Ablauf |
| CT3R1002 | Introaktion 1260 spielt die Zeile auf Trooper 452; andere Originalverwendungen bleiben im Audit sichtbar |
| GWRX7119 | Sound-Notify 191 aus Geonosian.ukx wird vom ausgewählten Intro-Warrior 530 ausgeführt |

Scorch wird erst während des Spiels erzeugt. Deshalb besitzt er keinen Exportindex eines platzierten Kartenakteurs. Der Adapter verwendet eine stabile eigene Kennung `0x40000000 | 457` (= 1073742281). **Diese Zahl ist eine Portkennung**, kein rekonstruierter ursprünglicher UObject-Zeiger. Der Builder prüft Factoryklasse, Prototyp, Tag und Kapazität gegen die Originaldatei. Die Kennung beschreibt den einen persistenten Begleiter des vorhandenen Hosts; sie modelliert keine allgemeine Factory mit mehreren Lebenszyklen oder Generationen.

Die Prüfung unterscheidet jetzt sämtliche ursprünglichen Verwendungen eines Clips von den tatsächlich geladenen Aktionskontexten. Bei den ausgewählten Skriptaktionen müssen Soundreferenz und Aktivierung mit den Originaldaten übereinstimmen. Der Audit behält auch andere Verwendungen, deaktivierte Aktionen und nicht aufgelöste ungewählte Figuren bei. So wird eine Zuordnung aus dem ausgewählten Host nicht als allgemeine Original-VM-Regel ausgegeben.

Für die Warrior-Stimme wurde zusätzlich `UAnimNotify_Sound::Notify` (10502d20) bis `UAudioSubsystem::CheckHearSound` (103d6640) untersucht: Der Actor wird weitergegeben; der direkte PlayUSound-Aufruf verwendet Flag 0. Dies ist ein neuer Decompilerbefund, kein zusätzlicher ausgeführter Originalfunktionsvergleich.

Der Spielcode und der Android-Audiocode sind gegenüber 0.75 unverändert. Das Paket ergänzt elf Tonbindungen und deren Herkunftsnachweise. Geometrie, Texturen, WAV-/Musikbytes, Skriptzeiten und alle inneren Szenenschichten bleiben erhalten; B, C und die Fabrik sind vollständig bytegleich.

## Nachweise

- **1.519 Debug- und 1.519 Release-Bibliothekstests**, Materialimport, Doctests, Clippy, Inspectorbuild, Java und Buildvertrag bestehen über den Zentralcheck.
- **19.319 echte A-Replayeingaben**, 81 identische Spielzustände und elf korrigierte ausgegebene Sprachereignisse. Alle sieben Scorch-Clips wurden dabei tatsächlich erreicht.
- Gezielte OpenAL-Prüfung mit diesen echten Scorch-Ereignissen: Eine neue ungeschützte Zeile desselben Besitzers ersetzt die alte; nach einem nativen Restore gilt dieselbe Identität. Ein anderer Besitzer bleibt unabhängig. Die gezielten Überlagerungen normalisieren Ereignisnummer/Tick und sind kontrollierte Audioeingaben.
- Aufnahme nach **11.270 gewöhnlichen Eingaben** mitten in Scorchs Satz, separates natives Restore sowie Java-Speichern und -Laden durch die produktiven Checkpointklassen bestehen. Sprachbesitzer und Sampleposition bleiben erhalten.
- Vollständiger A/B/C→Fabrik-Lauf: **49.795 Eingaben**, 1.230 WAV-Ereignisse, drei Übergänge und vier kalte Wiederherstellungen durch C-ABI/OpenAL.
- Erneute Paketkorrektur ist bytegleich, einschließlich ZIP. Die bereits geprüfte APK wird unverändert wiederverwendet.

[Zentralcheck](../build/voice-context/check/validation.json) · [A-Zustandsvergleich](../build/voice-context/routing.json) · [Scorch-Sprachbesitz](../build/voice-context/scorch-context.json) · [Getrennte native/JVM-Prozesse](../build/voice-context/scorch-process/audio-checkpoint.json) · [Mission](../build/voice-context/campaign/mission-audio.json) · [Originalquellen und Aktionskontexte](../build/voice-context/package/voice-owners.json) · [Android-Laufzeit](../build/voice-context/android-runtime-final/validation.json)

Die Android-Prüfung importiert das Paket über den produktiven Dateidialog. Für Scorch verwendet sie den tatsächlich aufgenommenen Windows-Spiel-/Audiostand und die produktive Java-Speicherhülle; die native Android-Ausgabe muss die Stimme laden, nach Pause/Prozessverlust erneut laden und ihre Sampleposition weiterführen. B/C-Musik und Fortsetzen werden anschließend mit echten Replaycheckpoints geprüft. Das ist kein freier Android-Durchlauf und kein Hörtest.

Der erste Scorch-UI-Versuch lief länger als die verbleibende Satzdauer und scheiterte deshalb am erwarteten noch aktiven Dialog. Das Native-Protokoll zeigte beide Restores und den natürlichen Abschluss. Der Prüfer übernimmt nun die beobachteten Buttonkoordinaten vor dem Start und pausiert nach einem kurzen Intervall, ohne während des Satzes zeitaufwendige UI-Baumabfragen auszuführen. Dafür wurde kein Produktverhalten geändert.

## Weiter offen

**42 zugeordnete Clips bedeuten keine vollständige Original-Audioimplementierung.** Verschiedene ursprüngliche Besitzer können weiterhin gleichzeitig sprechen. Der genaue schnelle Handy-Spielerweg ist nicht reproduziert; die gesamte gemeldete Überlagerung ist noch nicht frei abgenommen. Räumliche Originalmischung, nicht portierte Sprach-/Reminderprogramme und die vollständige Musikzustandsmaschine bleiben getrennte Arbeit. Die anfänglich vermisste Musik auf A ist weiterhin offen.

Bei der Abseilstelle wurden zusätzlich die ursprünglichen Briefing-Defaults geprüft: `bSetBriefingWeapon=false`, `BriefingWeapon=""`. Es ist damit keine besondere Handwaffe belegt. Das eigene Körpermodell hat `bOwnerNoSee=true`; das Seilobjekt nennt `CloneEntrance1760`, der Spieler `EntranceRope600`. **Ego-Hände/Rüstung und der aktive Seilreceiver sind weiterhin nicht korrigiert.**

[Briefing-, Seil- und Factorywerte](../build/voice-context/research/original-receivers.json) · [Bisherige Abseilbefunde](RAPPEL_RECONSTRUCTION.md)

## Reproduzieren

```powershell
cargo build --release --offline --locked -p rc-inspect --bin rc-mission-voice-audit
python scripts/Build-MissionVoiceRouting.py --builder target/release/rc-mission-voice-audit.exe --game-data "D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData" --package build/rappel-dialog/package/mission1-scenes.rcpack.zip --output build/voice-context/package
./build.ps1 -Target Check -OutputDirectory build/voice-context/check
```

Die Quelldaten und Paketargumente sind explizit. Frühere Lieferungen bleiben erhalten. Kein Commit, Tag oder Push wurde erzeugt.
