"""Independently check original lifecycle gates, handler lookup and phase writes."""
from pathlib import Path
from collections import Counter
import hashlib
import json

root = Path(__file__).resolve().parents[1]
path = root / "analysis/reports/state-transitions.json"
report = json.loads(path.read_text(encoding="utf-8-sig"))
previous = json.loads((root / "analysis/reports/auto-state-selection.json").read_text())
added = {"scope", "auto_transition_probes", "resolved_auto_exit_probes"}
assert {k: v for k, v in report.items() if k not in added} == {
    k: v for k, v in previous.items() if k != "scope"}
owners = {o["path"].lower(): o for o in report["owners"]}
indices = {o["path"].lower(): i for i, o in enumerate(report["owners"])}
compositions = {c["owner"].lower(): c for c in report["original_mask_compositions"]}
auto = {p["class"].lower(): p for p in report["auto_class_targets"]}
hardcoded = json.loads((root / "analysis/reports/hardcoded-names-proof.json").read_text())
fixed = {e["name"].lower(): e["index"] for e in hardcoded["entries"]}
assert fixed["beginstate"] == 316 and fixed["endstate"] == 317
assert fixed["auto"] == 690
used_names = sorted({child["path"].rsplit(".", 1)[-1].lower()
                     for owner in owners.values()
                     for child in owner["ordered_children"]["children"] if child["is_struct"]})
bindings = {}
next_index = max(fixed.values()) + 1
for name in used_names:
    if name in fixed:
        index = fixed[name]
    else:
        index = next_index
        next_index += 1
    bindings[name] = {"handle": index + 1 if index else 0, "resolved_index": index}


def find_struct(owner_path, name):
    seen = set()
    while owner_path:
        assert owner_path not in seen
        seen.add(owner_path)
        owner = owners[owner_path]
        # Own list entries were prepended into hash chains: last name wins.
        for child in reversed(owner["ordered_children"]["children"]):
            if child["is_struct"] and child["path"].rsplit(".", 1)[-1].lower() == name:
                qualified = owner["path"].split(".", 1)[0] + "." + child["path"]
                return qualified, child["is_function"]
        owner_path = owner["parent"].lower() if owner["parent"] else None
    return None


def find_function(cls, state, event):
    selected = find_struct(state, event)
    if selected is None:
        selected = find_struct(cls, event)
    if selected is None or not selected[1]:
        return None
    return selected[0]


entries = report["auto_transition_probes"]
assert len(entries) == len(auto) == 3170
entry_counts = Counter()
entry_handlers = Counter()
seen = set()
for probe in entries:
    cls = probe["class"].lower()
    assert cls not in seen
    seen.add(cls)
    base = auto[cls]
    selected = base["selected"].lower()
    target = probe["target"]
    assert target["selection"] == base["target"]
    fallback = base["target"]["route"] == "ClassFallback"
    if fallback:
        assert target["name"] == {"handle": 0, "resolved_index": 0}
    else:
        assert target["name"] == bindings[selected.rsplit(".", 1)[-1]]
    wants_begin = not fallback and bool(base["frame"]["probe_mask"] & (1 << 16))
    expected = "NotFound" if fallback else "UnresolvedCallback" if wants_begin else "Success"
    assert probe["outcome"] == expected
    assert probe["execution"] == {
        "probe": base["frame"], "node": indices[selected], "code": None,
        "latent_action": 0, "object_flags": 4096 if expected == "Success" else 0}
    callback = probe["callback_request"]
    if wants_begin:
        handler = find_function(cls, selected, "beginstate")
        assert callback == {"event": "BeginState", "selected_function": handler}
        entry_handlers[handler] += 1
    else:
        assert callback is None
    entry_counts[expected] += 1
assert entry_counts == {"NotFound": 2653, "UnresolvedCallback": 374, "Success": 143}

exits = report["resolved_auto_exit_probes"]
assert len(exits) == 517
exit_counts = Counter()
exit_handlers = Counter()
seen = set()
for probe in exits:
    cls = probe["class"].lower()
    assert cls not in seen
    seen.add(cls)
    base = auto[cls]
    assert base["target"]["route"] == "AutoState"
    selected = base["selected"].lower()
    assert probe["supplied_state"].lower() == selected
    wants_end = bool(base["frame"]["probe_mask"] & (1 << 17))
    expected = "UnresolvedCallback" if wants_end else "NotFound"
    assert probe["outcome"] == expected
    if wants_end:
        handler = find_function(cls, selected, "endstate")
        assert probe["callback_request"] == {"event": "EndState", "selected_function": handler}
        exit_handlers[handler] += 1
        frame, node, code, flags = base["frame"], indices[selected], 123, 8192
    else:
        assert probe["callback_request"] is None
        frame = {"object_class": indices[cls], "state_node": indices[cls],
                 "probe_mask": compositions[cls]["composed_mask"]}
        node, code, flags = indices[cls], None, 4096
    assert probe["execution"] == {
        "probe": frame, "node": node, "code": code, "latent_action": 0, "object_flags": flags}
    exit_counts[expected] += 1
assert exit_counts == {"NotFound": 440, "UnresolvedCallback": 77}

asm = (root / "analysis/decompiled/active-state.asm").read_text()
for instruction in (
    "1013a104 MOV word ptr [EAX + 0x24],BX",
    "1013a232 TEST AH,0x20", "1013a237 AND EAX,0xffffefff",
    "1013a23c OR EAX,0x2000", "1013a246 CALL 0x101066d0",
    "1013a24e AND EDX,0xffffdfff", "1013a256 TEST AH,0x10",
    "1013a25e MOV EAX,0x2", "1013a279 MOV dword ptr [EDX + 0x4],EBX",
    "1013a27f MOV dword ptr [EAX + 0x18],EBX",
    "1013a285 MOV dword ptr [ECX + 0xc],0x0",
    "1013a2d2 MOV EAX,dword ptr [EDX + 0x4f0]",
    "1013a32e AND EBX,0xffffefff", "1013a35c TEST AH,0x10",
    "1013a36b OR EAX,0x1000", "1013a375 MOV EAX,0x1",
):
    assert instruction in asm, instruction
native = (root / "analysis/decompiled/end-state-event.c").read_text()
assert "FindFunctionChecked(this,*(undefined4 *)(_Names + 0x4f4),0)" in native
assert "(*(int *)this + 4)" in native

sources = ["crates/rc-package/src/state_selection.rs", "crates/rc-package/src/state_transition.rs",
           "crates/rc-package/src/event_lookup.rs", "crates/rc-inspect/src/bin/rc-state-link-probe.rs",
           "analysis/decompiled/active-state.c", "analysis/decompiled/active-state.asm",
           "analysis/decompiled/end-state-event.c", "scripts/Record-StateTransitions.py"]
result = {
    "date": "2026-10-07", "rust_tests": 182,
    "native_base": "UObject.GotoState 1013a0c0", "end_state_wrapper": "101066d0",
    "entry_outcomes": dict(entry_counts), "begin_requests": dict(entry_handlers),
    "exit_outcomes": dict(exit_counts), "end_requests": dict(exit_handlers),
    "report_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    "source_sha256": {s: hashlib.sha256((root / s).read_bytes()).hexdigest() for s in sources},
    "checks": ["cargo test --workspace", "cargo clippy --workspace --all-targets -- -D warnings",
               "cargo fmt --all -- --check", "independent original mask gates, ordered handler lookup and phase writes",
               "prior target, mask and lookup reports unchanged", "synthetic recursive callbacks and phase failures"],
    "scope": report["scope"] + " Exit probes use separate supplied resolved-state snapshots, not continuations past unresolved BeginState."
}
(root / "analysis/reports/state-transitions-validation.json").write_text(
    json.dumps(result, indent=2) + "\n", encoding="utf-8")
path = root / "analysis/evidence.json"
evidence = json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"] = 182
evidence["state_transition_validation"] = result
path.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
note = (
    "2026-10-07: Zustandswechsel-Ablauf ergänzt. Einheitlicher Resolver erhält benannte Identität, "
    "ersetzt Auto durch den Kandidatennamen und Fallback durch None. Base GotoState bildet "
    "LatentAction-Löschung, alte EndState-/neue BeginState-Masken, Node/StateNode/Code-Zuweisung, "
    "Rekursion mit 0x2000-Guard und Preemption über 0x1000 ab. Callbacks werden ausdrücklich vom Host "
    "geliefert; unbekannte Originalhandler werden nicht ausgeführt. Originalproben: 3170 Auto-Eintritte "
    "(2653 Fallbacks, 143 ohne Begin-Aufruf, 374 an BeginState angehalten); 517 separat gelieferte "
    "Auto-State-Austritte (440 ohne End-Aufruf, 77 an EndState angehalten). Unabhängig aus Originalmasken "
    "und Kinderlisten geprüft. 182 Workspace-Tests, Clippy und Format bestanden. Keine vollständige "
    "VM-/ProcessEvent-, Laufzeit-Override- oder Spielparität; Metadaten bleiben feste Snapshots. "
    "Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\STATE_TRANSITIONS.md."
)
path = root / "README.md"
text = path.read_text(encoding="utf-8-sig")
if "Zustandswechsel-Ablauf ergänzt" not in text:
    path.write_text(text + "\n" + note + " [Nachweis](docs/STATE_TRANSITIONS.md).\n", encoding="utf-8")
wiki = Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md", "architecture/porting.md", "log.md"):
    path = wiki / relative
    text = path.read_text(encoding="utf-8-sig")
    if "Zustandswechsel-Ablauf ergänzt" not in text:
        with path.open("a", encoding="utf-8") as output:
            output.write("\n\n" + note + "\n")
print(json.dumps({k: result[k] for k in ("entry_outcomes", "exit_outcomes", "rust_tests")}))
