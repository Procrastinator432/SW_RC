"""Record the tested timer milestone, keeping earlier original-map evidence historical."""
from pathlib import Path
import hashlib
import json

root = Path(__file__).resolve().parents[1]
sources = ["analysis/decompiled/pawn-physics-dispatch.c", "analysis/reports/sources/System__engine.u/Pawn.uc", "crates/rc-package/src/crouch_motion.rs", "crates/rc-package/src/pawn_motion.rs"]
result = {
    "date": "2026-10-06", "rust_tests": 127,
    "native_function": "APawn.performPhysics104955f0",
    "timer_offset": "Pawn+0x650", "expiry_flag_mask": "0xfffffff5",
    "countdown_conditions": ["Walking", "WantsToCrouch", "CanCrouch", "already IsCrouched", "TryToUncrouch"],
    "arithmetic": "finite caller-supplied f32 UncrouchTime minus dt cast to f32, no clamp; <=0 clears Wants and Try",
    "integration_tests": ["uncrouch_timer_expires_after_f32_countdown_and_stands_after_motion", "uncrouch_timer_gates_preserve_inactive_snapshot", "expired_uncrouch_timer_clears_flags_even_when_standing_is_blocked"],
    "checks": ["cargo test --workspace", "cargo clippy --workspace --all-targets -- -D warnings", "cargo fmt --all -- --check"],
    "original_map_runs_this_milestone": 0,
    "limits": "Timer arming by other functions, native callbacks, dynamic bases, cylinder collision and full x87/SSE runtime parity absent; original-map reports remain previous inactive-timer milestone.",
    "android": "deferred until end per user",
    "source_sha256": {s: hashlib.sha256((root/s).read_bytes()).hexdigest() for s in sources},
}
(root/"analysis/reports/uncrouch-timer-validation.json").write_text(json.dumps(result, indent=2)+"\n", encoding="utf-8")
path = root/"analysis/evidence.json"
evidence = json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"] = 127
evidence["uncrouch_timer_validation"] = result
path.write_text(json.dumps(evidence, indent=2, ensure_ascii=False)+"\n", encoding="utf-8")
readme = root/"README.md"
readme.write_text(readme.read_text(encoding="utf-8-sig").replace("124 Rust-Unit-Tests", "127 Rust-Unit-Tests").replace("124Tests bestehen", "127Tests bestehen")+"\nUncrouchTime-Countdown in den Duckbewegungspfad integriert; 127 Tests bestehen. Timer-Snapshot und Flags werden in nativer Reihenfolge verarbeitet; automatische Timerinitialisierung bleibt offen. Details: [CROUCH_MOTION.md](docs/CROUCH_MOTION.md).\n", encoding="utf-8")
note = "2026-10-06: UncrouchTime-Countdown in VolumeWorld.crouching_script_tick integriert. performPhysics104955f0 dekrementiert Pawn+0x650 nur bei Walking/Wants/Can/bereitsCrouched/Try; erster Duck-Schritt überspringt Timer. f32-Snapshot minus dt; <=0 löscht Wants/Try, kein Clamp. Duckbewegung im Ablauf-Tick, danach Aufstehen; bei Blockade geduckt mit gelöschten Flags, erneuter Versuch bei duck0. Gehaltener Duckinput kann im folgenden Tick erneut Ducken anfordern. Drei neue Integrationstests für Countdown/Ablauf/Reentry, alle Gates und blockiertes Aufstehen/Retry. Früherer Timer-Rejection-Test prüft nun NaN.127 Workspace-Tests, Clippy und Format bestanden. Keine neuen Originalkartenläufe; frühere210Ticks bleiben historische Evidenz mit inaktivem Timer. Neuer Nachweis analysis/reports/uncrouch-timer-validation.json mit Quellhashes. Automatische Timerinitialisierung durch andere Enginefunktionen, Callbacks und vollständige native Laufzeitparität bleiben offen. Android weiterhin zum Schluss, kein APK-Build/Emulatorlauf. Details: D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\CROUCH_MOTION.md."
wiki = Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md", "architecture/porting.md", "log.md"):
    with (wiki/relative).open("a", encoding="utf-8") as output:
        output.write("\n\n"+note+"\n")
print("Recorded timer evidence and project notes.")
