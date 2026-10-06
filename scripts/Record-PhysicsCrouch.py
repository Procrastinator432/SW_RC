"""Persist completed shared crouch-physics regression evidence."""
from pathlib import Path
import json

root=Path(__file__).resolve().parents[1]
validation=json.loads((root/"analysis/collision/physics-crouch-validation.json").read_text())
assert validation["rust_tests"]==138 and validation["crouch_ticks_unchanged"]==210
path=root/"analysis/evidence.json"
evidence=json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"]=138
evidence["physics_crouch_validation"]=validation
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
path=root/"README.md"
text=path.read_text(encoding="utf-8-sig").replace("135 Rust-Unit-Tests","138 Rust-Unit-Tests")
if "Gemeinsamer Duck-Physics-Einstieg" not in text:
    text+="\nGemeinsamer Duck-Physics-Einstieg erhält KI-Wants und gespeicherte Sprungevents ohne PlayerWalking-Phase. HitWall-Arming bis Timerablauf/Aufstehen synthetisch geprüft; 138 Tests bestehen. Originalkarten210 Duck-,1906 Script- und2340 Legacy-Ticks unverändert (inaktive Timerfelder/Scopebeschreibung aus Vergleich entfernt). [Details](docs/CROUCH_PHYSICS.md).\n"
path.write_text(text,encoding="utf-8")
for relative in ("docs/CROUCH_MOTION.md","docs/HIT_WALL.md"):
    path=root/relative
    text=path.read_text(encoding="utf-8-sig")
    if "## Folgearbeit: gemeinsamer Physics-Einstieg" not in text:
        text+="\n## Folgearbeit: gemeinsamer Physics-Einstieg\n\nEin separater Physics-Einstieg ohne PlayerWalking-Phase erhält jetzt die gespeicherte KI-Duckanforderung und den Pending-Sprungevent. Der Spielerpfad delegiert nach seiner Scriptphase an dieselbe Implementierung. Explizites HitWall-Arming bis Timerablauf/Blockade/Retry synthetisch geprüft,138Tests bestehen. Originalkarten210Duck-/1906Script-/2340Legacy-Ticks unverändert bis auf additive inaktive Timerfelder und Scopebeschreibung. Native Recovery und automatischer Kontaktdispatch bleiben offen. Siehe [CROUCH_PHYSICS.md](CROUCH_PHYSICS.md).\n"
    path.write_text(text,encoding="utf-8")
path=root/"docs/CROUCH_PHYSICS.md"
text=path.read_text(encoding="utf-8-sig")
if "## Originalkarten-Ergebnis" not in text:
    text+="\n## Originalkarten-Ergebnis\n\nDie Regression besteht für geo_01a und dm_hangar:210Duckbewegungs-Ticks,1906Script-Bewegungs-Ticks und2340Legacy-Controller-Ticks sind vollständig identisch zum gespeicherten Vorstand. Nur neu hinzugefügte uncrouch_time0-Felder und die aktualisierte Scopebeschreibung werden aus dem Vergleich entfernt. Nachweis: analysis/collision/physics-crouch-validation.json; neue Rohberichte *-physics-crouch.json. Die Timeraktivierung selbst bleibt synthetisch geprüft.\n"
path.write_text(text,encoding="utf-8")
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
note="2026-10-06: Gemeinsamer Duck-Physics-Einstieg VolumeWorld.crouching_physics_tick ohne PlayerWalking-Inputphase. Erhält gespeichertes Wants und PendingJump, führt keinen Sprung aus; verarbeitet Shape/Timer/Walking/Falling/Uncrouch atomisch. Spielerscriptpfad delegiert nach eigenerJump-/Duckphase an dieselbePhysics. DreiIntegrationstests: expliziteHitWall-Aktivierung->Ducken->10CountdownTicks->Standbei0,05; DuckbewegungunterDecke->TimerablaufmitgelöschtenFlags->Blockade->Retry; Jumpinput/fehlendeGeometryohneTeilzustand.138WorkspaceTests/Clippy/Formatbestanden. NeueOriginalkartenregression geo_01a/dm_hangar:210Duck-/1906Script-/2340Legacy-Ticks identischzumVorstand, nuradditiveinaktiveuncrouch_time0-FelderundaktualisierteScopebeschreibungausVergleichentfernt. Nachweis analysis/collision/physics-crouch-validation.json und scripts/Validate-PhysicsCrouch.py. OriginalkartenbelegenSpielerpfadregression, KI-Arming/Timer synthetischgeprüft. NativeRecovery/zweiterDuckversuch/automatischerKontakt-Dispatch/ControllerCallbacks/dynamischeActorsweiteroffen. AndroidweiterzumSchluss. Details: D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\CROUCH_PHYSICS.md."
for relative in ("index.md","architecture/porting.md","log.md"):
    path=wiki/relative
    if "Gemeinsamer Duck-Physics-Einstieg VolumeWorld.crouching_physics_tick" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as output:
            output.write("\n\n"+note+"\n")
print("Recorded shared crouch physics and verified original-map regressions.")
