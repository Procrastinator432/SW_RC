# Missionsinszenierung: Landungsbriefing und Verwundung

Aktuelle Lieferung: **0.31.0 Live-Abseilsequenz** in `analysis/mission13-rappel/delivery`. Der Außenblick aus0.30.0 ist durch den Spielerstandpunkt im Gunship und originale EntranceRope600-Bewegung ersetzt. [Neue Bedienung, Prüfungen und Grenzen](RAPPEL_RECONSTRUCTION.md). Die folgende Beschreibung dokumentiert den eingefrorenen0.30.0-Briefingstand.

Version0.30.0 / Code34, Lieferung `analysis/mission12-opening`: `republic-commando-0.30.0.apk` und `geo_01a-opening.rcplay` gemeinsam verwenden. Die RCPLAY13-Szene umschließt die unveränderte Squad-/Wiederbelebungsszene.

## Spielbarer Ablauf

Der Abschnitt beginnt mit einem Außenblick auf das originale Gunship. Die ersten Sekunden sperren Bewegung, Waffen, Springen und Ducken; Pause und Neustart bleiben verfügbar. Danach kehrt die Ansicht zur bestehenden Landeposition zurück. Das originale Auftragsbriefing läuft weiter, während der Spieler bereits schauen und laufen kann. Vor Abschluss des Briefings ist das Bacta-Ziel gesperrt.

Die ursprünglichen Funkzeilen erklären den Auftrag gegen Sun Fac. Anschließend löst das Script die Verwundung aus:175Schaden, sichtbarer Einschlag über den vorhandenen Explosionsadapter, kurzes Schadenssignal und15s Gesundheitswarnung. Die Bacta-Hinweise folgen. Nach rund32,08s ist der bisherige spielbare Bacta-Einstieg erreicht; die Ziele0–6 schließen sich unverändert an.

Auf Android die APK installieren, Szene öffnen und den Auftakt laufen lassen. Die Funktextanzeige verwendet die expliziten englischen Karten-Kommentare als lesbare Begleittexte; sie ist kein rekonstruierter lokalisierter Untertitelhost. Gleichzeitig abgespielte Zeilen bleiben zusammen sichtbar. Nach dem Briefing gelten dieselben Touch-Befehle wie bisher.

## Originalbindung

- Script12: ausgewählte levelstart-/Rappel-Zeitfolge, CA1G1701, GunshipDoorExplosion, EndBriefing und GunshipIdle. Die originale Folge1,5s/0,5s/4s bleibt erhalten.
- Script523: Advisor-Einweisung, Text-Comms-Prompt, CheckpointLevelStart, Auftragszeilen und StartBactaTraining. ACTION2502 wartet ausdrücklich nicht; sie startet gleichzeitig mit2503. Die beiden Audios werden getrennt ausgelöst.
- Script497: Bacta-Sperre, zufällige Wartezeit, Spielerexplosion/175Schaden, Gesundheitsblinken, Schmerzlaut, Advisor-Reaktion und Freigabe des Missionsziels. Originaler Zufallsbereich1–3s; der begrenzte Host verwendet deterministisch dessen Mittelpunkt2s.
- Script23/Actor580: originale cGEO01GunshipSet-Posen WarriorIntroGunshipBreathe, WarriorIntroGunship und WarriorIntroGunshipLoop. GunshipIdle startet den Übergang. Der bestehende Artilleriecontroller übernimmt später dieselbe Figur; es wird kein zweites Gunship erzeugt.
- Elf zusätzliche originale PCM-Spuren erweitern die Bank von65 auf76. Die bereits vorhandenen Bacta-Zeilen werden über ihre bestehenden Rollen0/1 wiederverwendet, statt sie doppelt abzuspielen.

Die anfänglichen200HP werden aus PlayerCommando.MaxHealth gelesen; nach175Schaden bleiben25. Die bestehende spätere Missions-MaxHealth100 bleibt erhalten. Damit ist der inszenierte Schaden korrekt ausgeführt, ohne die bereits akzeptierten Kampf-/Bacta-Abschnitte stillschweigend auf andere Gesundheitswerte umzustellen.

`opening-receipt.json` führt jede untersuchte Aktion samt Kartenwerten, ausgewählter Operation oder Auslassungsgrund auf. `gunship-bounds.json` bindet den eigenen Bildausschnitt: ein weltgeprüfter Außenblick auf die ursprüngliche Gunship-Geometrie. Dies ist eine Portkamera; die originale Abseilkamera wird damit nicht als nachgebildet ausgegeben.

## Prüfungen

- `playthrough/validation.json`:1.925 feste Ticks Auftakt, Verwundung beiTick1.397, Freigabe bei1.925;18.565 normale Eingaben bis zum ursprünglichen Turmausgang,20HP/acht Gesamtabschüsse. Keine Zuweisung von Position, Gesundheit oder Zielantworten im Testlauf.
- Alle ausgewählten Originalaktionen werden genau einmal ausgeführt. Nicht wartende Funkzeilen beginnen im selben Tick. Steuerungssperre, exakter Zustand-/Renderneustart und identische Auftaktereignisse bei144Hz geprüft.
- Windows-/Android-ABI prüfen zusätzlich200→25HP, gesperrte frühe Schüsse, elf neue Audioereignisse jeweils einmal und weiterhin den Scorch-Fokus-Kampflauf.
- APK-Dateidialog, sichtbarer Auftakt, GPU, Übergang zu Bacta, Touch-Schießen/Benutzen/Ducken, Audio und Pause auf API35-x86_64 geprüft. Bekannte und unbekannte Dateilänge funktionieren mit112MiB-Java-Heap.
- Vollständige Workspace-Prüfungen, Clippy, Format und Liefermanifest;0.29.1-Belege bleiben unverändert.

## Weitere Inszenierungsarbeit

Die vollständige Einleitung ist noch nicht rekonstruiert. Originale EntranceRope600-Spielerbewegung/Abseilkamera, Intro-Warrior, dekorative LZ-Trooper/Rückzug/Überflug und allgemeine SpawnLandingZone-Empfänger fehlen. Visor-Blur und vollständige Partikel-/Licht-/Gunship-Intro-Notify-Parität bleiben offen; Einschläge verwenden den vorhandenen Sprite-Adapter. Scriptseitige Inventar-, Kollisions-, Invulnerable- und zerstörte-Dekorationsfälle außerhalb der ausgewählten Figuren sind in der Aktionsliste ausgewiesen.

Nächste konkrete Inszenierungsarbeit: Originale Spieler-/Rope-/Warrior-Animationsbindung und Kamera nachverfolgen, anschließend die LZ-Statisten samt ihren Lebenszyklen integrieren. GEO_01B-Laden/Zustandsübernahme und vollständige Mission-/Touch-/physische ARM64-Abnahme bleiben weitere Voraussetzungen.
