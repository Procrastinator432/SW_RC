"""Dump all distinct lifecycle functions selected by original transition probes."""
from pathlib import Path
from collections import defaultdict
import json
import subprocess

root = Path(__file__).resolve().parents[1]
report = json.loads((root / "analysis/reports/state-transitions.json").read_text())
owners = {o["path"].lower(): o for o in report["owners"]}
groups = defaultdict(set)
handlers = {p["callback_request"]["selected_function"]
            for key in ("auto_transition_probes", "resolved_auto_exit_probes")
            for p in report[key] if p["callback_request"]}
assert None not in handlers
for handler in handlers:
    package = owners[handler.rsplit(".", 1)[0].lower()]["package"]
    groups[package].add(handler.split(".", 1)[1])
output = root / "analysis/reports/lifecycle-functions"
output.mkdir(exist_ok=True)
failed = []
for package, paths in sorted(groups.items()):
    target = output / (Path(package).stem + ".json")
    result = subprocess.run([str(root / "target/debug/rc-script-dump.exe"),
                             package, str(target), *sorted(paths)], cwd=root,
                            capture_output=True, text=True)
    if result.returncode:
        failed.append({"package": package, "error": result.stderr})
    decoded = json.loads(target.read_text())
    print(json.dumps({"package": package, "functions": len(paths), "errors": decoded["errors"]}))
if failed:
    raise SystemExit(json.dumps(failed, indent=2))
