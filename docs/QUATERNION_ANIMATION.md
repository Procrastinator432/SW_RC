# Quaternion-Decodierung und Slerp

`crates/rc-package/src/quaternion_animation.rs` implementiert die belegten Rechenwege von FAnimRot.operator FQuat (`engine.dll:104fdc60`) und FQuat.Slerp (`core.dll:101449f0`). `QuaternionTrackHost` verbindet beide mit dem vorhandenen Track-Sampler. Damit können gelieferte komprimierte Rotationstracks ohne Platzhalter-Quaternionen ausgewertet werden. Die originale CPU-Mathematik bleibt eine austauschbare, ausdrücklich benannte Strategie.

## Komprimierte Rotation

Jeder Schlüssel besteht aus drei u16-Wörtern. Jeweils Bit null wird vor signierter i16-Konvertierung maskiert. Die Komponenten werden mit dem initialisierten Rotationsmaßstab multipliziert. Dieser Maßstab ist anhand der Schreibreferenz auf `108b85d0` und der Initialisierungsinstruktionen rekonstruiert:

1. x87 berechnet sqrt der gespeicherten Doublekonstante 0,5 und speichert f32.
2. SSE dividiert 32767 durch diesen Float und konvertiert mit Abschneiden nach i32: 46339.
3. SSE dividiert 1 durch diesen Wert: f32-Bitmuster `0x37b506e6`.

Der Rustmaßstab beschreibt den Zustand nach dieser Initialisierung. Er emuliert weder DLL-Startreihenfolge noch spätere Speicheränderungen. Die Initialisierungs-Sqrt wurde als auf f32 gerundeter mathematischer Wert geprüft; eine allgemeine x87-Emulation folgt daraus nicht.

Die vierte Komponente wird aus `((1-a*a)-b*b)-c*c` rekonstruiert. Das Original verwendet einen RSQRTSS-Startwert r und die getrennten Floatoperationen `(3-(r*n)*r)*(r*0.5)`, anschließend Multiplikation mit n. Bei n=0 wird das fertige Bitmuster auf positive Null maskiert; die CPU-Operationen werden davor trotzdem ausgeführt. Das niedrigste Bit des dritten Wortes steuert die Subtraktion `0-d`. Die niedrigsten Bits der ersten beiden Wörter bestimmen eine zyklische Komponentenrotation um 0 bis 3 Stellen. Diese Operationen werden in ihrer Originalreihenfolge übernommen.

Es gibt keinen zusätzlichen Clamp eines negativen Radikanden und keine nachträgliche Quaternion-Normalisierung. Fehler an der Mathegrenze verhindern das Zurückgeben einer fertigen Quaternion; der Track-Sampler erhält dadurch seine vorher belegte Schreibreihenfolge.

## Slerp-Auswahl und Mischung

Der Skalarproduktablauf ist `(ax*bx+az*bz)+ay*by+bw*aw`. Ein negatives Ergebnis wird für die Pfadauswahl durch Subtraktion von null positiv gemacht. Der Originalschwellwert beträgt **0,55**, als Bitmuster `0x3f0ccccd`; die verbreitete Schwelle nahe eins wäre hier falsch.

Ab dieser Schwelle wird mit Gewichten `1-alpha` und `alpha` linear gemischt. Bei negativem Skalarprodukt wird das zweite Gewicht negiert, damit äquivalente Quaternionen den kurzen Weg verwenden. Anschließend normalisiert das Original das Ergebnis: Quadratsumme in Reihenfolge w,z,y,x, RSQRTSS und dieselbe einmalige Korrektur. Anders als die Rotationsdecodierung maskiert diese Normalisierung bei Quadratsumme null nichts weg. Ein ungeordneter Vergleich (NaN) nimmt ebenfalls den linearen Zweig.

Unterhalb der Schwelle berechnet das Original acos, FSIN und die sphärischen Gewichte mit x87. Beide Gewichte werden nach f32 gespeichert und erst danach mit den Quaternionkomponenten gemischt. Dieser Zweig normalisiert nicht noch einmal. Alpha und Skalarprodukt werden nicht zusätzlich begrenzt. Insbesondere bleibt die vom Track-Sampler belegte Extrapolation möglich.

## Portable Mathe und Genauigkeitsgrenze

`QuaternionMath` kapselt ausschließlich den RSQRTSS-Startwert und die x87-Gewichte. Damit lassen sich native Referenzwerte später einsetzen, ohne Schlüsseldecodierung oder Mischreihenfolge neu zu implementieren. Ein fehlender Referenzwert kann als expliziter Fehler zurückkommen.

`PortableQuaternionMath` ist eine verwendbare plattformunabhängige Strategie: f32-Sqrt mit Kehrwert als Startwert und f64-acos/sin für sphärische Gewichte. Die originale Korrektur und alle anschließenden f32-Operationen bleiben erhalten. Diese Strategie emuliert keine CPU-spezifische RSQRTSS-Näherung, x87-Kontrollwortpräzision, Ausnahmeflags, Denormalmodi oder NaN-Payload-Auswahl. Bitgleichheit zur alten x86-Version wird deshalb nicht behauptet. Originaltracks, die vorgeschaltete Framezeitberechnung und Vollposeauswertung fehlen weiterhin.

## Prüfung

80 Decodierungsfälle kombinieren fünf Komponentensätze, alle acht Flagkombinationen und erfolgreiche beziehungsweise fehlende Matheprimitiven. 84 Slerp-Fälle kombinieren sechs Quaternionpaare, sieben Zeitwerte und erfolgreiche beziehungsweise fehlende Primitiven. Die unabhängige Pythonprüfung berechnet alle Ausgabe- und Ereigniswörter dieser 164 Fälle bitweise nach. Ein gelieferter Startwert 0,875 und Gewichte 0,25/0,75 sind dabei ausdrücklich Diagnosedaten, keine native Messung.

Zwölf weitere Fälle verwenden den verbundenen Track-Sampler mit tatsächlicher portabler Rotationsberechnung. Sie prüfen identische, äquivalent negierte und allgemeine komprimierte Quaternionen samt Positionsinterpolation gegen eine unabhängige mathematische Quaternionreferenz. Der größte Komponentenfehler in diesen zwölf Fällen beträgt etwa `4,81e-8`, unter der gewählten Toleranz `1e-6`. Diese kleine Stichprobe ist keine globale Fehlergarantie oder sichtbare Originalanimation.

Sechs neue Tests prüfen Komponentenflags/Vorzeichen, Originalschwelle, kurzen Weg, fehlenden Clamp/Normalisierung im sphärischen Zweig, NaN-Auswahl, Fehlerweitergabe, portable Normalisierung und Track-Verbindung. Insgesamt 248 Workspace-Tests (238 rc-package, 4 rc-native, 6 rc-render), Clippy mit Warnungen als Fehler und Formatprüfung bestanden.

```text
cargo run -p rc-inspect --bin rc-quaternion-probe -- analysis/reports/quaternion-animation.json
python scripts/Record-QuaternionAnimation.py
```

Nachweise: `analysis/decompiled/quaternion-slerp.c/.asm`, `skeletal-rotation-decode.c/.asm`, `rotation-scale-refs.txt`, `rotation-scale-init.asm`, `analysis/reports/quaternion-animation.json`, `quaternion-animation-validation.json` und `analysis/evidence.json`. Androidprüfung weiterhin zum Schluss.

Fortsetzung: [Originale Animationstracks](ORIGINAL_ANIMATION_TRACKS.md) lädt nun 61.310 Tracks aus 166 originalen MeshAnimation-Exporten und berechnet 4.194 erste Track-Samples. Mesh-Knochenzuordnung und Vollpose bleiben offen; die obigen synthetischen Prüfungen dokumentieren weiterhin den vorausgehenden Mathemeilenstein.
