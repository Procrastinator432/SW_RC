"""Validate prepared intro assets without claiming a live camera reconstruction."""
import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission13-rappel'
GAME = Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData')


def read(name):
    return json.loads((OUT / name).read_text(encoding='utf-8'))


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


curves = read('player-curves.json')
assert curves['source'] == 'Clone.CloneCommandoSPSet:EntranceRope600'
assert curves['frames'] == 141 and curves['rate'] == 30
assert abs(curves['duration'] - 4.7) < 1e-6
assert curves['camera_parity'] is False
samples = curves['samples']
assert len(samples) == 141
for i, sample in enumerate(samples):
    assert sample['frame'] == i and abs(sample['seconds'] - i / 30) < 1e-6
    values = sample['root'] + sample['root_delta']
    values += [v for row in sample['head'] for v in row]
    assert all(math.isfinite(v) for v in values)
assert samples[0]['root_delta'] == [0, 0, 0]
assert abs(samples[-1]['root_delta'][2] + 600) < .001
notifies = read('player-notifies.json')
assert len(notifies) == 7
assert sum(n['class'] == 'Engine.AnimNotify_Sound' for n in notifies) == 5
assert sum(n['class'] == 'Engine.AnimNotify_Footstep' for n in notifies) == 2
assert all(0 <= n['normalized_time'] < 1 for n in notifies)
bindings = read('mesh-bindings.json')
assert len(bindings) == 18
assert {r['label'] for r in bindings} == {'rope', 'trooper', 'warrior'}
for r in bindings:
    assert r['triangles'] > 0
    assert all(math.isfinite(v) for row in r['bounds'] for v in row)
    assert all(r['bounds'][0][i] <= r['bounds'][1][i] for i in range(3))
actors = read('actors.json')['actors']
assert {11, 452, 477, 530, 413, 895, 1070} <= {r['original'] for r in actors}
prior = json.loads((ROOT / 'analysis/mission12-opening/validation-manifest.json').read_text())
for name, row in prior['evidence'].items():
    p = ROOT / name
    assert p.stat().st_size == row['bytes'] and digest(p) == row['sha256'], name
originals = [GAME / 'Maps/geo_01a.ctm']
originals += [GAME / 'Animations' / n for n in ['Clone.ukx', 'miscanimprops.ukx', 'Geonosian.ukx']]
originals += [GAME / 'System' / n for n in ['engine.u', 'gameplay.u', 'ctgame.u', 'ctcharacters.u']]
originals += [GAME / 'Properties/properties.u']
evidence = {
    str(p.relative_to(ROOT)): {'bytes': p.stat().st_size, 'sha256': digest(p)}
    for p in sorted(OUT.glob('*.json')) if p.name != 'validation.json'
}
result = {
    'passed': True, 'stage': 'original asset preparation; no live rappel controller',
    'player_frames': len(samples), 'sampled_pose_bounds': len(bindings),
    'player_sound_notifies': 5, 'player_material_footsteps': 2,
    'previous_delivery_evidence_unchanged': len(prior['evidence']),
    'originals': {str(p): digest(p) for p in originals}, 'evidence': evidence,
    'tool_sha256': digest(ROOT / 'crates/rc-inspect/src/bin/rc-rappel-audit.rs'),
}
(OUT / 'validation.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
print(f"Rappel assets checked: {len(samples)} player frames, {len(bindings)} pose bounds; prior delivery unchanged")
