"""Freeze prepared objective5 evidence; deliberately no playable delivery claim."""
import hashlib
import json
import re
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission6-demolition'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(name):
    return json.loads((OUT / name).read_text(encoding='utf-8-sig'))


prior = json.loads((ROOT / 'analysis/mission5-playable/validation-manifest.json').read_text())
for name, row in prior['evidence'].items():
    path = ROOT / name
    assert path.stat().st_size == row['bytes'] and sha(path) == row['sha256'], name
for name, digest in prior['originals'].items():
    assert sha(Path(name)) == digest, name
assert len(read('original-bindings.json')['actors_and_actions']) == 65
assert len(read('original-bindings.json')['clips']) == 3
assets = read('demolition-assets.json')
assert sum(len(p['actions']) for p in assets['programs']) == 24
assert assets['setup_seconds'] == 10 and assets['navigation'] == 1094
assert len(read('sound-bindings.json')) == 9
flow = read('flow-validation.json')
assert flow['passed'] and len(flow['runs']) == 6
for run in flow['runs']:
    assert run['state']['objective_completed'] == run['detonate']
animation = read('animation-validation.json')
assert animation['passed'] and len(animation['samples']) == 15
assert all(s['visible_pixels'] > 300 for s in animation['samples'])
world = read('world-validation.json')
assert world['passed'] and world['scene_state_unchanged'] and world['normal_prefix_inputs'] == 10448
assert world['destroyed_hulls'] == 23 and all(t['reached'] for t in world['trace'])
legacy = read('legacy-07-regression/validation.json')
assert legacy['passed'] and legacy['restart_exact'] and legacy['input_driven_objectives'] == [0, 1, 2, 3, 4]
tests_log = (OUT / 'workspace-tests.log').read_text(encoding='utf-8-sig')
assert 'FAILED' not in tests_log
tests = sum(map(int, re.findall(r'test result: ok\. (\d+) passed; 0 failed', tests_log)))
assert tests == 1054, tests
clippy = (OUT / 'clippy.log').read_text(encoding='utf-8-sig')
assert 'Finished' in clippy and 'error:' not in clippy
for name in ['format.log', 'source-diff-check.log']:
    assert not (OUT / name).read_text(encoding='utf-8-sig').strip(), name
names = set(prior['sources'])
for folder in ['crates', 'docs', 'scripts']:
    names.update(p.relative_to(ROOT).as_posix() for p in (ROOT / folder).rglob('*')
                 if p.is_file() and p.suffix in ['.rs', '.md', '.py', '.ps1', '.toml', '.c'])
sources = {name: sha(ROOT / name) for name in sorted(names)}
with ZipFile(OUT / 'source-snapshot.zip', 'w', ZIP_DEFLATED) as archive:
    for name in sources:
        archive.write(ROOT / name, name)
with ZipFile(OUT / 'source-snapshot.zip') as archive:
    assert archive.testzip() is None
    for name, digest in sources.items():
        assert hashlib.sha256(archive.read(name)).hexdigest() == digest
evidence = {p.relative_to(ROOT).as_posix(): dict(bytes=p.stat().st_size, sha256=sha(p))
            for p in OUT.rglob('*') if p.is_file() and p.name != 'validation-manifest.json'}
result = dict(passed=True, stage='prepared_objective5', playable_version='0.24.0',
              new_playable_scene=False, new_android_apk=False, live_objective5_complete=False,
              library_tests=tests, original_actions=24, pcm_sounds=9, pose_samples=15,
              source_prefix_inputs=10448, destroyed_bunker_hulls=23,
              prior_evidence_unchanged=len(prior['evidence']),
              evidence=evidence, sources=sources, originals=prior['originals'],
              limitations=['Explicit host arrival; no live Squad GoalQueue/input/animation integration',
                           'Prepared dynamic object; no live mesh/collision swap or passage acceptance',
                           'Blast, factories, voice and sound are emitted requests only',
                           'No live plunger/charge attachment, NPC interruption/resume or difficulty multiplier',
                           'No new Android acceptance; prior objective0-4 scene regression only'])
(OUT / 'validation-manifest.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8', newline='\n')
print(f'Frozen objective5 preparation: {tests} tests, {len(evidence)} evidence files, {len(sources)} sources; prior delivery unchanged')
