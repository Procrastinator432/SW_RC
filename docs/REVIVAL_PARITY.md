# Wiederbelebung: Gesundheitsverlauf und Animationsabschluss

Version0.29.1 / Code33, Lieferung `analysis/mission11-revival`. Die `geo_01a-revival.rcplay` ist bytegleich mit der bisherigen Squad-Szene; neue Assets sind für diese Laufzeitkorrektur nicht nötig. Die APK enthält die geänderte Simulation.

## Geschlossene Lücke

Die lokale Originalquelle `analysis/reports/sources/System__engine.u/Pawn.uc`, State Reviving, setzt beim Beginn Health=1, erhöht im Tick um `MaxHealth / 12.0f * DeltaTime` und setzt beim Ende `Health=MaxHealth/2`. Zuvor fehlte der fortlaufende Tick-Schritt im Port.

Die neue Rechnung erhält f32-Bruchteile über alle Ticks. Die vorhandene ganzzahlige Gesundheitsanzeige rundet erst bei der Ausgabe auf. Externe Gesundheitsänderungen werden mit dem Akkumulator abgeglichen, sodass zusätzlicher Schaden nicht durch einen alten Heilungswert überschrieben wird. Dieser Abgleich ist eine Portgrenze zur bestehenden ganzzahligen Missionsgesundheit, keine vollständige Nachbildung von Pawn.TakeDamage.

Scorch steigt während der originalen ImmobileGetUp-Animation von1 bis182 angezeigteHP und endet bei200. Der Spieler steigt während PlayerImmobileGetUp von1 bis55 angezeigteHP und endet bei50. Der kleine Rückgang auf50 beim Spieler folgt dem Original: Tick hat keine Begrenzung auf die Hälfte, EndState weist die Hälfte ausdrücklich zu. Der Spieler verwendet weiterhin die bereits etablierte Missions-MaxHealth100; Scorch verwendet die extrahierte MaxHealth400.

Nach dem Aufstehen wechselt Scorch sofort zu ActionBreathe. Die helfende Figur hält HealImmobileStop am letzten Animationsbild, anstatt es während des langen Spieler-Aufstehens wiederholt abzuspielen. Nach Abschluss werden die temporären Gesundheitsakkumulatoren verworfen; Neustart setzt sie ebenfalls zurück.

## Prüfung

- Drei zusätzliche Unit-Tests: kleine Bruchteilschritte, zusätzlicher Schaden und abweichende Spieler-MaxHealth/Heilung.
- `behavior/revival-ramp.json`:325 Scorch- und383 Spieler-Stützstellen während der Wiederbelebung, Abschlusswerte und direkte Idle-Pose auf Originalgeometrie. Der Test setzt Positionen/Gesundheit ausdrücklich als Fixture-Aufbau.
- Derselbe Rust-Fixture-Code läuft als eigenes Android-x86_64-Programm; `android-fixture/binding.json` bindet den Test und bestätigt den identischen Gesundheitsverlauf auf Windows und Android.
- Vollständiger normaler Missionslauf und Scorch-Fokus-Kampflauf über Windows-/Android-ABI bleiben Regressionen. APK-Dateidialog, GPU, Touch, Audio und Pause werden mit der neuen APK geprüft.
- `validation-manifest.json` bindet Artefakte, Quellen und Belege. Die Lieferung0.29.0 bleibt unverändert.

## Weiterhin offen

Die Änderung betrifft den ausgewählten Wiederbelebungsablauf und dessen Gesundheitsrechnung. Allgemeine CTBot-/Squad-GoalQueue-, Deckungs-/Granaten-/Perch-Parität, vollständige DamageType-Todesausnahmen aus Pawn.Reviving.TakeDamage, dynamische Pawn-Ausweichkollision und allgemeiner Squad-Travel-Host bleiben offen. Auch vollständige Inszenierung, GEO_01B-Laden/Zustandsübernahme sowie manueller Touch-Gesamtlauf und physische ARM64-Abnahme bleiben offen.

Bedienung von Folgen/Halten/Fokus und Wiederbelebung: [Gegner und Squad](ENEMIES_AND_SQUAD.md).
