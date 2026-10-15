"""Freeze the map2 integration; do not label map2 mission scripts complete."""
import hashlib
import json
import re
import shutil
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT / 'analysis/mission16-map2'
OUT = WORK / 'delivery'


def read(p): return json.loads(p.read_text(encoding='utf-8-sig'))
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()


play = read(OUT / 'playthrough/validation.json')
win = read(OUT / 'native/validation.json')
android = read(OUT / 'android/native-validation.json')
ui = read(OUT / 'android/validation.json')
assert all(v['passed'] for v in [play, win, android, ui])
assert play['first_map_inputs'] == win['real_map1_inputs'] == android['real_map1_inputs'] == 18730
assert play['early_and_invalid_travel_atomic'] and play['invalid_inventory_atomic']
assert play['restart_exact_state_and_frame'] and play['movement_jump_crouch_fire_reload']
assert win['saved_restore_and_graphics_cache'] and android['saved_restore']
entry = play['entry']; assert entry['health'] == 20 and entry['ammo'] == 13 and entry['reserve'] == 180
assert entry['map']['restored'] and entry['map']['companion']['joined']
assert entry['map']['companion']['health'] == android['scorch_health'] == 100
assert entry['pawn']['body']['position'] == [-10640.3818359375, -2686.11376953125, -808.0003662109375]
assert play['distance_moved'] > 700 and ui['apk_saved_handoff_resume'] and ui['original_weapon_audio']
build = read(OUT / 'build-receipt.json')
assert build['triangles'] == 249736 and build['colliders'] == 348 and build['bytes'] == 32810668
world = read(WORK / 'world/scene.json')
assert len(world['instances']) == 437 and len(world['omitted']) == 8
assert all(v['reason'] == 'bHidden' for v in world['omitted'])
surface = read(WORK / 'surfaces/surface-receipt.json')
assert surface['untextured'] == 12 and len(surface['errors']) == 12
assert all(v['kind'] == 'bsp' and v['material'] == '' for v in surface['errors'])
assert surface['fx_omitted'] == 4559
scene = OUT / 'geo_01b.rcplay'
assert scene.stat().st_size == build['bytes']
for mode in ['known', 'unknown']:
    reader = read(WORK / f'scene-reader-{mode}.json')
    assert reader['passed'] and reader['heap_limit'] == 48*1024*1024 and reader['sha256'] == sha(scene)
tests = (WORK / 'workspace-tests.log').read_text(encoding='utf-8-sig')
count = sum(map(int, re.findall(r'test result: ok\. (\d+) passed', tests)))
assert count == 1097 and 'FAILED' not in tests
assert 'Finished `release`' in (WORK / 'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log', 'whitespace.log']:
    assert not (WORK / name).read_text(encoding='utf-8-sig').strip()
prior = read(ROOT / 'analysis/mission15-complete/delivery/validation-manifest.json')
for name, record in prior['evidence'].items():
    p = ROOT / name
    assert p.stat().st_size == record['bytes'] and sha(p) == record['sha256'], name
with zipfile.ZipFile(OUT / 'republic-commando-0.33.0.apk') as apk:
    assert all(f'lib/{abi}/librc_native.so' in apk.namelist() for abi in ['arm64-v8a', 'x86_64'])
assert "versionCode='37'" in (WORK / 'apk-info.txt').read_text(encoding='utf-8-sig')
for name in ['workspace-tests.log', 'clippy.log', 'format.log', 'whitespace.log', 'native-build.log',
             'android-build.log', 'world-build.log', 'scene-reader-known.json', 'scene-reader-unknown.json', 'apk-info.txt']:
    shutil.copyfile(WORK / name, OUT / name)
shutil.copyfile(WORK / 'surfaces/surface-receipt.json', OUT / 'surface-receipt.json')
shutil.copyfile(WORK / 'world/scene.json', OUT / 'world-receipt.json')
shutil.copyfile(ROOT / 'target/release/rc_native.dll', OUT / 'native/rc_native.dll')
sources = [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', ROOT / 'README.md']
for folder, extensions in [('crates', {'.rs', '.toml'}), ('android', {'.java', '.c', '.xml', '.gradle'}),
                            ('scripts', {'.py', '.ps1', '.c', '.java'}), ('docs', {'.md'})]:
    for p in (ROOT / folder).rglob('*'):
        if p.is_file() and p.suffix in extensions and not {'build', '.gradle', '.cxx'}.intersection(p.relative_to(ROOT / folder).parts):
            sources.append(p)
with zipfile.ZipFile(OUT / 'source-snapshot.zip', 'w', zipfile.ZIP_DEFLATED) as archive:
    for p in sorted(set(sources)): archive.write(p, p.relative_to(ROOT).as_posix())
files = [p for p in OUT.rglob('*') if p.is_file() and p.name != 'validation-manifest.json']
result = dict(passed=True, version='0.33.0', version_code=37,
    map2_environment_and_transition_integrated=True, map2_original_mission_complete=False,
    complete_three_map_mission=False, map1_real_inputs=18730, library_tests=count,
    prior_delivery_evidence_unchanged=len(prior['evidence']),
    original_map_sha256=sha(Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Maps\geo_01b.ctm')),
    limits=['Map2 objectives, original encounters, interactive mover/terminal scripts and combat squad commands pending',
            'Static mover/Karma poses and base diffuse preview; FX omitted and 12 original-null BSP material fallbacks',
            'Full Android ABI travel plus APK persisted-resume smoke, no full manual touch/physical ARM64 acceptance'],
    evidence={p.relative_to(ROOT).as_posix(): dict(bytes=p.stat().st_size, sha256=sha(p)) for p in sorted(files)})
(OUT / 'validation-manifest.json').write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8')
print(f'Map2 0.33.0 frozen: {len(files)} evidence files, {count} tests; '
      f'{len(prior["evidence"])} prior delivery files unchanged; map2 mission scripts remain pending')
