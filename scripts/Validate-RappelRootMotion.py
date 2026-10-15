"""Bind native root-motion evidence, original-world fixture and prior mission regression."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission13-rappel'


def read(name):
    return json.loads((OUT / name).read_text(encoding='utf-8-sig'))


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


rust = (OUT / 'matrix-rust.bin').read_bytes()
assert len(rust) == 141 * 64 and rust == (OUT / 'matrix-sse.bin').read_bytes()
assert (OUT / 'matrix-input.bin').stat().st_size == 141 * 128
path = read('world-path.json')
assert path['isolated_original_curve_probe'] and path['native_relative_matrix']
assert not path['native_move_smooth_parity']
assert len(path['rows']) == 141
for row in path['rows']:
    assert row['sweep']['complete'] and not row['sweep']['contacts']
assert path['start_placement']['state'] == path['endpoint_placement']['state'] == 'Free'
floor_drop = path['endpoint_floor']['hit']['fraction'] * 32
assert 6 < floor_drop < 7
regression = read('opening-regression/validation.json')
assert regression['passed'] and regression['normal_inputs_only']
assert regression['opening_ticks'] == 1925 and regression['injury_tick'] == 1397
state = regression['complete_state']
assert state['phase'] == 'Won' and state['health'] == 20
assert state['statistics']['ticks'] == 18565
prior = json.loads((ROOT / 'analysis/mission12-opening/validation-manifest.json').read_text())
for name, row in prior['evidence'].items():
    p = ROOT / name
    assert p.stat().st_size == row['bytes'] and sha(p) == row['sha256'], name
files = [
    'crates/rc-package/src/lib.rs',
    'crates/rc-package/src/root_motion.rs', 'crates/rc-package/src/root_motion_orders.rs',
    'crates/rc-package/src/move_coords.rs', 'crates/rc-game/src/visual.rs',
    'crates/rc-inspect/src/bin/rc-rappel-path-check.rs', 'scripts/Generate-RootMotionOrders.py',
    'scripts/Validate-RappelRootMotion.py', 'docs/RAPPEL_RECONSTRUCTION.md',
]
for name in ['native-root-motion.c', 'native-root-entry.c', 'native-root-motion.asm',
             'relative-orders.json', 'root-motion-sse.rs', 'root-motion-sse.exe',
             'matrix-input.bin', 'matrix-rust.bin', 'matrix-sse.bin', 'world-path.json',
             'opening-regression/validation.json', 'clippy.log']:
    files.append('analysis/mission13-rappel/' + name)
original = Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\engine.dll')
result = {
    'passed': True, 'stage': 'native movement foundations; no live rappel controller',
    'relative_matrices_bit_exact_against_original_sse': 141,
    'world_curve_frames_clear': 141, 'endpoint_floor_drop': floor_drop,
    'prior_mission_inputs': 18565, 'prior_delivery_evidence_unchanged': len(prior['evidence']),
    'original_engine_sha256': sha(original),
    'files': {name: {'bytes': (ROOT / name).stat().st_size, 'sha256': sha(ROOT / name)} for name in files},
}
(OUT / 'root-validation.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
print('141 native matrices bit-exact, 141 world frames clear; prior mission and delivery unchanged')
