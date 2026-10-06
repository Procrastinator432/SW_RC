# Nativer freier Flug als isolierter Rechenteil

`falling::free_fall` rekonstruiert den kollisionsfreien Rechenteil von APawn::physFalling10496dc0. Ghidra-Exporte: `analysis/decompiled/pawn-falling.c` und `pawn-falling.asm`. Das ist die Pawn-Funktion; die früher zusätzlich exportierte AActor::physFalling10490a30 ist eine andere Implementierung.

## Regeln aus dem Original

Die ursprüngliche Beschleunigung wird gesichert und am Ende wiederhergestellt. Für den Flug wird Beschleunigung in Z zunächst entfernt. AirControl liegt bei Pawn+0x544, AccelRate bei+0x53c, GroundSpeed bei+0x528.

Bei AirControl größer als der ursprüngliche float-Wert0,05 wird eine horizontale Vorausprobe ausgeführt. Die geplante XY-Verschiebung ist `(old_velocity + normalized_acceleration * AccelRate * AirControl) * full_dt`; Z bleibt unverändert. Der native Aufruf nutzt Traceflags0x286 und Actor-Maße bei+0x1f8/+0x1fc. Ein Actor-Treffer schaltet den wirksamen AirControl-Wert auf0. Der Rust-Rechenteil beschreibt diese Verschiebung, führt die native Probe aber noch nicht aus. Ein erforderliches unbekanntes Ergebnis ist ein Fehler; es wird nicht als freie Strecke angenommen.

Das Beschleunigungslimit beginnt bei `AccelRate * effective_AirControl`. Bei horizontalem Tempo unter10 und positivem AirControl kommt `(10 - speed) / full_dt` hinzu. Die angeforderte Beschleunigung wird nur gekürzt, kleinere analoge Eingaben bleiben erhalten.

Ab horizontalem Tempo mindestens GroundSpeed gibt es zwei andere Zweige: bei wirksamem AirControl über0,05 wird das ursprüngliche horizontale Tempo als Cap erhalten; bei AirControl bis0,05 wird das Beschleunigungslimit auf1 gesetzt. Auch ein durch die Vorausprobe auf0 gesetzter AirControl-Wert kann damit bei hohem Anfangstempo dieses Limit1 erhalten. GroundSpeed ist hier kein allgemeiner harter Cap: Ein Schritt, der unter GroundSpeed beginnt, kann ihn überschreiten.

Zeitschritte über0,05 werden aus der verbleibenden Zeit halbiert und auf maximal0,05 begrenzt. Es gibt höchstens acht Substeps bei einem anfänglichen Iterationszähler0. Pro Substep wird `velocity += (Gravity + acceleration) * step_dt` berechnet, gegebenenfalls horizontal auf das erhaltene Anfangstempo begrenzt und die Bewegung als `(velocity + ZoneVelocity) * step_dt` geplant. ZoneVelocity ist hierbei eine Bewegungsverschiebung und wird nicht zur gespeicherten Geschwindigkeit addiert.

Die TerminalVelocity-Prüfung folgt im Original nach dem Bewegungs-/Kontaktpfad und begrenzt bei nicht gesetztem Sperrflag Pawn+0x6c Bit31 die gesamte dreidimensionale Geschwindigkeit. Der isolierte Rechenteil bildet den freien Pfad ohne dieses Flag ab: erst Verschiebung mit der noch ungekürzten Geschwindigkeit, danach Vektor-Cap auf TerminalVelocity. Das unterscheidet sich vom bisherigen eigenen Körperlöser mit Begrenzung allein in Z.

Direkt aus der Original-engine.dll geprüft: VA10665bbc=0,05000000074505806,10668e18=10 und1065efcc=0. Zusammen mit den vorhandenen vier Rechenkonstanten werden sie durch `scripts/Extract-VelocityConstants.py ... --falling` aus den PE-Sektionen gelesen; Bytes, Dateioffsets und SHA256 stehen in `analysis/collision/native-falling-constants.json`. Rust verwendet f64/hypot und den exakt nachf64 erweiterten0,05-f32-Wert; vollständige native SSE-f32-Parität ist nicht bestätigt.

## Prüfung vom 2026-10-06

85 Workspace-Tests und Clippy über alle Targets bestehen. Fünf neue Tests prüfen den Anteil bei geringem Tempo und dessen Grenze10, analoge Eingaben, unbekannte/gesperrte Vorausprobe, den0,05-Schwellenwert, erhaltenes hohes Anfangstempo, das native Limit1 bei geringem AirControl, Überschreiten von GroundSpeed, den dreidimensionalen Terminal-Cap nach der Verschiebung, ZoneVelocity, Substep-Aufteilung und explizite Fehler bei ungültigen Eingaben oder erschöpftem Budget.

`rc-falling-probe <GameData> <report.json>` liest das Original-Bewegungsprofil und führt drei isolierte Sequenzen mit je360Ticks bei1/60Sekunde aus. Initiale Z-Geschwindigkeit ist der aufgelöste JumpZ-Wert475; dies ist ein gesetzter Startzustand, keine rekonstruiert ausgelöste Sprungaktion.180TicksVorwärtsbeschleunigung,60Querbeschleunigung,120TicksohneBeschleunigung. Treffer-/Freigabezustand der Vorausprobe ist vorgegeben.

| Variante | Schwerkraft Z | Vorausprobe | maximales XY-Tempo | erster Tick mit VelocityZ≤0 |
|---|---:|---|---:|---:|
| Basisprofil | -1100 | frei | 452,02665913899784 | 25 |
| geringe Schwerkraft | -110 | frei | 452,02665913899784 | 259 |
| gesperrte Luftsteuerung | -1100 | Treffer | 0 | 25 |

Ticks sind nullbasiert. Gravity-110 ist eine kontrollierte Variante passend zum zuvor auditierten Hangar-Wert, keine Karten-Auswahl in diesem Werkzeug. Die freien Sequenzen halten nach Loslassen horizontale Trägheit; eine Bodenlandung wird nicht simuliert. Nach6Sekunden ist Z-Geschwindigkeit beim Basisprofil etwa-6125, bei geringer Schwerkraft etwa-185.

Zusatzprobe0,12Sekunden: drei Substeps von0,05000000074505806,0,03499999962747097 und0,03499999962747097; bei Startgeschwindigkeit0 und GravityZ-1100 ergeben sich VelocityZ-132 und VerschiebungZ-10,642500012293457. Die Terminalprobe mit Anfangsgeschwindigkeit[6000,8000,-12000] endet mit Vektorbetrag12000; alle Komponenten werden skaliert. Eine fehlende erforderliche Vorausprobe und ein Beispiel mit1Sekunde und erschöpftem Acht-Schritte-Budget melden explizite Fehler.

Berichte: `analysis/collision/free-falling-validation.json`, Zusammenfassung `falling-summary.json` im selben Ordner und `analysis/evidence.json`.

## Grenzen und nächste Verbindung

Die zurückgegebenen Verschiebungen sind Rechenergebnisse für freien Raum und keine kollisionsgeprüften Positionsvorschläge. Eine vorgegebene blockierte Vorausprobe prüft den Rechenzweig; sie rekonstruiert keinen anschließenden Hinderniskontakt. Welt-/Actor-Kollision, native Hit-Daten, Landung, Wassereintritt, Zwei-Wand-Korrektur, Landed/NotifyJumpApex/ModifyVelocity, laufende Volume-Wechsel und weitere native Flags fehlen.

Bei Budgeterschöpfung gibt Rust keinen Teilzustand zurück; das ist eine Diagnoseentscheidung. Native Iterationszähler aus vorherigen Physikzuständen werden noch nicht übergeben. Die bestehenden Controller-, Walking- und Android-Pfade verwenden weiterhin ihre bisherigen Regeln. Der neue Rechenteil ist noch nicht mit Weltkollision oder JNI verbunden. Android-Prüfung auf Nutzerwunsch zum Schluss; kein APK-Build und kein Emulatorlauf in diesem Schritt.

## Folgearbeit: statische Weltkollision

Ein separater Adapter verbindet nun Einzelschritte des Rechenteils mit echter AABB-Vorausprobe, Gleiten, Kontaktprojektion und diagnostischer Bodenbindung. Drei Originalkartenbahnen über672Ticks landen ohne Fehler; 90Tests/Clippy bestehen. Native Landed-/Walking-Übergänge und mehrteilige kollisionsbehaftete Ticks bleiben offen. [Ergebnisse und Grenzen](FALLING_COLLISION.md). Android-Prüfung weiterhin zum Schluss.
