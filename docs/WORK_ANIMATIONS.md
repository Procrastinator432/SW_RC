# Originale Arbeitsanimationen auf Karte B

Stand: 29.10.2026 · **0.84.0-work-animations / Code 88**.

Scorch und Fixer zeigen jetzt die Originalclips für Start, Arbeit und Abschluss der fünf autonomen B-Aufträge. Während dieser Posen wird die zusätzlich angehängte Squad-Waffe ausgeblendet. Bewegung, gespeicherter Arbeitsfortschritt, Missionsereignisse und Auftragsabschluss bleiben unverändert.

[APK für ARM64 und x86_64](../build/work-animations/android/artifacts/republic-commando-0.84.0-work-animations.apk) · [Neues Szenenpaket](../build/work-animations/package/mission1-scenes.rcpack.zip)

APK installieren und das neue Paket einmal importieren. Für die neuen Posen eine neue Mission starten oder aus A nach B wechseln. Bereits gespeicherte B-Stände verwenden weiterhin ihre bisherige Szene; ihre Bedienung und Darstellung werden durch den Import nicht umgeschrieben.

## Originaldaten und Zeitbindung

Alle neun Clips stammen aus `Clone.ukx` und werden mit der vorhandenen Clone-Hierarchie abgeglichen. Fixers sechs Hackclips werden an seinen vorhandenen Mesh angehängt; Scorchs bereits importierte Ladungsclips werden unverändert wiederverwendet. Materialien, Skins, Geometrie, Musik und Markerbindungen werden nicht ersetzt. A, C und Fabrikeinstieg bleiben bytegleich.

| Auftrag | Originalfamilie | Start | Abschluss | Schleifen im verbleibenden Arbeitsfenster |
|---|---|---:|---:|---:|
| Drei Sprengziele | `DemolitionSetCharge…` | 29/30 s | 39/30 s | 1 |
| Schildkonsole | `HackUnlockConStation…` | 42/30 s | 31/30 s | 6 |
| Sicherheitstür | `HackUnlockDoor…` | 30/30 s | 40/30 s | 2 |

Start und Abschluss behalten ihre Originaldauer. Dazwischen wird eine ganze Anzahl vollständiger Schleifen in die bestehende Arbeitszeit von drei bzw. sechs Sekunden eingepasst. Besonders die Ladungsschleife läuft dadurch schneller als der Originalclip. Dies ist eine ausdrückliche Anpassung an die bisherigen Port-Arbeitszeiten, keine Behauptung nativer Marker-/GoalQueue-Treue.

Die Pose wird direkt aus dem gespeicherten Fortschritt abgeleitet. Es gibt keine zusätzliche, unabhängig laufende Animationsuhr und keine neue Checkpointzustandsmaschine. Zeichnen verbraucht weder Zeit noch Ereignisse. Eine pausierte Arbeit behält ihre Pose, solange das einsatzbereite Mitglied ruhig am Marker steht. Geht es zurück zum Spieler, läuft oder fällt es aus, greift die vorhandene Darstellung; beim Fortsetzen wird aus dem erhaltenen Fortschritt weiter abgetastet.

Die Roottransformation ist für diese Darstellung gesperrt: Die physische Bewegung bleibt beim bereits geprüften Markerlauf. Die Spielerwaffe bleibt sichtbar; nur die zusätzliche Waffe des arbeitenden Squad-Mitglieds wird ausgeblendet.

## Prüfergebnisse

- Zentralcheck: je **1.572 Debug-/Release-Bibliothekstests**, Materialimport, Doctests, Clippy, Java und Buildvertrag bestanden; fünf neue Zeit-/Phasentests.
- Alle fünf Auftrags-Fixtures ergeben **1.584 exakt unveränderte Gameplayzustände** gegenüber dem Paket 0.83. Nach Abschluss sind auch die Renderbilder wieder identisch.
- **15 Originalposen** für Start/Schleife/Abschluss: sichtbarer Unterschied zur bisherigen Ruhepose, genau eine ausgeblendete Squad-Waffe, byte-/bildgleiche kalte Wiederherstellung und wiederholtes Zeichnen ohne Zustandsänderung.
- Native Schnittstelle: dieselben 15 Rust-Bilder pixelgleich geladen; **150 weitere Schritte und 30 kalte Wiederherstellungen** bestehen.
- Vollständige Mission mit **50.031 Eingaben und 1.232 Audioereignissen**, drei Übergängen und vier kalten Wiederherstellungen bestanden. ARM64-/x86_64-Build sowie ELF-/JNI-Verträge bestehen; der zentrale Paketbau ist bytegleich reproduziert.
- Android-Emulator: alle drei Arbeitsvarianten zeigen nach Prozessneustart bytegleiche Spielstände und pixelgleiche Pausenbilder; Fortsetzen erhöht den Arbeitsfortschritt.
- **36 ältere Checkpoints und neun Einstiegskamerabilder** sowie die gespeicherten autonomen 0.83-Aufträge mit 1.584 weiteren Schritten und 14 Restores bleiben unverändert.

[Gameplay-/Bildprüfung](../build/work-animations/behavior/validation.json) · [Originalclips](../build/work-animations/clips/validation.json) · [Native Posen](../build/work-animations/native-poses/validation.json) · [Gesamtnachweis](../build/work-animations/verification-summary.json)

Die Fixtures stammen aus den echten Vergabepunkten der geprüften 0.83-Spur und isolieren dort Kampf/Spawnzustand. Für den Vergleich werden sie ausdrücklich auf die neue, ansonsten unveränderte Szene gebunden. Unterstützte Beobachterpositionen werden nur für Bildaufnahmen gesetzt und ihre Blickwinkel mit dem normalen Controller ohne Zeitschritt normalisiert und vor den nächsten Vergleichsschritten zurückgenommen. Die Squad-Positionen werden dabei nicht gesetzt. Das ersetzt keinen frei gespielten Handyweg; der vollständige Missionslauf wird separat geprüft.

## Reproduzierbarer Bau

```powershell
./build.ps1 -Target WorkAnimations `
  -CampaignPackage build/marker-execution/package/mission1-scenes.rcpack.zip `
  -GameData '<eigener GameData-Ordner>' `
  -Python python -OutputDirectory build/work-animations
```

Der neue Exporter und Paketbuilder verwenden ausdrückliche Eingaben. Der zentrale Check baut auch den Animationsprüfer mit.

## Weitere Anschlüsse

Eigene Hack-/Ladungswerkzeuge, neue Animations-Notify-Geräusche und -Effekte sowie native Rootmotion-/Marker-/GoalQueue-Ausführung bleiben offen. Die heutigen Clips lösen keine zusätzlichen Missionsereignisse aus. Allgemeiner Recall, vollständige Gegner-/Squad-KI und freie Handy-, Hör- und Leistungsabnahme bleiben weitere Arbeit.
