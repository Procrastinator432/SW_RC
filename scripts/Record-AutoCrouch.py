"""Verify native arming constants/exports and record the tested static adapter."""
from pathlib import Path
import hashlib
import json
import struct

root = Path(__file__).resolve().parents[1]
binary = Path(r"D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\engine.dll")
data = binary.read_bytes()
pe = struct.unpack_from("<I", data, 0x3c)[0]
assert data[:2] == b"MZ" and data[pe:pe+4] == b"PE\0\0"
optional = pe+24
assert struct.unpack_from("<H", data, optional)[0] == 0x10b
base = struct.unpack_from("<I", data, optional+28)[0]
count = struct.unpack_from("<H", data, pe+6)[0]
size = struct.unpack_from("<H", data, pe+20)[0]
sections = [struct.unpack_from("<IIII", data, optional+size+i*40+8) for i in range(count)]
constants = []
for address, expected in [(0x1065efc4,0.5),(0x10653578,1.0)]:
    rva = address-base
    offsets = [raw+rva-start for _,start,length,raw in sections if start <= rva and rva+4 <= start+length]
    assert len(offsets) == 1
    offset = offsets[0]
    value = struct.unpack_from("<f", data, offset)[0]
    assert value == expected
    constants.append({"va":f"{address:08x}","file_offset":offset,"bytes":data[offset:offset+4].hex(),"f32":value})
asm = (root/"analysis/decompiled/pawn-auto-crouch.asm").read_text()
assert "104904e5 UNCONDITIONAL_CALL 104900b0 APawn::processHitWall" in asm
assert "10490598 UNCONDITIONAL_CALL 104900b0 APawn::processHitWall" in asm
assert "OR EAX,0xa" in asm and "MOVSS dword ptr [ESI + 0x650],XMM0" in asm
sources = ["analysis/decompiled/pawn-auto-crouch.c","analysis/decompiled/pawn-auto-crouch.asm","analysis/decompiled/pawn-hit-wall.c","crates/rc-package/src/auto_crouch.rs","crates/rc-package/src/crouch.rs"]
result = {"date":"2026-10-06","rust_tests":131,"function":"APawn.CanCrouchWalk1048db00","direct_caller":"APawn.processHitWall104900b0","direct_call_sites":["104904e5","10490598"],"arming_mask":"0xa","timer_offset":"Pawn+0x650","constants":constants,"engine_dll_sha256":hashlib.sha256(data).hexdigest(),"source_sha256":{s:hashlib.sha256((root/s).read_bytes()).hexdigest() for s in sources},"checks":["cargo test --workspace","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check"],"scope":"Native CanCrouchWalk ordering/flags/timer arming with own explicit static AABB segment queries; different trace masks and controller/AI dispatch not implemented. Found native direct callers require non-human-controlled Walking pawn. No automatic player ducking.","original_map_runs_this_milestone":0,"android":"deferred until end per user"}
(root/"analysis/reports/auto-crouch-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
path = root/"analysis/evidence.json"
evidence = json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"] = 131
evidence["auto_crouch_validation"] = result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
path = root/"README.md"
text = path.read_text(encoding="utf-8-sig").replace("127 Rust-Unit-Tests","131 Rust-Unit-Tests")
marker = "CanCrouchWalk-Timeraktivierung rekonstruiert"
if marker not in text:
    text += "\n"+marker+": expliziter statischer Rust-Adapter setzt nach erfolgreichen Queries Wants/Try und0,5Sekunden. Gefundene native Aufrufe gelten für nicht menschlich gesteuerte Figuren; Controller-/KI-Anbindung bleibt offen.131Tests bestehen. [Details](docs/AUTO_CROUCH.md).\n"
path.write_text(text,encoding="utf-8")
path = root/"docs/CROUCH_MOTION.md"
text = path.read_text(encoding="utf-8-sig")
if "## Folgearbeit: Timeraktivierung" not in text:
    text += "\n## Folgearbeit: Timeraktivierung\n\nCanCrouchWalk ist jetzt als expliziter statischer Adapter rekonstruiert und aktiviert Wants/Try sowie Timer0,5. Die gefundene processHitWall-Aufrufbedingung gilt für nicht menschlich gesteuerte Figuren. Controller-/KI-Anbindung und native Tracefilter bleiben offen. Siehe [AUTO_CROUCH.md](AUTO_CROUCH.md).131Tests bestehen; frühere Kartenberichte bleiben historische Nachweise.\n"
path.write_text(text,encoding="utf-8")
wiki = Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
note = "2026-10-06: CanCrouchWalk1048db00 und processHitWall104900b0 exportiert; direkte Aufrufe104904e5/10490598 und Assembler geprüft. CanCrouchWalk verschiebt Segment um CrouchHeight-currentHeight, Punkttrace0x286 ohneActor, Körpertrace0x86 mitTime1. Erfolg OR0xa Wants/Try und UncrouchTime0,5 (1065efc4 ausPE geprüft); keine Körperbewegung/Größenänderung. processHitWall-Calls erfordern im geprüften Zweig nicht menschlich kontrollierte Walking-Figur mitCan und ohneIsCrouched; kein pauschales automatisches Spielerd ucken. Neuer StaticBodyWorld.can_crouch_walk_diagnostic als expliziter Adapter mit eigener vollständiger statischer AABB-Query, ohne native Masken/Controllerdispatch. Vier Tests fürArming/Reset/Shift/Bodyerhaltung, Point-/ShapeBlock, EndkontaktTime1 undDisabled/Unknown/Input.131Tests/Clippy/Format bestanden. Nachweis analysis/reports/auto-crouch-validation.json samtQuell-/DLLhashes. Keine neuen Originalkartenläufe, Android weiterhin zum Schluss. NativeFilter/dynamischeActors/ControllerCallbacks/KI-Einbindung offen. Details: D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\AUTO_CROUCH.md."
note = note.replace("Spielerd ucken","Spielerducken")
for relative in ("index.md","architecture/porting.md","log.md"):
    path = wiki/relative
    if "CanCrouchWalk1048db00 und processHitWall104900b0 exportiert" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as output:
            output.write("\n\n"+note+"\n")
print("Verified 0.5/1.0 native constants, two caller sites, and recorded auto-crouch milestone.")
