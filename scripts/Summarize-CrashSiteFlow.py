"""Verify and archive the bounded original crash-site event foundation (no new APK)."""
import hashlib
import json
import re
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission2'

def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def read(name):
    return json.loads((OUT / name).read_text(encoding='utf-8-sig'))

checks = read('check/validation.json')
assert checks['passed'] and checks['death_orders'] == 120
assert checks['timer_rates'] == [30, 60, 120, 240]
assert checks['invalid_assets_rejected'] == 8
for field in ['five_factories', 'duplicate_deaths_ignored', 'invulnerability_gate',
              'events_not_queued', 'parallel_script_tracks', 'invalid_input_unchanged']:
    assert checks[field], field
cross = read('original-crosscheck.json')
assert cross['passed'] and cross['original_script_effect_order']
assert cross['original_completion_effect_order']
assert cross['original_factory_ids'] == [133, 134, 277, 441, 836]
visuals = read('visuals/validation.json')
assert visuals['passed'] and visuals['evaluated_clip_frames'] == 36
assert visuals['models'][0]['source'] == 'droid.BattleDroid'
assert any(c.endswith(':Impact') for c in visuals['models'][0]['clips'])
assert read('landing-regression/validation.json')['passed']
replay = read('legacy-replay.json')
assert replay['entries'] == 1002 and len(replay['runs']) == 6
assert all(run['exact_replay'] for run in replay['runs'])
tests = sum(map(int, re.findall(r'test result: ok\. (\d+) passed; 0 failed',
                               (OUT / 'workspace-tests.log').read_text(encoding='utf-8-sig'))))
assert tests == 986
assert 'Finished' in (OUT / 'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log', 'diff-check.log']:
    assert not (OUT / name).read_text(encoding='utf-8-sig').strip()
previous = json.loads((ROOT / 'analysis/mission1/validation-manifest.json').read_text())
for name, row in previous['evidence'].items():
    path = ROOT / name
    assert path.stat().st_size == row['bytes'] and sha(path) == row['sha256'], name
originals = previous['originals'].copy()
for name, digest in originals.items():
    assert sha(Path(name)) == digest, name
game = Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData')
for name in ['System/ctcharacters.u', 'Properties/Properties.u', 'Animations/droid.ukx']:
    originals[str(game / name)] = sha(game / name)
for model in read('visuals/visual-assets.json').values():
    for material in model['materials']:
        name = material['source'].split('.')[0] + '.utx'
        path = game / 'Textures' / name
        originals[str(path)] = sha(path)
with ZipFile(ROOT / 'analysis/mission1/source-snapshot.zip') as archive:
    names = set(archive.namelist())
for folder in ['crates', 'docs', 'scripts']:
    for path in (ROOT / folder).rglob('*'):
        if path.is_file() and path.suffix in ['.rs', '.md', '.py', '.ps1', '.java', '.toml']:
            names.add(path.relative_to(ROOT).as_posix())
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
for name in cross['inputs'] + ['analysis/reports/sources/System__gameplay.u/Counter.uc',
    'analysis/reports/sources/System__gameplay.u/ACTION_ObjectiveUpdate.uc',
    'analysis/reports/sources/System__ctgame.u/PawnFactory.uc']:
    path = ROOT / name
    evidence[name] = dict(bytes=path.stat().st_size, sha256=sha(path))
manifest = dict(scope='Selected original crash-site event foundation only. Effects are not yet connected to Game, scene collision, rendering, damage or AI. Playable delivery remains 0.19.0/mission1; no new Android acceptance.',
    checks=dict(workspace_library_tests=tests, death_orders=120, timer_rates=[30, 60, 120, 240],
        original_visual_clip_frames=36,
        independent_original_crosscheck=True, clippy=True, format=True, landing_regression=True,
        legacy_replay_commands=1002, previous_evidence_verified=len(previous['evidence'])),
    originals=originals, sources=sources, evidence=evidence,
    next_step='Apply effects to scene and collision, physical tracker route, original BattleDroid visuals and combat; then contiguous Android delivery.')
(OUT / 'validation-manifest.json').write_bytes((json.dumps(manifest, indent=2) + '\n').encode())
print(f'Verified {tests} tests, 120 death orders, {len(evidence)} evidence files, {len(sources)} source files; no new APK')
