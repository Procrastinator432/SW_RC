"""Record the verified first-attempt controller policy without claiming full AI movement."""
from pathlib import Path
import hashlib
import json

root=Path(__file__).resolve().parents[1]
sources=["analysis/decompiled/pawn-hit-wall.c","analysis/decompiled/pawn-auto-crouch.asm","analysis/reports/sources/System__engine.u/Controller.uc","analysis/reports/sources/System__engine.u/Pawn.uc","crates/rc-package/src/hit_wall.rs"]
native=(root/sources[0]).read_text()
assert "CanCrouchWalk(this,pFVar1" in native and "IsHumanControlled(this)" in native
assert "this + 0x28c" in native and "0x4000000" in native and "iVar8 + 0x2c4" in native
result={"date":"2026-10-06","rust_tests":135,"native_function":"APawn.processHitWall104900b0","implemented_scope":"Explicit controller-snapshot gates, destination/wall angle calculation and first CanCrouchWalk attempt only; no event execution, vertical recovery, second attempt, automatic contact dispatch or full AI movement.","arithmetic":"f64 normalization; native Walking XY projection and strict dot>MinHitWall filter; no rsqrtss/f32 parity claim","tests":["destination_drives_flattened_walking_probe_despite_opposite_velocity","controller_gates_skip_queries_and_preserve_state","angle_equality_and_zero_destination_are_not_filtered","blocked_attempt_requires_recovery_and_unknown_or_invalid_input_is_atomic_error"],"checks":["cargo test --workspace","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check"],"source_sha256":{s:hashlib.sha256((root/s).read_bytes()).hexdigest() for s in sources},"original_map_runs_this_milestone":0,"android":"deferred until end per user"}
(root/"analysis/reports/hit-wall-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
path=root/"analysis/evidence.json"
evidence=json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"]=135
evidence["hit_wall_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
path=root/"README.md"
text=path.read_text(encoding="utf-8-sig").replace("131 Rust-Unit-Tests","135 Rust-Unit-Tests")
if "Erster Auto-Crouch-Versuch bei Wandkontakt" not in text:
    text+="\nErster Auto-Crouch-Versuch bei Wandkontakt mit nativen Controllerbedingungen verbunden: Zielrichtungsfilter, Notify-Snapshot, nur nicht menschliche Walking-Figur. Bei Blockade ausdrücklich RecoveryRequired; keine vollständige KI-/HitWall-Verarbeitung.135Tests bestehen. [Details](docs/HIT_WALL.md).\n"
path.write_text(text,encoding="utf-8")
path=root/"docs/AUTO_CROUCH.md"
text=path.read_text(encoding="utf-8-sig")
if "## Folgearbeit: erste Controllerprüfung" not in text:
    text+="\n## Folgearbeit: erste Controllerprüfung\n\nDie Controllerbedingungen, Ziel-/Wandwinkel und erste CanCrouchWalk-Prüfung sind jetzt in einem expliziten Snapshot-Adapter verbunden. Recovery und zweite Prüfung bleiben offen; keine automatische Gameplay-Einbindung.135Tests bestehen. Siehe [HIT_WALL.md](HIT_WALL.md).\n"
path.write_text(text,encoding="utf-8")
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
note="2026-10-06: Erste Controllerprüfung vor Auto-Crouch in StaticBodyWorld.hit_wall_auto_crouch_first_attempt umgesetzt. Native processHitWall104900b0 prüft Wall/excluded/direct/controller/Velocity0, Destinationrichtung und Walking-XY-Normale, strictDot>MinHitWall, NotifyHandled, Walking/nonhuman/Can/notCrouched; Segment Location+Radius*Zielrichtung anCanCrouchWalk. Scriptkommentar nenntVelocity, nativerBodybenutztDestination. Armed übernimmtTimerflags, BlockadeexplizitRecoveryRequired; nativeVertikalbewegung/zweiterVersuch/Callbacks/Kontaktdispatch fehlen. VierneueTests einschließlichelfSkipGates, entgegengesetzterVelocity, Winkelgleichheit/Nullziel, Blockade/Unknown.135Tests/Clippy/Formatbestanden. Nachweis analysis/reports/hit-wall-validation.json mitQuellhashes. KeinvollständigerKI-Bewegungspfad: PlayerWalkingduck0 darfKI-Wantsnichtlöschen, eigenerPhysics-Einstiegnoch nötig. KeineOriginalkartenläufe/Androidprüfung. Details: D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\HIT_WALL.md."
for relative in ("index.md","architecture/porting.md","log.md"):
    path=wiki/relative
    if "Erste Controllerprüfung vor Auto-Crouch" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as output:
            output.write("\n\n"+note+"\n")
print("Recorded first HitWall auto-crouch policy and 135-test milestone.")
