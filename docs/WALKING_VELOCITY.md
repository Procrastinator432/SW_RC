# Native Bodenreibung: isolierter Rust-Rechenteil

`pawn_velocity::walking_velocity` rekonstruiert den XY-Rechenteil von `APawn::calcVelocity` (engine.dll1048f900) für den Walking-Aufruf aus10492b60. Der Aufruf liefert DeltaTime, GroundSpeed, Volume-GroundFriction und die Flags fluid=0, brake=1, buoyancy=0. Walking setzt Geschwindigkeit und Beschleunigung in Z zunächst auf0 und übergibt die normierte Beschleunigungsrichtung. Der Export steht in `analysis/decompiled/pawn-velocity.c`; darin ist zusätzlich AActor::physFalling10490a30 enthalten, keine Pawn-Falling-Implementierung.

Die Native-Zugriffe betreffen Geschwindigkeit bei Pawn+0x150/154/158, Beschleunigung+0x15c/160/164, Volumezeiger+0x134, Gravity im Volume+0x308/30c/310 und GroundFriction+0x314. Die aufeinanderfolgenden Volume-Felder entsprechen dem extrahierten PhysicsVolume-Quelltext. GroundSpeed wird beim Aufruf aus Pawn+0x528 gelesen; das Beschleunigungslimit im Rechenteil aus+0x53c. Laufzeit-SpeedFactor und die nachfolgenden Geschwindigkeitsbeschränkungen sind noch nicht vollständig rekonstruiert.

## Rekonstruierte Rechnung

Mit Beschleunigung dreht der Rechenteil zuerst die bestehende Geschwindigkeit zur Beschleunigungsrichtung und addiert anschließend die begrenzte Beschleunigung:

```text
v = v - (v - |v| * direction) * dt * GroundFriction
v = v + limited_acceleration * dt
```

Ohne Beschleunigung reduziert er die Geschwindigkeit, ohne die Richtung umzukehren:

```text
speed_reduction = |Gravity| * dt * GroundFriction * 0.125
v = v * max(0, (|v| - speed_reduction) / |v|)
```

Die Native-Konstanten wurden direkt aus den PE-Sektionen der Original-engine.dll gelesen: VA106701b4=0.125,10653578=1,1065efc8=3,1065efc4=0.5. Die letzten beiden gehören zur SSE-Näherung für inverse Quadratwurzeln. Das reproduzierbare Werkzeug `scripts/Extract-VelocityConstants.py` überprüft diese Werte und speichert Bytes, Dateioffsets und SHA256 in `analysis/collision/native-velocity-constants.json`. Die Rust-Rechnung verwendet f64/hypot; bitgenaue SSE-f32-Parität ist nicht bestätigt.

Pawn.DecelRate600 wird in diesem Bremszweig nicht gelesen. Die bisherige eigene Controller-Regel bremst dagegen mit diesem konstanten Wert. Daraus folgt keine Aussage darüber, ob DecelRate an anderen Stellen der Engine verwendet wird.

## Prüfung vom 2026-10-06

78 Workspace-Tests, Clippy über alle Targets und Formatprüfung bestehen. Vier neue Tests prüfen gravitations-/reibungsabhängiges Bremsen ohne Richtungsumkehr, Nullreibung/Nullschwerkraft, Drehen vor der Beschleunigung, Begrenzung der Beschleunigung, analoge Beschleunigung, vektorielle Geschwindigkeitsgrenze, exakten Stillstand und explizite Fehler bei ungültigen Eingaben.

`rc-velocity-probe` liest das Original-Bewegungsprofil und führt drei isolierte Abläufe mit je420Schritten bei1/60Sekunde aus:60Vorwärtsbeschleunigung,60Querbeschleunigung,300Schritte ohne Beschleunigung. Beschleunigungslimit1024 und Geschwindigkeitslimit450 werden vom Diagnoseaufrufer vorgegeben; vollständige Native-Runtimefaktoren sind damit nicht nachgestellt.

| Variante | Gravity-Betrag | GroundFriction | Bremsbetrag pro Sekunde | Schritte bis Stillstand ab Tempo450 |
|---|---:|---:|---:|---:|
| Basisprofil | 1100 | 8 | 1100 | 25 |
| geringe Schwerkraft | 110 | 8 | 110 | 246 |
| ohne Reibung | 1100 | 0 | 0 | kein Stillstand |

Die geringe Schwerkraft und Nullreibung sind kontrollierte Diagnosevarianten, keine Karten-Trajektorien. Der Wert110 entspricht dem zuvor auditierten dm_hangar-Volume, dessen Auswahl hier aber nicht ausgeführt wird. Ohne Reibung bleibt die Geschwindigkeit in der300-Schritte-Ruhephase erhalten. Maximaltempo liegt innerhalb f64-Rundung bei450 (max450.0000000000001). Bericht: `analysis/collision/walking-velocity-validation.json`; Zusammenfassung in `analysis/evidence.json`.

## Noch offene Verbindung zur Bewegung

Der Helfer erwartet bereits effektive Beschleunigungs- und Geschwindigkeitsgrenzen. Native SpeedFactor wurde separat unter1048cd00 exportiert (`pawn-speed-factor.c`), seine Decompilierung enthält jedoch nicht korrekt zugeordnete Register-/Argumentwerte. Die vollständige Umsetzung benötigt außerdem MaximumDesiredSpeed, Walking-/weitere Runtimeflags und das ModifyVelocity-Ereignis. Im vorliegenden extrahierten Pawn-Quelltext ist das Ereignis nur deklariert; daraus folgt kein Beweis, dass es zur Laufzeit immer ohne Wirkung bleibt.

Der Rechenteil umgeht ModifyVelocity und verwendet den vorgegebenen End-Cap. Seine Eingabegrenzen (dt bis0,05, endliche begrenzte Werte) sind Diagnosebedingungen. Er enthält keine Kollision oder Unterstützungserkennung und setzt keine Pawn-Zustände um. Die vorhandene PC-Steuerung verwendet weiterhin ihre eigene Formel; der neue Helfer ist noch nicht mit den Originalkarten-Bewegungsproben oder JNI verbunden. Wasser/Falling und eine vollständige native Walking-Rekonstruktion bleiben offen. Android-Prüfung wie gewünscht zum Schluss; kein APK-Build oder Emulatorlauf in diesem Schritt.

## Folgearbeit: SpeedFactor und Geometrie

Die zuvor unklaren SpeedFactor-Zweige sind jetzt anhand des Assemblers geklärt. Der separate Walking-Diagnosepfad verwendet diese Faktoren zusammen mit Volume-Parametern und Weltkollision. 300 Schritte auf geo_01a erfolgreich geprüft; 80 Tests/Clippy bestehen. Weitere Runtimeflags, ModifyVelocity und native Falling-Parität bleiben offen. [Details und Annahmen](WALKING_BRIDGE.md). Android-Prüfung weiterhin zum Schluss.
