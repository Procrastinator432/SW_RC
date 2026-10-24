# Originale Arbeitsgeräusche und Sprengladungen in GEO_01B

Stand: 29.10.2026 · **0.85.0-work-equipment**, Android-Code 89.

Scorchs drei Sprengaufträge zeigen jetzt die originale Ladung an der Hand, anschließend am Arbeitsmarker mit inaktiver und scharfgeschalteter Originaltextur. Nach der tatsächlichen Zündung verschwindet sie. Die fünf B-Aufträge spielen die belegten Tastengeräusche ihrer Arbeitsclips. Pause und Prozessverlust erhalten den Fortschritt, ohne bereits ausgelöste Geräusche erneut zu erzeugen.

## Lieferung

- APK: [0.85.0](../build/work-equipment/android/artifacts/republic-commando-0.85.0-work-equipment.apk), ARM64 und x86-64.
- Szenenpaket: [mission1-scenes.rcpack.zip](../build/work-equipment/package/mission1-scenes.rcpack.zip).
- [Prüfzusammenfassung](../build/work-equipment/verification-summary.json) und [gesicherter Quellstand](../build/work-equipment/source-and-verification.json).

**APK aktualisieren und das neue Paket einmal importieren.** Für die neuen B-Bindungen frisch beginnen oder aus A nach B wechseln. Bereits gespeicherte B-Szenen behalten ihre bisherigen Daten und damit ihre bisherige Darstellung und Geräuschzuordnung. Die Speicherung wird nicht auf ein anderes Szenenpaket umgeschrieben.

## Originalbefunde und Umsetzung

`rc-work-notify-audit` liest die neun ausgewählten Originalclips aus `Animations/Clone.ukx`. Ihre 17 Notifies enthalten 16 Geräuschaufrufe und den Trigger `MDplaceCharge`. Die Klassenbefunde aus `System/ctmarkers.u` und `System/ctinventory.u` binden die Sprengladung an `CTInventory.DemolitionChargeObject` mit dem Mesh `MarkerIcons.Demolition.DemolitionCharge`. [Notify-Befunde](../build/work-equipment/original-notifies.json) · [Klassenbefunde](../build/work-equipment/work-classes.json).

Die Zeitpunkte werden auf die bereits vorhandenen Start-/Loop-/Stop-Abschnitte aus 0.84 abgebildet. Deren Gesamtdauer und ganze Schleifenzahl bleiben erhalten. Daraus ergeben sich **5, 20, 11, 5 und 5 Geräuschereignisse** in den fünf Aufträgen. Ein Fortschrittsschritt löst alle überschrittenen Zeitpunkte genau einmal aus; es gibt keinen zusätzlichen gespeicherten Audiocursor. Unveränderter oder rückläufiger Fortschritt erzeugt nichts. Renderabfragen lösen keine Geräusche aus.

| Originalbindung | Gewählter Originalclip |
|---|---|
| `GEN_Sound.Interface.buttons_01` | `GEN_Sound.Interface.int_GEN_button_03` |
| `GEN_Sound.Interface.buttons_02` | `GEN_Sound.Interface.int_GEN_button_02` |
| `GEN_Sound.Interface.int_GEN_button_06` | derselbe Clip |

Bei SoundMultiple wird ausdrücklich die erste Originalvariante verwendet; die Parameterwahl benutzt einen lokalen festen Seed. Das ersetzt noch nicht den ursprünglichen globalen Zufallsablauf. Die tatsächlich aufgelösten Parameter sind Lautstärke 1, Pitch 1, Radius 500, Kategorie SFX; der Parameterdatensatz ist `NonPositional`. Der bestehende Missions-SFX-Pfad gibt sie hörerbezogen aus, mit einer spielseitigen Reichweitenprüfung von 500. Allgemeine räumliche Emitter-/Verdeckungs- und Aktivierungsparität wird damit nicht behauptet. Die Parameter gelangen über den nativen Audioabruf bis zum Mixer. [Exportnachweis](../build/work-equipment/assets/validation.json).

Die Platzierung folgt dem belegten DemoStart-Notify bei **0,606333327 Sekunden** Arbeitsfortschritt. Vorher hängt die inaktive Ladung an `weaponAttach_L` der ausgewerteten Originalpose; danach sitzt sie am Originalmarker. Nach Scharfschaltung erscheint `MarkerIconTextures.Demolition.DemolitionCharge_Active`, davor `_Inactive`. Nach Zündung wird sie entfernt. Die drei ursprünglich ständig sichtbaren Ladungen mit insgesamt **2.052 Renderdreiecken** werden anhand ihrer tatsächlichen Geometrie zugeordnet und durch diese zustandsabhängige Darstellung ersetzt. Kollision, Arbeitsdauer, Schaden, Ziele und Kartenwechsel bleiben erhalten. `MDplaceCharge` wird hier als belegte Darstellungsgrenze verwendet, nicht als neu implementierter allgemeiner Originalskript-Trigger.

Die ausgewählten Konsolen-/Türclips zeigen weiterhin ihre Originalhandbewegungen und erhalten ihre Originalgeräusche. Diese Notify-Bindungen belegen kein zusätzliches Hackwerkzeug und keinen Partikeleffekt; dafür wurde kein Ersatzmodell erfunden. Das ist eine Aussage über die untersuchten Bindungen, kein vollständiger Negativbefund für sämtliche Spielressourcen.

## Prüfungen

- Zentraler Check: je **1.578 Bibliothekstests** in Debug und Release, Formatierung, Clippy, Java- und Buildvertrag; beide Android-ABIs gebaut und ELF-/APK-Struktur geprüft.
- Alle fünf Aufträge: tatsächliches Pausieren/Fortsetzen per BENUTZEN, 46 Geräusche mit erwarteter Reihenfolge, keine Wiederholung während Pause; Gameplayvergleich zu 0.84 mit ausschließlich Audiofeldern ausgenommen. Alle drei Ladungen durchlaufen Hand, Platzierung, Scharfschaltung und Entfernung nach echtem FEUER.
- Native Schnittstelle: **17 gespeicherte Ansichten**, exakte kalte Zustands-/Bildwiederherstellung, **1.200 Fortsetzungsschritte und 16 weitere kalte Restores**; 41 verbleibende Geräusche einschließlich Kategorie, Lautstärke, Pitch und Radius geprüft.
- Vollständiger Missionslauf: **50.031 Eingaben, 1.278 Audioereignisse, drei Kartenwechsel und vier kalte Restores** über Windows-C-ABI und OpenAL. 36 ältere Checkpoints und neun ältere Einstiegsbilder bleiben exakt; zusätzlich 15 Arbeitsposen aus 0.84 mit 150 Schritten und 30 Restores geprüft.
- Android: produktiver Paketimport, Hand-/platzierte-/scharfe Ladung und unterbrochene Konsole nach Prozessverlust; die vier pausierten Ansichten werden pixelgleich wiederhergestellt. Ein kurzer Touch-Befehl setzt den Konsolenauftrag fort. B-/C-Musik und deren kalte Fortsetzung werden separat geprüft.

Die beobachteten Testkameras stehen auf unterstützten, kollisionsgeprüften Spielerpositionen; Squadpositionen werden nicht zugewiesen. Die isolierten Auftragsfälle begrenzen Kampfzustände. Sie ergänzen den vollständigen Missionslauf und ersetzen keine freie Spiel- und Hörabnahme auf dem physischen Handy.

## Reproduktion

```powershell
./build.ps1 -Target WorkEquipment `
  -CampaignPackage build/work-animations/package/mission1-scenes.rcpack.zip `
  -MarkerInventory local-assets/mission1/assets/map_b_inventory.json `
  -GameData 'D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData' `
  -Python python -OutputDirectory build/work-equipment-rebuild
```

Der zentrale Builder exportiert Originalclips und Notify-/Klassenbefunde, bindet Ladungsmodelle und Geräusche und erstellt das Paket. Alle übrigen Karten, Musik und die eingebettete bisherige B-Basis bleiben bytegleich; die neue Metadatenbindung steuert den Austausch der drei Ladungen zur Laufzeit. Die Prüfprogramme sind `rc-work-equipment-check`, `scripts/Test-WorkEquipment.py`, `scripts/Test-MissionNativeAudio.py` und `scripts/Test-MusicAndroid.py --work-equipment-checkpoints …`.

## Nächste offene Arbeiten

Für weitere Originaltreue folgen allgemeine CTBot-/GoalQueue-Ausführung und die zugehörigen ursprünglichen Marker-/Unterbrechungsregeln. Die Arbeitsloops sind weiterhin auf die vorhandenen Port-Arbeitszeiten abgestimmt; native Zeitskalen, globale SoundMultiple-Auswahl und allgemeine Effekt-/Skriptaktivierung sind nicht vollständig rekonstruiert. Außerdem bleiben die vollständige Ego-/Receiverbindung und die freie Handyabnahme von Bild, Klang, Sprachüberlagerungen, Bedienung und Leistung offen.
