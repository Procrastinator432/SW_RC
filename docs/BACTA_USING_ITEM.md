# Bacta: ursprünglicher Spielercontroller

Stand: 16.10.2026. Fortsetzung der Bacta-Animation; der zweite Handytest-Korrekturblock bleibt insgesamt offen.

Fortsetzung17.10.: Der originale Energie-Dauerton ist inzwischen angebunden und wird beim Fortsetzen aus dem aktuellen Stationszustand aufgebaut. Die14 visuellen Emittertemplates sind extrahiert; ihre Renderintegration bleibt offen. [Nachweise und Grenzen](BACTA_AMBIENT.md).

## Originalbefund

`CTPlayer.uc`, Zustand `UsingItem`, enthält den fehlenden Controller direkt als originales UnrealScript. Eine angenommene feste Kamera oder ein größerer Stationsabstand wäre dafür keine ausreichende Rekonstruktion.

- `DesiredLocation = Target.Location + PlayerOffset * Vector(Target.Rotation)` bei gesetztem `bLockPlayerLocation`; die bisherige Pawn-Z-Höhe bleibt erhalten.
- Bei `Dist > 450 * DeltaTime` erhält `ProcessMove` die Beschleunigung `15 * ToLoc - Pawn.Velocity`, mit Z=0.
- Andernfalls versucht der Controller bei Dist>1 `SetLocation` und setzt Geschwindigkeit/Beschleunigung auf null.
- Der Blick läuft zum ursprünglichen Actor plus `HUDMarkerIconZOffset`. Yaw/Pitch-Differenzen werden auf den kurzen Rotatorbogen gebracht und mit45.000 Rotatoreinheiten/Sekunde begrenzt.
- Freie Bewegung/Blicksteuerung und Waffenaktionen werden in `UsingItem` ersetzt beziehungsweise ignoriert. `StopUse` kehrt zu `PlayerWalking` zurück. Ein Versuch, während des gehaltenen Benutzens wegzulaufen oder wegzusehen, ist damit kein Abbruchmechanismus; Loslassen bleibt der normale Abbruch.

Der originale Actor736 löst zu `PlayerOffset=90`, `PlayerRange=256`, `bLockPlayerLocation=true` und `HUDMarkerIconZOffset=10` auf. Die Kollisionsdefaults sind Radius40, Höhe120, bCollideActors/bBlockActors/bBlockPlayers jeweils true. Die originale Quelle und die aufgelösten Defaults stehen in den lokalen Extraktionsnachweisen unter `build/bacta-controller/original`.

## Umsetzung und Grenze

Die optionale `bacta.controller`-Bindung enthält Originalposition, den aus Originalrotation/-Offset berechneten XY-Zielpunkt und die übrigen genannten Defaults. Die Bactastufe importiert diese Werte; ältere Szenen ohne diese Bindung behalten ihre bisherige Bedienung.

Während des Benutzens wird die Originalbeschleunigung durch denselben Volumen-/Geh-/Fallsolver wie die freie Bewegung geführt. Der neue `pawn_acceleration_tick` umgeht ausschließlich die Abbildung des Sticks auf Beschleunigung. Die kurze SetLocation-Korrektur wird zusätzlich gegen die vorhandene Welt gesweept; sie versetzt den Spieler nicht durch eine Wand. Beschädigte oder unvollständige Abfragen werden abgewiesen. Nach Loslassen beziehungsweise Zielabschluss kehrt die freie Steuerung zurück.

Die bestehende konservative Box-/AABB-Kollision ist weiterhin ein Portadapter. Sie verhindert am diagonalen Standpunkt die letzten ungefähr10 Welt-Einheiten zum idealen Originalziel. Der Controller richtet den Blick trotzdem korrekt auf die Station aus; die bisher seitlich in die Kamera ragende geöffnete Station erscheint in den normalen Replaybildern mittig. Exakte Zylinder-/Pawn-Kollision, sämtliche nativen Rotator-SSE-Rundungen und eine physische Bildabnahme werden damit nicht behauptet. Zielauswahl bleibt zusätzlich durch den bisherigen Tutorial-Nähe-/Blickadapter begrenzt. Originaler HandSignal-Waffenwechsel, ViewShake und Netzwerkreplikation sind nicht angeschlossen.

## Eingaben und Prüfungen

Die deklarierte Stufe `map-a-bacta-replay` nimmt die neue Szene und das gepinnte bisherige Eingabefile entgegen. Sie simuliert den bisherigen Controllerzustand als Vergleich und ergänzt **99 reguläre Bewegungs-/Blickeingaben nach dem Loslassen**, um dessen bisherigen Anschlussort und Blick zu erreichen. Alle ursprünglichen Eingabezeilen bleiben bytegleich erhalten. Es werden weder Position, Blick, Gesundheit noch Missionszustand als Testfixture gesetzt. Das neue File umfasst18.829 statt18.730 Eingaben; `exit-rejoin.json` hält Ausgangs-/Ziel-/Endposition und beide Fingerabdrücke fest. Die Ortsabweichung am Anschluss beträgt etwa0,018 Einheiten. Die komplette Mission muss anschließend mit diesem File bestehen.

`Test-BactaLifecycle.py` prüft zusätzlich, dass gleichzeitig angeforderte Bewegung, Blick, Springen, Ducken, Feuer, Nachladen und Granaten während `UsingItem` exakt denselben fortgesetzten Zustand ergeben wie ausschließlich gehaltenes Benutzen. Loslassen, Bereichsverlassen nach Loslassen, erneutes Benutzen, fünf Checkpointphasen, beschädigte Checkpoints und sechs einmalige Original-Soundereignisse bleiben geprüft. Der ursprüngliche Rotator-Halbbogen und das Verhalten an der0/65536-Grenze haben einen gezielten Regressionstest.

Die bestehenden Checkpoints speichern bereits Benutzungszustand, Position, Blick, Geschwindigkeit und Stationsanimation; eine neue Snapshot-ABI ist nicht notwendig. Historische Szenen/Spielstände werden beim Fortsetzen mit ihrer archivierten Szene geladen.

Die Arbeitsnachweise liegen unter `build/bacta-controller`: zentraler Check,21-Stufen-Neubau, native Bacta-Prüfung mit Spielerbildern und Paket-/Legacy-Prüfungen. Die veröffentlichte0.50.0 bleibt unverändert; dies ist noch keine neue Android-Testlieferung.

Bestanden: je1.132 Debug-/Release-Bibliothekstests, zwei Materialimporttests, Doc-Tests, Clippy aller Targets, Java-Prüfungen und Buildvertrag; der komplette21-Stufen-Neubau mit25 Zustands-/Bild-/Fortlauf-/Neustartproben bis zur Fabrik;30 Soundreferenzen/26 Originalcues am endgültigen ZIP; beide Droidentöne und Rampenmaterial sowie vier echte historische Checkpoint-Restores. Das Arbeits-ZIP hat SHA-256 `a271fd54de12f1f5636682aa6a52581c13b4838abe902903b3035e8fdc773b61`. `source-and-input-proof.json` hält zusätzlich die Originalquellen-Hashes und die bytegleiche Erhaltung aller ursprünglichen Eingabezeilen fest. Android-Geräte-/Hörabnahme bleibt für die gemeinsame Inszenierungslieferung offen.

## Als Nächstes

Eye-/Körpereffekte und deren Original-Energiedauerton anbinden; dabei Emitterdefaults und Start-/Stop-/FadeOut-Lebensdauer ausdrücklich auflösen. Anschließend Verwundungs- und Barrierenablauf sowie Scorchs Bunker, Tür und A→B-Übergang aus normaler Spieleransicht abnehmen. Der verbleibende Kollisions-/Nahansichtspunkt gehört zur visuellen Gesamtabnahme.
