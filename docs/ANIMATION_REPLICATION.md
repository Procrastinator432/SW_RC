# Animationsreplikation und PostInit-Grenze

Folgemeilenstein: [GetMoveCoords und aktive Animationskanäle](MOVE_COORDINATES.md) bestimmt das Skelett-Vtableziel +e0 als GetMoveCoords und ergänzt die Auswahl für relative=false. Der frühere Bericht bleibt unverändert; dessen Bezeichnung GetRootLocation beschreibt eine damalige Hostgrenze.

`animation_replication.rs` ergänzt den skalaren Ablauf von AActor.ReplicateAnim (`103ba470`) und das Gate von USkeletalMeshInstance.PostInitAnim (`104ffde0`). Actorflags, Levelzustand, Sequenzauflösung und Framekonvertierung werden ausdrücklich geliefert. Netzwerktransport, tatsächliche Mesh-Instanzen und Matrixberechnung werden nicht daraus abgeleitet.

## Replikationsvoraussetzungen

Der Originalcode prüft zuerst, ob der Replikationsslot kleiner als 6 ist. Bei notify=true folgen der Actorflagtest `flags(+68) & 0x10000`, das Levelbyte bei `Actor.Level(+94) +440` und die vorzeichenbehaftete Kanalnummer kleiner als 3. Jeder Abbruch erfolgt vor Sequenzauflösung und Actor-Schreibzugriffen. notify=false überspringt diese drei Gates.

Die native Slotprüfung hat keine Untergrenze: Negative Slots würden in frühere Actorfelder schreiben. Der Rust-Adapter liefert dafür ausdrücklich einen Fehler, da nur die sechs Slots modelliert sind. Ein negativer Kanalwert hingegen passiert die originale Kanalprüfung und wird beim Packen auf sein niederwertiges Byte verkürzt.

Anschließend wird geprüft, ob die MeshInstance von ULodMeshInstance abstammt. Nur dann wird GetSequence mit dem separat gelieferten Kanalindex aufgerufen und gegebenenfalls dessen Sequenzname gelesen; andernfalls wird None (Handle 0) gepackt. Diese Klassen-/Sequenzprüfung ist ein Hostvertrag, keine anhand der synthetischen Kanäle erratene Originalinstanz.

GetSequence (`1044faf0`) liefert das gespeicherte Sequenzfeld des Kanals bei +44. Im Editormodus führt es vorher die virtuelle Mesh-Sequenzsuche (+ac) mit dem Kanalnamen und load=false aus und aktualisiert +44. Dieser mögliche Instanzschreibzugriff bleibt im Host; er ist nicht mit einer Actor-Slotänderung gleichzusetzen.

## Paketfelder und Quantisierung

Jeder Actor-Slot besteht aus acht Bytes: vier Bytes Sequenzhandle und vier gepackten Bytes. Der ursprüngliche Slot 0 beginnt bei Actor +23c.

| Feld | Herkunft |
|---|---|
| Sequence | Ergebnis der LOD-Sequenzauflösung, sonst Handle 0 |
| Bone | Niederwertiges Byte des Kanalworts +3c |
| Channel/Loop | Niederwertiges Byte von +0c, OR 0x80 bei gesetztem Loop-Byte +04 |
| Rate | Quantisierte Rate aus +18, oder Sentinel 124 bei +24=0 |
| Frame | Quantisierter Frame aus +1c |

Ist das Floatfeld +24 exakt null (einschließlich −0), verwendet Rate den Wert 124. Andernfalls wird die Rate auf [−4,4] begrenzt, danach werden in einzelnen f32-Operationen 4 addiert und mit 31 multipliziert. Der native Konvertierungshelfer schneidet den Bruchteil ab. Das Resultat liegt zwischen 0 und 248. Ungeordnete Vergleiche führen bei NaN zur oberen Grenze 4. Die Ratekonvertierung ist im Adapter umgesetzt.

Frame wird auf [−1,1] begrenzt; NaN führt anhand der COMISS-Sprünge zu 1. Die anschließende native Rechnung lädt diesen f32-Wert in x87, addiert 1, multipliziert mit 127 und konvertiert mit Abschneiden. Der tatsächliche Prozess-Controlword/Präzisionsmodus ist noch nicht belegt. Daher bleibt diese Konvertierung ein expliziter Hostaufruf. `diagnostic_frame_binary64` liefert ausschließlich eine ausdrücklich gewählte binary64-Diagnosevariante; sie behauptet keine allgemein bitgleiche x87-Arithmetik bei sehr kleinen Werten.

Alle Konstanten wurden direkt aus engine.dll geprüft: 0, 124, −4, 4, 31 und 127. Der lokale x87-Konvertierungshelfer `10621ef4` wurde einschließlich FISTP und anschließender Differenzkorrektur exportiert. Der Adapter ersetzt ihn nicht durch eine ungeprüfte Rundungsannahme.

## Gleichheitsprüfung und Schreiben

Bei notify=true vergleicht das Original ausschließlich Sequence, Bone, Channel/Loop und Rate mit dem vorherigen Slot. Frame wird dabei ausgelassen. Sind diese vier Felder gleich, kehrt die Funktion zurück: Auch ein neu berechneter anderer Frame wird dann nicht geschrieben. Sequenzauflösung und Framekonvertierung erfolgen dennoch vor dieser Gleichheitsprüfung.

Bei einer relevanten Änderung wird Actorflag +68 um 0x100 ergänzt und der vollständige Slot geschrieben. Bei notify=false wird der Slot immer vollständig geschrieben, ohne Dirtyflag zu ergänzen. Bereits gesetzte Flags bleiben erhalten. Ein Hostfehler vor der Packetbildung verhindert Actor-Schreibzugriffe; eventuelle Nebenwirkungen der Sequenzauflösung sind weiterhin Sache des Hosts.

## PostInitAnim

ULodMeshInstance.PostInitAnim (`103b0780`) ist ein leerer Return. Die Skelettimplementierung ruft dagegen virtuell +e0 mit der persistenten Matrix bei Instanz +128 und einem Nullargument auf. Liefert diese Rootoperation false, endet PostInit sofort; etwaige zuvor erfolgte Matrixschreibzugriffe bleiben erhalten.

Liefert sie true, folgen GetActor (+9c), FMatrix.Inverse, die Actor-Matrixoperation (+e4), Matrixkomposition und das Kopieren von 16 Wörtern zurück nach +128. Dieser Rest bleibt eine ausdrücklich benannte Hostgrenze. Der Rust-Adapter erfindet keine Transformationsmatrix. Ein Fehler erhält die bereits von der Rootoperation geschriebenen Wörter; erst ein vollständig geliefertes Kompositionsergebnis ersetzt die Matrix.

## Prüfung

Die Probe übernimmt die 768 bisherigen NewSequence-Diagnoseszenarien und prüft je neun Replikationsvarianten. Sequenznamen, Actorflags, Levelresultate, Slots und die binary64-Framekonvertierung sind gelieferte Diagnoseeingaben. Es werden keine Originalreplikation oder tatsächlichen Netzwerkpakete behauptet.

| Ergebnis | Fälle |
|---|---:|
| ActorDisabled | 768 |
| LevelDisabled | 768 |
| ChannelDisabled | 768 |
| SlotOutOfRange | 768 |
| Written | 2.304 |
| Unchanged trotz geändertem Frame | 768 |
| Offene x87-Framekonvertierung | 768 |

Drei weitere Szenarien prüfen PostInit ohne Root, mit offener Matrixfortsetzung und mit ausdrücklich gelieferter Komposition. Die unabhängige Prüfung berechnet sämtliche Actor-Slots, Flags und Aufrufreihenfolgen nach. Originalkonstanten und maßgebliche Assemblerstellen werden verglichen. Frühere Diagnoseberichte bleiben unverändert.

```text
cargo run -p rc-inspect --bin rc-animation-replication-probe -- analysis/reports/channel-playback.json analysis/reports/animation-replication.json
python scripts/Record-AnimationReplication.py
```

Nachweise: `analysis/reports/animation-replication.json`, `animation-replication-validation.json`, `analysis/decompiled/animation-post-replicate.c/.asm`, `animation-replication-support.c/.asm` und `analysis/evidence.json`.

223 Workspace-Tests (213 rc-package, 4 rc-native, 6 rc-render), Clippy mit Warnungen als Fehler und Formatprüfung bestanden. Sechs neue Tests prüfen Gate-Reihenfolge, ungültige Slots, notify=false, Frame-only-Unterdrückung, Rate-/Frame-Clamps und NaN, niederwertige Paketbytes, None-Sequenz, Fehler vor Actorwrites sowie die PostInit-Matrixgrenze. Originalclips, echte Sequenz-/Klassenauflösung, x87-Prozessmodus, Root-/Matrixberechnung, Netzwerktransport und sichtbare Animationen bleiben offen. Androidprüfung weiterhin zum Schluss.
