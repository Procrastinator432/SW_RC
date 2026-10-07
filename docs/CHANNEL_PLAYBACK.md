# LOD-Kanalwiedergabe: Zustandsfortsetzung

Folgemeilenstein: [Animationsreplikation und PostInit-Grenze](ANIMATION_REPLICATION.md) ergänzt Actor-Slotprüfungen, Paketbildung und das Skelett-Root-Gate. Die frühere Kanalprobe bleibt unverändert; echte Matrix- und x87-Ausführung sind weiterhin explizite Grenzen.

`channel_playback.rs` setzt die zuvor geprüfte ULodMeshInstance.PlayAnim-Funktion (`10451100`) nach der Kanalauswahl fort. Der Adapter verarbeitet explizit gelieferte Kanalwörter, Sequenzmetadaten und Instanzbytes. Er rekonstruiert die Schreibreihenfolge einschließlich der offenen Callbackgrenzen; er wertet noch keine Posen aus und spielt keine Originalclips ab.

## Sequenz und Blendzeit

Der neue Sequenzsnapshot beschreibt die in diesem Ablauf gelesenen Felder: nicht null Pointeridentität, Frameanzahl (+10), Rate (+14), Mindestblendwert (+1c) und Zufallsstartbyte (+20). Die Pointeridentität ist ein opaques Diagnosetoken, kein Paketexportindex. Der Snapshot wird nicht aus Originalanim-Paketen geladen.

Die anfängliche Blendzeit beträgt 0,25. Bei vorhandener Sequenz und ausgeschaltetem Loop wird sie gegebenenfalls auf `(Frameanzahl / Sequenzrate) * 0,5` verkürzt. Danach kann der Sequenzwert bei +1c sie wieder erhöhen. Die Berechnung folgt den einzelnen f32-SSE-Operationen. Die Konstanten wurden direkt in engine.dll geprüft:

| Adresse | Bits | Wert |
|---|---|---|
| 106739cc | 3e800000 | 0,25 |
| 1065efc4 | 3f000000 | 0,5 |
| 10664f44 | 38000100 | 1/32767, auf f32 gerundet |

Ungeordnete NaN-Vergleiche werden anhand der COMISS-/UCOMISS-Sprünge behandelt. Insbesondere kann ein NaN-Dauerwert zunächst ausgewählt und anschließend durch den Mindestblendwert ersetzt werden. Ein NaN-Startframe setzt im Wiederverwendungszweig keine Frames; im Reset-Zweig deaktiviert er den Blendübergang. Der Adapter normalisiert diese Werte nicht. Native FP-Ausnahmemasken und MXCSR-Sondermodi werden nicht emuliert; die Berechnung setzt reguläre f32-Arithmetik voraus.

## Wiederverwendung und Neuinitialisierung

Ein Kanal wird ohne Neuinitialisierung weiterverwendet, wenn sein Sequenzhandle gleich bleibt und entweder seine alte Rate ungleich null oder die angeforderte Rate null ist. Ein Wechsel von gestoppt (alte Rate null) zu einer nicht null Rate initialisiert ihn erneut, selbst bei identischem Sequenznamen.

Bei Wiederverwendung überschreibt ein nicht negativer StartFrame die beiden Framefelder +1c und +30. Ein ausgelassener StartFrame (−1) erhält beide Felder. Sequenzpointer, Blendzustände und übrige Kanalwörter bleiben erhalten; PostInitAnim und ReplicateAnim werden nicht aufgerufen.

Bei Neuinitialisierung werden Sequenzhandle und Sequenzpointer zuerst geschrieben. Mit Sequenz wird das Feld +38 gelöscht und +24 auf `1 − 1/Frameanzahl` gesetzt. Ein expliziter StartFrame setzt beide Framefelder. Andernfalls fordert ein gesetztes Sequenz-Zufallsbyte einen nativen Rand-Wert an und setzt beide Felder auf `f32(rand) * f32(1/32767)`. Der Rand-Aufruf bleibt eine Hostgrenze. Ohne Zufallsstart bleiben die alten Frames nur erhalten, wenn alte Rate positiv, altes Loop-Byte gesetzt und neues Loop ebenfalls gesetzt sind; sonst werden beide Frames null.

Bei einer None-Sequenz wird +24 null. Der bisherige Blendwert +28 des ausgewählten Kanals wird nur an die davor liegenden Kanäle weitergegeben. Deren Felder +2c und +34 werden auf 1 gesetzt, wenn dieser Wert null ist, sonst auf 0. Nachfolgende Kanäle bleiben unverändert. Ein fehlender nicht-None-Clip muss bereits an der vorherigen LOD-Eintrittsprüfung abbrechen und wird vom Fortsetzungsadapter abgelehnt.

Danach setzt der Reset-Zweig +20 null und entscheidet über den Blendübergang. Bei Blendzeit null, gelöschtem Instanzbyte +60 oder nicht negativem bzw. ungeordnetem StartFrame einer nicht-None-Anforderung werden +28=0 und +2c=1 geschrieben. Sonst werden +28=1/Blendzeit und +2c=0 geschrieben. +34 übernimmt +2c.

## Gemeinsamer Abschluss und Callbackgrenzen

Beide Zweige schreiben anschließend das Loop-Byte (+04), die Rate (+18) und das Feld +10. Die drei Paddingbytes neben dem Loop-Byte bleiben erhalten. +10 übernimmt den Anforderungswert bei vorhandener Sequenz, sonst null. Ist das Kanalfeld +14 null, wird derselbe Wert zusätzlich nach +38 geschrieben. Bei Blendzeit null wird +2c auf 1 gesetzt. Die Bedeutungen von +10/+14/+38 werden über diese belegten Schreibzugriffe hinaus nicht angenommen.

Nur nach Neuinitialisierung ruft die Funktion PostInitAnim (virtuell +104) und anschließend AActor.ReplicateAnim (`103ba470`) auf. Letzteres erhält den Kanalindex zweimal sowie abschließend true. Erst danach löscht sie Instanzbyte +61 und liefert true. Der Host kann den ausgewählten Kanal in PostInitAnim verändern; die Replikationsgrenze sieht die danach gültigen Wörter. Kanalidentität und Arraylänge müssen während dieser Callbackgrenze erhalten bleiben.

Ein ungelöster Rand-/PostInit-/Replikationsaufruf hält genau an seiner Grenze an. Vorherige Schreibzugriffe werden nicht zurückgerollt, und +61 wird bei einem solchen Fehler nicht abschließend gelöscht. Native Callbackimplementierungen, Netzwerktransport und Instanzlebensdauer sind weiterhin offen. Profiler-, RDTSC- und ExceptionList-Verwaltung werden nicht emuliert.

## Nachweise

Die neue Probe übernimmt die 768 vorherigen synthetischen Szenarien mit ausgewähltem Kanal. Jeder Fall erhält neun explizite Varianten: neue Sequenz, Wiederverwendung, gestoppt zu laufend, expliziter Frame, Loopkontinuität, Zufallsstart, None, offene PostInit-Grenze und offene Replikationsgrenze. Anfangswerte, Sequenzmetadaten, Randwert 16384 und erfolgreich gelieferte Callbacks sind Diagnosedaten. Die Namen und Zuordnung der zugrunde liegenden 256 AnimProp-Anforderungen bleiben nachvollziehbar.

| Ergebnis | Fälle |
|---|---:|
| Mit gelieferten Callbacks abgeschlossen | 5.376 |
| Vor PostInitAnim angehalten | 768 |
| Vor ReplicateAnim angehalten | 768 |

Von insgesamt 6.912 Fällen initialisieren 6.144 den Kanal neu und 768 verwenden ihn weiter. Die unabhängige Prüfung berechnet sämtliche 18 Kanalwörter, Instanzbytes, Ereignisse, Blendzeiten und Fehlergrenzen nach. Sie prüft außerdem die DLL-Konstanten und nativen Callbackinstruktionen. Frühere Berichte werden nur gelesen.

```text
cargo run -p rc-inspect --bin rc-channel-playback-probe -- analysis/reports/mesh-animation-channels.json analysis/reports/channel-playback.json
python scripts/Record-ChannelPlayback.py
```

Nachweise: `analysis/reports/channel-playback.json`, `channel-playback-validation.json`, `analysis/decompiled/mesh-animation-entry.c/.asm` und `analysis/evidence.json`.

217 Workspace-Tests (207 rc-package, 4 rc-native, 6 rc-render), Clippy mit Warnungen als Fehler und Formatprüfung bestanden. Sieben neue Tests prüfen Wiederverwendung, Padding-/Pointererhalt, Rate-null-Übergang, Frame-/Blendwerte, Loopkontinuität, Rand- und Callbackfehler mit Teilschreibzugriffen, None-Siblingweitergabe, PostInit-Änderungen vor Replikation sowie NaN- und ungültige Eingaben. Originalsequenzdecoder, native PostInit-/Replikationsimplementierung, Animations-Ticking, Posen, Skinning und sichtbare Ausgabe bleiben offen. Androidprüfung zum Schluss.
