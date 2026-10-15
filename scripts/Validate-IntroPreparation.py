"""Freeze intro preparation evidence without claiming a live NPC milestone."""
import hashlib
import json
import re
import struct
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission14-intro'


def read(name):
    return json.loads((OUT / name).read_text(encoding='utf-8-sig'))


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


flow = read('flow-validation.json')
assert flow['passed'] and flow['teleport_and_movement_execution_pending']
assert (flow['rappel_tick'], flow['warrior_anim_end_tick']) == (128, 368)
assert [v[1] for v in flow['warrior_notify_events']] == [190, 191, 192, 193]
assert flow['state']['trooper_destroyed'] and flow['state']['warrior'] == 'Moving'
assert all(v['original'] != 1772 for v in flow['state']['events'])
binding = read('bindings.json')
assert [v['original'] for v in binding['teleport_targets_in_level_order']] == [895, 1070]
assert binding['selected_teleport']['original'] == 895
assert len(binding['audio']) == 6
placement = read('placement-validation.json')
assert placement['passed'] and placement['native_find_spot_control_flow']
assert not placement['native_world_queries'] and not placement['native_far_move_actor']
assert placement['accepted_by_diagnostic_host'] and placement['destination'] == 895
assert placement['extent'] == [50., 50., 115.] and len(placement['queries']) == 4
assert placement['queries'][0]['result']['contacts'][0]['object'] == 'StaticMeshActor43'
assert placement['queries'][-1]['result']['complete']
assert placement['queries'][-1]['result']['contacts'] == []
rise = placement['candidate'][2] - placement['requested'][2]
assert 9.89 < rise < 9.90
expected_constants = [1., .5, 0., 2., .55, .2, 1.1]
for row, value in zip(read('placement-constants.json').values(), expected_constants, strict=True):
    assert row['bits'] == struct.unpack('<I', struct.pack('<f', value))[0]
tests = (OUT / 'workspace-tests.log').read_text(encoding='utf-8-sig')
assert 'FAILED' not in tests and '0 failed' in tests
count = sum(map(int, re.findall(r'test result: ok\. (\d+) passed', tests)))
assert count == 1097, count
clippy = (OUT / 'clippy.log').read_text(encoding='utf-8-sig')
assert 'Finished `release`' in clippy and 'error:' not in clippy
for name in ['format.log', 'whitespace.log']:
    assert not (OUT / name).read_text(encoding='utf-8-sig').strip()
prior = json.loads((ROOT / 'analysis/mission13-rappel/delivery/validation-manifest.json').read_text())
for name, row in prior['evidence'].items():
    path = ROOT / name
    assert path.stat().st_size == row['bytes'] and sha(path) == row['sha256'], name
sources = [
    'crates/rc-package/src/level.rs', 'crates/rc-package/src/lib.rs',
    'crates/rc-package/src/placement.rs', 'crates/rc-game/src/lib.rs',
    'crates/rc-game/src/intro_flow.rs', 'crates/rc-inspect/src/bin/rc-intro-assets.rs',
    'crates/rc-inspect/src/bin/rc-intro-flow-check.rs',
    'crates/rc-inspect/src/bin/rc-intro-placement-check.rs',
    'docs/INTRO_ACTORS.md', 'docs/CONTINUATION_PLAN.md',
    'scripts/Validate-IntroPreparation.py',
]
with zipfile.ZipFile(OUT / 'source-snapshot.zip', 'w', zipfile.ZIP_DEFLATED) as archive:
    for name in sources:
        archive.write(ROOT / name, name)
evidence = [p for p in OUT.iterdir() if p.is_file() and p.suffix in ['.json', '.c', '.asm', '.log', '.zip']
            and p.name != 'validation-manifest.json']
files = [ROOT / name for name in sources] + evidence
originals = Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData')
result = {
    'passed': True, 'stage': 'original intro assets, lifecycle and placement control flow preparation',
    'live_npc_milestone_complete': False, 'current_playable_version': '0.31.0',
    'native_world_query_parity': False, 'native_far_move_actor': False,
    'live_teleport_and_goto_execution': False, 'library_tests': count,
    'diagnostic_target_rise': rise, 'prior_delivery_evidence_unchanged': len(prior['evidence']),
    'whitespace_policy': 'Git cr-at-eol: preserve existing Windows CRLF files',
    'original_sha256': {n: sha(originals / n) for n in ['System/engine.dll', 'System/ctgame.dll', 'Maps/GEO_01A.ctm']},
    'evidence': {p.relative_to(ROOT).as_posix(): {'bytes': p.stat().st_size, 'sha256': sha(p)} for p in files},
}
(OUT / 'validation-manifest.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
print(f'{count} library tests; intro lifecycle and diagnostic placement passed; '
      f'{len(prior["evidence"])} prior evidence files unchanged; live integration remains pending')
