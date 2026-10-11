"""Bind the original detonator/gun-position foundation without claiming scene integration."""
import hashlib
import json
import re
import wave
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission4-detonator'
GAME = Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData')

def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def read(name):
    return json.loads((OUT/name).read_text(encoding='utf-8-sig'))

oracle = read('toss-oracle.json')
toss = read('toss-validation.json')
assert toss['passed'] and toss['bit_exact'] and toss['cases'] == len(oracle['cases']) == 2052
assert oracle['binary_sha256'] == sha(GAME/'System/engine.dll')
assert oracle['assembly_sha256'] == sha(OUT/'toss-instructions.txt')
assert oracle['inheritance_constant']['bits'] == 0x3ecccccd
flow = read('flow-validation.json')
for field in ['passed','original_projectile_defaults','contact_shortens_fuse_once',
              'lifetime_without_explosion','once_only_explosion','grenade_class_contact_only',
              'objective_before_delayed_geometry','script_damage_50000_for_49000_health',
              'hatch_and_collision_effects','confirmation_timing','eight_original_pcm_samples']:
    assert flow[field], field
visual = read('visual-validation.json')
assert visual['passed'] and visual['snapshot_roundtrip']
assert visual['triangles'] == visual['textured_faces'] == 352 and visual['untextured_faces'] == 0
assert visual['mesh'] == 'Weapons.CloneWeapons.ThermalDetonator'
sounds = read('sounds.json')
assert len(sounds) == 8 and {r.get('role') for r in sounds} == {None,'bounce','empty'}
for row in sounds:
    with wave.open(str(OUT/'sounds'/row['file'])) as sample:
        assert abs(sample.getnframes()/sample.getframerate()-row['seconds']) < 1e-9
tests_log = (OUT/'workspace-tests.log').read_text(encoding='utf-8-sig')
assert 'FAILED' not in tests_log
tests = sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed',tests_log)))
assert tests == 1013, tests
assert 'Finished' in (OUT/'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log','diff-check.log']:
    assert not (OUT/name).read_text(encoding='utf-8-sig').strip(), name
previous_path = ROOT/'analysis/mission3-playable/validation-manifest.json'
previous = json.loads(previous_path.read_text())
for name,row in previous['evidence'].items():
    path = ROOT/name
    assert path.stat().st_size == row['bytes'] and sha(path) == row['sha256'], name
originals = dict(previous['originals'])
for name in ['System/engine.dll','System/engine.u','System/ctinventory.u','Properties/properties.u',
             'Maps/geo_01a.ctm','StaticMeshes/Weapons.usx','Textures/WeaponTextures.utx','Sounds/GEN_Sound.uax','Sounds/GEO_Voice.uax']:
    originals[str(GAME/name)] = sha(GAME/name)
for name,digest in originals.items():
    assert sha(Path(name)) == digest, name
names = set(previous['sources'])
for folder in ['crates','docs','scripts','android/app/src/main/java']:
    names.update(p.relative_to(ROOT).as_posix() for p in (ROOT/folder).rglob('*')
                 if p.is_file() and p.suffix in ['.rs','.md','.py','.ps1','.java','.toml'])
sources = {name:sha(ROOT/name) for name in sorted(names)}
with ZipFile(OUT/'source-snapshot.zip','w',ZIP_DEFLATED) as archive:
    for name in sources:
        archive.write(ROOT/name,name)
with ZipFile(OUT/'source-snapshot.zip') as archive:
    assert archive.testzip() is None
    for name,digest in sources.items():
        assert hashlib.sha256(archive.read(name)).hexdigest() == digest
evidence = {p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p))
            for p in OUT.rglob('*') if p.is_file() and p.name != 'validation-manifest.json'}
for name in ['analysis/decompiled/detonator-native.c','analysis/decompiled/detonator-falling.c',
             'analysis/mission3-playable/validation-manifest.json',
             'analysis/reports/sources/System__ctinventory.u/CTProjExplosive.uc',
             'analysis/reports/sources/System__ctinventory.u/GrenadeProj.uc',
             'analysis/reports/sources/System__engine.u/Actor.uc',
             'analysis/reports/sources/System__engine.u/Weapon.uc',
             'analysis/reports/sources/System__engine.u/Trigger.uc']:
    path = ROOT/name
    evidence[name] = dict(bytes=path.stat().st_size,sha256=sha(path))
manifest = dict(stage='original-objective3-detonator-foundation',
    scope='Prepared native toss arithmetic, selected explosive script clocks/reflection/damage arithmetic, original trigger621 and script325/754 core effects with synthetic contacts. Textured original projectile and eight PCM assets prepared. No integrated world trajectory, original animation release, inventory consumer, damage target selection, rendered live grenade, touch throw or playable objective3 acceptance. AActor::physFalling export is evidence for further work, not a completed port.',
    checks=dict(workspace_library_tests=tests,new_script_rule_tests=14,native_toss_cases=2052,bit_exact_prepared_toss=True,
                original_script_bindings=True,textured_projectile_faces=352,original_pcm_samples=8,
                selected_flow=True,clippy=True,format=True,source_snapshot_verified=True,
                prior_delivery_evidence_unchanged=len(previous['evidence']),playable_objective3=False,android_tested=False),
    diagnostics=dict(superseded_initial_test_log='game-tests.log: one incorrect decimal expectation for f32 bounce result; fixed, all 1013 current workspace tests pass.'),
    previous_delivery=dict(version='0.21.0',manifest_sha256=sha(previous_path)),
    originals=originals,sources=sources,evidence=evidence,
    next_step='Port selected AActor::physFalling with collision callbacks, bind animation/input/inventory and visible damage targets, integrate gun-position geometry/audio/scripts, then validate contiguous Android scene.')
(OUT/'validation-manifest.json').write_bytes((json.dumps(manifest,indent=2)+'\n').encode())
print(f'{tests} tests, 2052 native toss cases, 352 textured faces, eight samples; scene integration OPEN; {len(evidence)} evidence files bound')
