# GetRotPos: Schlüsselzeiten und Positionen

Der Rust-Adapter in `crates/rc-package/src/skeletal_track.rs` übernimmt FSkelAnimSeq.GetRotPos (`engine.dll:10500fc0`) für ausdrücklich gelieferte Track-Snapshots. Er enthält außerdem die geordnete Schlüssel-Suche aus GetLinkupFromSeq (`105003d0`). Originale Skelette oder Animationstracks werden damit noch nicht aus Spieldateien geladen.

## Tracklayout und Schlüsselwahl

Das Original findet einen Track über Sequenz +5c und Index × 1c. Jeder Track enthält Rotationsarray/Anzahl bei +0/+4, Positionsarray/Anzahl bei +8/+c, Positionsmaßstab bei +10 und Byte-Dauerarray/Anzahl bei +14/+18. Beide Schlüsseltypen bestehen aus drei 16-Bit-Werten. Positionskomponenten sind signiert; Rotationswörter bleiben für den Host unveränderte u16.

Wichtig: +10 ist ein Float-Bitmuster. Das zeigen MOVSS/MULSS eindeutig, auch wenn der Dekompiler das Feld wie einen Integer darstellt. Die Zahl wird mit der PE-geprüften Konstante `0x38000100` (f32-Näherung von 1/32767) multipliziert; jeder signierte Positionswert wird anschließend separat in f32 konvertiert und damit multipliziert.

Die Daueranzahl wird nativ durch SHL/SAR um drei Bits mit Vorzeichen erweitert. Bei einem Ergebnis ≤1 liest die Funktion ohne Zeitprüfung den jeweils ersten Rotations- und Positionsschlüssel. Dabei werden selbst Schlüsselanzahlen von null nicht geprüft. Rust reproduziert die Auswahl, gibt bei tatsächlich fehlendem Speicher aber einen Fehler zurück.

Bei mehreren Dauern subtrahiert die Funktion Bytewerte nacheinander von der gelieferten Framezeit. Die erste strikt negative Differenz beendet die Suche; zur Interpolation dient der Rest **vor** dieser Subtraktion. Eine exakte Abschnittsgrenze wählt den folgenden Schlüssel ohne Interpolation. Nach einem vollständigen Durchlauf springt die Auswahl auf Schlüssel null, behält jedoch den Rest nach genau einem Durchlauf. Sie berechnet kein wiederholtes Modulo: Zeit 15 bei Dauern [2,3,5] ergibt daher Faktor 2,5 zwischen Schlüssel 0 und 1. Der nächste Schlüssel nach dem letzten ist null.

Nicht positive oder ungeordnete Restzeiten verwenden den direkten Schlüssel. NaN durchläuft die Dauern und landet bei direktem Schlüssel null. Null-Dauern und positive Restzeit können zu einem unendlichen Interpolationsfaktor führen; der Adapter führt keinen zusätzlichen Clamp ein. Rotations- und Positionsanzahl werden separat mit `0x1fffffff` maskiert. Genau ein Schlüssel verwendet immer Index null.

## Reihenfolge und offene Rotation

Bei interpolierter Rotation decodiert das Original zuerst den nächsten, dann den aktuellen Schlüssel und ruft Slerp(current,next,alpha) auf. Erst das fertige Rotationsergebnis wird ins Root-Output geschrieben. Danach folgen Positionszugriffe und die komponentenweise SSE-Reihenfolge `(next-current)*alpha+current`. Ein späterer Positionsfehler erhält daher eine bereits geschriebene Rotation. Ein Fehler in der Rotation erhält beide alten Outputs.

Quaternion-Decodierung und Slerp sind explizite `TrackRotationHost`-Operationen. Die zusätzliche native Analyse von FAnimRot.operator FQuat (`104fdc60`) belegt maskierte/signierte Komponenten, Bit-gesteuerte Komponentenpermutation und Vorzeichen sowie eine RSQRTSS-Näherung mit anschließender Korrektur. Diese CPU-Näherung wird noch nicht durch eine unbestätigte Android-Sqrt-Implementierung ersetzt. Auch die vorgeschaltete x87-Multiplikation von Frameanzahl und normalisiertem Kanalframe bleibt offen; `sample_track` erhält die bereits berechnete f32-Framezeit.

GetLinkupFromSeq vergleicht den opaken Wert Sequenz +64 mit Linkup +4 und liefert den ersten Treffer in Arrayreihenfolge. `find_linkup` bildet diese Suche für gelieferte Einträge ab; None bedeutet fehlende Sequenz, während Schlüssel null ein gültiger Vergleichswert ist. Es erfolgt keine spekulative Interpretation als Name oder Zeiger. Native Array-/Pointerverwaltung und Linkup-Aufbau bleiben außerhalb dieses Snapshots.

## Nachweise

240 synthetische Fälle kombinieren zwölf Track-/Fehlervarianten mit zwanzig Framezeiten. Eine unabhängige Pythonprüfung berechnet Schlüsselwahl, Floatpositionen, Ereignisreihenfolge, Ergebnis und sämtliche Outputwörter erneut. Die Diagnose liefert Quaternionwerte ausdrücklich als Platzhalter; die Rotationsgrenzen können auch gezielt fehlschlagen. Sie beansprucht keine Originalclip-Wiedergabe. Frühere Berichte bleiben erhalten.

```text
cargo run -p rc-inspect --bin rc-skeletal-track-probe -- analysis/reports/skeletal-track.json
python scripts/Record-SkeletalTrack.py
```

Acht neue Tests prüfen Grenzen und einmaligen Umlauf, NaN/Null-Dauern/Anzahltags, Floatmaßstab und Aufrufreihenfolge, Singleton- und Kurzpfad, Teilschreibzugriffe, erste Linkuptreffer und die Verbindung vom vorbereiteten Rootzweig über ein geliefertes Track bis zur Matrix. Im Verbindungstest ist die Framezeit 2 × 0,5 exakt vorgegeben; damit wird keine allgemeine x87-Emulation behauptet.

242 Workspace-Tests (232 rc-package, 4 rc-native, 6 rc-render), Clippy mit Warnungen als Fehler und Formatprüfung bestanden. Nachweise: `analysis/decompiled/skeletal-get-rot-pos.c/.asm`, `skeletal-rotation-decode.c/.asm`, `analysis/reports/skeletal-track.json`, `skeletal-track-validation.json` und `analysis/evidence.json`. Vollpose, echte Tracks und sichtbare Animation bleiben offen. Androidprüfung weiterhin zum Schluss.

Fortsetzung: [Quaternion-Decodierung und Slerp](QUATERNION_ANIMATION.md) übernimmt die Rechenwege und verbindet sie über `QuaternionTrackHost` mit dem Sampler. Eine portable sqrt/f64-Trigonometrie-Strategie berechnet jetzt Rotationen aus gelieferten komprimierten Schlüsseln. Native RSQRTSS-/x87-Bitgleichheit bleibt unbestätigt; die oben beschriebenen Platzhalterfälle dokumentieren weiterhin den früheren Meilenstein.
