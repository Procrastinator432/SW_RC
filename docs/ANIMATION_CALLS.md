# Native Animationsaufrufe

Folgemeilenstein: [Mesh-Animation: Eintritt und Kanalauswahl](MESH_ANIMATION_CHANNELS.md) ergänzt die LOD-Eintrittsbedingungen und Skelett-Kanalauswahl auf gelieferten Diagnosesnapshots. Der hier dokumentierte frühere Bericht bleibt unverändert.

Die Actor-Wrapper für PlayAnim (NativeIndex 259, `104f7160`) und LoopAnim (260, `104f8700`) sind als geprüfte Rust-Adapter ergänzt. `NativeAnimationWrapper::verify` prüft beide Original-UFunctions einschließlich aller fünf NativeParm-Referenzen, Operanden, Offsets, Größen, Flags und Terminator. Die Argumente werden bereits ausgewertet geliefert; ein allgemeiner Bytecodeinterpreter ist weiterhin offen.

| Argument | Ausgelassener Wert | Nachweis |
|---|---|---|
| Bone | None | Nullinitialisierung vor Argumentauswertung |
| Rate | 1,0 | PE-Konstante `10653578`, Bits `3f800000` |
| StartFrame | −1,0 | PE-Konstante `10664fc4`, Bits `bf800000` |
| Channel | 0 | Nullinitialisierung vor Argumentauswertung |

Die beiden ursprünglichen AnimProp-Aufrufe enthalten nur Sequence und EndFunctionParms (`0x16`). Dessen nativer Handler `1012f3b0` schreibt keinen Parameterwert, setzt GPropObject auf null und dekrementiert den Codezeiger. Deshalb lesen alle optionalen Argumentauswertungen erneut dasselbe Endtoken und behalten ihre Initialwerte. Erst der Wrapper überspringt es abschließend. Ein explizites Nothing-Token (`1012f350`) schreibt ebenfalls keinen Ergebniswert. VM-Zeigerbewegung, GPropObject und optionale DebugInfo werden hier nicht ausgeführt.

Beide Wrapper übergeben FPlayAnim virtuell an Actor-Vtableoffset `+0x1b0`. Die belegten Felder liegen bei Sequence `+0x00`, Loop-Byte `+0x04`, Bone `+0x08`, Channel `+0x0c`, Rate `+0x18`, StartFrame `+0x1c`. `+0x10` ist mit 1,0 initialisiert; seine Bedeutung bleibt offen. `+0x14` und Padding nach dem Loop-Byte werden hier nicht initialisiert und erhalten keinen erfundenen Wert. Der Rust-Typ ist eine semantische Beschreibung, kein Speicherlayout und kein natives Serialisierungsformat. Explizite Floatargumente werden ohne Clamp oder Normalisierung einschließlich ihrer Bits weitergegeben.

Für die Basisimplementierung AActor.PlayAnim (`104e9680`) prüft `play_anim_base` den gelieferten Meshzustand. Ohne Mesh wird nur bei gelöschtem Objektflag `0x4000` die Loggrenze aufgerufen, danach kehrt die Funktion zurück. Mit Mesh folgt zuerst MeshGetInstance (Mesh-Vtable `+0xa0`), anschließend wird Actor.MeshInstance bei `+0xd4` erneut gelesen und dessen PlayAnim (`+0xb0`) aufgerufen. Der native Rückgabewert dieses Aufrufs wird ignoriert. Der Host muss diese Grenze einschließlich möglicher Aktualisierung des Actor-Instanzslots einhalten. Ein ungeklärter Hostaufruf liefert einen Fehler; der Adapter behauptet keine erfolgreiche Wiedergabe.

Der Basisadapter darf erst verwendet werden, wenn der tatsächliche virtuelle Zieltyp geklärt ist. Die Probe setzt dieses Ziel ausdrücklich diagnostisch voraus; sie löst weder Overrides noch Mesh- und Clipreferenzen der Originalinstanzen auf. Sie verändert die früheren Zustandswechselberichte nicht.

Die neue Probe liest die zwei Funktionen frisch aus engine.u und verarbeitet alle 256 gespeicherten AnimProp-Anforderungen (128 Play, 128 Loop). Pro Anforderung werden drei gelieferte Szenarien geprüft: ohne Mesh mit Log, ohne Mesh mit gesetztem Flag, mit Mesh bis zur offenen Wiedergabegrenze. Daraus ergeben sich 512 NoMesh-Ergebnisse und 256 angehaltene Mesh-Aufrufe. Diese 768 Fälle sind keine 768 tatsächlichen Spielobjekte; Tick bleibt ein Diagnosename ohne nachgewiesenen Clip.

```text
cargo run -p rc-inspect --bin rc-animation-call-probe -- <GameData> analysis/reports/anim-prop-transitions.json analysis/reports/animation-calls.json
python scripts/Record-AnimationCalls.py
```

Nachweise: `analysis/decompiled/anim-native-wrappers.c/.asm`, `anim-empty-argument.c/.asm`, `analysis/reports/animation-calls.json` und `animation-calls-validation.json`. Das Prüfskript vergleicht Konstanten und ausgewählte Instruktionen direkt mit den DLL-Bytes sowie alle Anfragen und Basisabläufe unabhängig mit dem Bericht.

203 Workspace-Tests (193 rc-package, 4 rc-native, 6 rc-render), Clippy mit Warnungen als Fehler und Formatprüfung bestanden. Fünf neue Tests prüfen Defaultwerte, vollständige Signaturabweichungen, unveränderte Floatbits und negative Channels, Logflag/Instanzreihenfolge, ignorierte Wiedergaberückgaben und Fehlergrenzen. Native Mesh-/Clipwiedergabe und Androidprüfung bleiben offen; Android wird am Schluss geprüft.
