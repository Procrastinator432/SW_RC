# Gegner und Scorch im spielbaren geo_01a-Abschnitt

Aktualisierung0.29.1: Die fortlaufende Gesundheitsrampe und der Animationsabschluss beim Wiederbeleben sind inzwischen integriert. Aktuelle Lieferung: `analysis/mission11-revival`; [Nachweis und verbleibende Grenzen](REVIVAL_PARITY.md). Die folgenden Liefernachweise beschreiben den eingefrorenen Stand0.29.0.

Lieferung 0.29.0 / Code32: `analysis/mission10-squad/republic-commando-0.29.0.apk` und `geo_01a-squad.rcplay` gehören zusammen. Die Szene enthält lokal abgeleitete Originalassets und umschließt die unveränderte RCPLAY11-Szene. Die Ziele0–6 bleiben spielbar; GEO_01B wird weiterhin nur als Reiseanforderung ausgelöst.

## Bedienung

Nach Scorchs Beitritt bleibt der Sprengauftrag vorrangig. Nach der Bunkerexplosion:

- Scorch folgt standardmäßig. Aus der Nähe Scorch anvisieren und **BENUTZEN kurz drücken**, um zwischen Halten und Folgen umzuschalten.
- Einen lebenden Warrior anvisieren und **BENUTZEN kurz drücken**: Fokusfeuer. Scorch priorisiert ihn und kann dessen zuletzt bekannte Position kurz verfolgen.
- Bei kampfunfähigem Scorch in seine Nähe gehen und **BENUTZEN halten**. Loslassen unterbricht den Fortschritt. Nach2,7s beginnt seine Aufstehanimation.
- Bei eigener Kampfunfähigkeit **BENUTZEN drücken**: Scorch um Hilfe rufen. Er nähert sich, bekämpft unmittelbare sichtbare Gefahren und belebt anschließend wieder. Bewegung und Schießen bleiben während der Kampfunfähigkeit gesperrt.
- Sind beide kampfunfähig, endet der Versuch. Vor Scorchs Beitritt bleibt die bisherige Niederlageregel bestehen.

Der HUD zeigt Scorchs tatsächliche Gesundheit, Befehl, Kampfunfähigkeit und Wiederbelebungsfortschritt. Ein Neustart setzt auch Befehle, Projektile, Verfolgung und Wiederbelebung zurück.

## Originaldaten und ausgewählte Umsetzung

Warriors verwenden die originalen Angriffsprioritäten, bedingten Zufallsziehungen, MeleeAttack-/Lunge-Notifies, Sprungpunkte, AirSpeed2000 und unskalierte Root-Kurven. Ihr Körper bleibt50/50/115. Ausfallschritte, offensiver Sprung, defensiver Sprung, Bodenbewegung und Trefferunterbrechung sind an den Missionscontroller angeschlossen. Die Originalregeln bestimmen Entscheidungen und Trefferzeitpunkte; die Ausführung verwendet einen begrenzten Rust-Host.

Scorch erhält originale CloneCommando-Posen für Feuer, Ducken, Kampfunfähigkeit, Helfen und Aufstehen, seine gelben Originalmaterialien sowie die originale DC17m-Attachment-Geometrie am weaponAttach_R-Knochen. DC17-Projektile verwenden Damage20, Speed6000, LifeSpan10 und ShotDelay0,1; Welt und Spieler können die Schussbahn blockieren. Ein zusätzlicher originaler DC17-Schusssound erweitert die Bank auf65 Rollen. Squad-Abschüsse lösen die Warrior-Todereignisse aus; die native Gesamtabschussanzeige zählt beide Beteiligten.

Die Gehverbindungen stammen aus geo_01a PathNode.PathList/ReachSpec. Die Begleiterprüfung verwendet den vorhandenen Körperbewegungssolver in16-Einheiten-Schritten mit35-Einheiten-Stufen/Bodenunterstützung. Stehen40/40/84, Ducken40/40/56, GroundSpeed450 und CrouchSpeedRatio0,5. Zusätzlich helfen tatsächlich durchlaufene Spieler-Wegpunkte; jeder gewählte Abschnitt wird erneut gegen Boden und Weltkörper geprüft. Keine Versetzung durch Wände. Gültige Umgehungswege werden bis zum Ziel beibehalten, statt durch ständig wechselnde Anschlüsse zu pendeln.

Scorchs eigene DV_OrganicCTSquadDamage-Liste wird aus dem bereits gebundenen Original-Schadensreport geladen. Schildregeneration:15/s nach4s. Die extrahierten Klassenvorgaben sind Health100/MaxHealth400 und Shields100/MaxShields50. Wiederbelebung verwendet MarkerReviveOther.Duration2,7, Health1 beim Beginn des Aufstehens und MaxHealth/2 nach dessen Ende; Scorch erhält200HP. Der Spieler verwendet die bestehende Missions-MaxHealth100 und erhält50HP.

## Nachweise

- `playthrough/validation.json`: zusammenhängende normale Eingaben über Ziele0–6, acht Gesamtabschüsse, Originalausgang und exakter Zustand-/Render-/Kollisionsneustart.
- `live/validation.json`: ausschließlich normale Eingaben bis zum Feindkontakt, Fokusbefehl, danach Gegenfeuer einstellen; Scorch muss selbst einen originalen Fabrik-Warrior erreichen, beschießen und töten. Keine Zustandszuweisung.
- `behavior/validation.json`: ausdrücklich isolierte Originalgeometrie-Fixtures für Befehlsflanken, Halten/Folgen, abgebrochene und gegenseitige Wiederbelebung, gesperrte Eingaben, gemeinsame Niederlage, Folgebewegung, Zielwahl und Projektilkampf. Positionen/Gesundheit werden hier als Testaufbau gesetzt; das ist kein Missionsdurchlauf.
- `motion/validation.json`: beide originalgroßen Ausfallschritte und originaler JumpDest mit TakeOff/FallBreathe/Land auf Originalgeometrie, einschließlich Bodenkontakt.
- `native/validation.json`, `android/native-replay.json`: derselbe vollständige Eingabelauf und Scorch-Kampflauf über Windows-/Android-ABI, geschützte Ausgabegrenzen, Soundereignisse und Neustart.
- `android/validation.json`: APK-Dateidialog, GPU, Touch-Schießen/Benutzen/Ducken, Pause und Audio auf dem vorhandenen API35-x86_64-Emulator. Kein vollständiger manueller Touch-Durchlauf.
- `validation-manifest.json`: Lieferung, Quellenstand und Belege gebunden; vorherige Lieferung unverändert.

## Verbleibende Grenzen

Diese Lieferung vervollständigt den ausgewählten Gegner-/Scorch-Spielablauf, nicht die gesamte Original-KI. Wahrnehmung, letzte Sichtposition3s, Zufallsstartwerte, Drehung, Zielwahl, statische Wegprüfung, Fokus-/Folge-/Heilplanung und ballistische Ausführung sind explizite Portregeln. Allgemeine CTBot/GoalQueue-/Perch-/Flug-/Deckungs-/Granatenplanung, vollständige vierköpfige Squad-KI und dynamische Pawn-Ausweichkollision sind nicht rekonstruiert. Scorchs Projektile und Explosionsschaden verwenden begrenzte Trefferhosts; vollständige Game.ReduceDamage-/Team-/FriendlyFire-Parität bleibt offen. Die originale fortlaufende Gesundheitsrampe während Pawn.Reviving ist noch nicht nachgebildet. Bei sofortigem Erreichen des Abschnittsendes kann Scorch hinter dem Spieler bleiben; die Reiseanforderung wartet wie bisher auf keinen allgemeinen Squad-Travel-Host.

Vollständige Einleitung/Inszenierung, GEO_01B-Laden mit Zustandsübernahme und die Gesamtmission-Abnahme bleiben nächste Meilensteine. Die neue Version benötigt noch den vollständigen manuellen Touch-Lauf und die physische ARM64-Abnahme.
