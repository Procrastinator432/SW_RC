"""Record tested own post-step static AI contact routing and its timing limit."""
from pathlib import Path
import hashlib
import json

root=Path(__file__).resolve().parents[1]
sources=["crates/rc-package/src/ai_contact.rs","crates/rc-package/src/pawn_velocity.rs","crates/rc-package/src/pawn_motion.rs","analysis/decompiled/pawn-walking-dispatch.c"]
result={"date":"2026-10-06","rust_tests":147,"entry":"VolumeWorld.ai_crouching_contact_tick","scope":"Own post-step static Walking contact routing to explicit controller-snapshot HitWall/recovery adapter; first approaching non-floor contact only, successful step-up excluded. Native within-step timing/remaining time/events/dynamic actors/AI steering not implemented.","velocity_policy":"Use computed pre-collision planned_velocity for event stationary gate; preserve collision-clipped final velocity in top-level state. Internal planned_velocity is serde-skipped.","metadata":"Explicit per-object excluded-class and NotifyHitWall results required; missing entry discards entire result.","tests":["static_wall_contact_arms_crouch_even_when_end_velocity_is_zero","human_contact_is_dispatched_but_never_arms_ai_crouch","floor_only_and_airborne_ai_ticks_do_not_require_wall_metadata","missing_contact_metadata_discards_completed_ai_motion_candidate"],"checks":["cargo test --workspace","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check"],"source_sha256":{s:hashlib.sha256((root/s).read_bytes()).hexdigest() for s in sources},"original_map_runs_this_milestone":0,"android":"deferred until end per user"}
(root/"analysis/reports/ai-contact-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
path=root/"analysis/evidence.json"
evidence=json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"]=147
evidence["ai_contact_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
path=root/"README.md"
text=path.read_text(encoding="utf-8-sig").replace("143 Rust-Unit-Tests","147 Rust-Unit-Tests")
if "Statische KI-Kontaktweiterleitung" not in text:
    text+="\nStatische KI-Kontaktweiterleitung ergänzt: eigener Physics-Schritt->erster Wandkontakt->HitWall/Recovery->Duckanforderung im nächsten Tick. Eingehende Velocity wird vor Kollision erhalten, Endgeschwindigkeit bleibt begrenzt; explizite Wallmetadaten erforderlich.147Tests bestehen. Native Kontaktzeitpunkte/Callbacks weiterhin offen. [Details](docs/AI_CONTACT.md).\n"
path.write_text(text,encoding="utf-8")
for relative in ("docs/HIT_WALL_RECOVERY.md","docs/CROUCH_PHYSICS.md"):
    path=root/relative
    text=path.read_text(encoding="utf-8-sig")
    if "## Folgearbeit: statische Kontaktweiterleitung" not in text:
        text+="\n## Folgearbeit: statische Kontaktweiterleitung\n\nEin zusätzlicher Diagnosepfad verbindet jetzt den eigenen vollständigen Physics-Schritt mit dem ersten berechtigten statischen Wandkontakt und der HitWall-/Recovery-Prüfung. Eingehende Velocity für den Ereignisgate, kollisionsbegrenzte Endvelocity für die Zustandsübernahme; Wallmetadaten ausdrücklich erforderlich.147Tests bestehen. Die Weiterleitung erfolgt nach dem Schritt, native innerhalb-des-Schritts-Reihenfolge bleibt offen. Siehe [AI_CONTACT.md](AI_CONTACT.md).\n"
    path.write_text(text,encoding="utf-8")
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
note="2026-10-06: Statische KI-Kontaktweiterleitung VolumeWorld.ai_crouching_contact_tick ergänzt. EigenePostTickPolicy: vollständigePhysics, ersterapproachingNonFloorWalkingKontakt, keinSuccessfulStepUp, HitWall/RecoverymitexplizitenController/WallSnapshots, atomischeGesamtrückgabe. Incomingplanned_velocityvorKollisionfürStationaryGate; TopstatebehältkollisionsbegrenzteEndvelocity, dispatch.stateSnapshotenthältincoming. planned_velocityserde(skip), alteJSONFelderunverändert. VierTests: echterÜberhangKontaktincoming10/end0aktiviertTimer->nächsterTickduckt/bewegt; HumanSkip; Floor/FallingohneMetadaten; fehlendeWallmetadatenverwerfenPhysicsKandidat.147Tests/Clippy/Formatbestanden. physWalking10492b60 zurTiminganalyseexportiert, keinevollständigenativeKontakt-/Callbackparität. KeineOriginalkartenläufe/CLI-Aktivierung, AndroidzumSchluss. Nachweis analysis/reports/ai-contact-validation.json. Details: D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\AI_CONTACT.md."
for relative in ("index.md","architecture/porting.md","log.md"):
    path=wiki/relative
    if "Statische KI-Kontaktweiterleitung VolumeWorld.ai_crouching_contact_tick" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as output:
            output.write("\n\n"+note+"\n")
print("Recorded 147 tests and own static AI contact dispatch milestone.")
