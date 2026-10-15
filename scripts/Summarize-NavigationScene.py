"""Verify and bind M3f delivery to its local source and test receipts."""
import hashlib
import json
import re
import struct
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/milestone3f'


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def read(name):
    return json.loads((OUT / name).read_text(encoding='utf-8-sig'))


navigation = read('check/navigation-validation.json')
for field in ['passed','moving_target_replans','cadence_120_30_10_equal','projectile_three_kills',
              'dead_owner_clears_route_and_attack','unsupported_target_waits_bounded_retries','restart_exact']:
    assert navigation[field], field
nav = read('navigation-receipt.json')
assert len(nav['nodes']) == 37 and len(nav['edges']) == 68
assert read('legacy-ballistics/melee-validation.json')['passed']
assert read('legacy-replay.json')['exact_replay']
assert read('legacy-combat/combat-validation.json')['all_three_enemies_15_hits']
assert read('legacy-melee/melee-validation.json')['passed']
assert read('m2-regression/animation-validation.json')['restart_exact']
assert read('abi-validation.json')['short_buffers_and_canaries']
for backend in ['gpu', 'cpu']:
    receipt = read(f'android-{backend}/validation.json')
    assert receipt['backend'] == backend
    for field in ['normal_picker_load', 'touch_aim_and_fire', 'three_kills',
                  'touch_move_and_exit_victory', 'touch_jump', 'enemy_damage_and_defeat',
                  'touch_reload', 'restart', 'pause_frames_exact',
                  'invalid_file_and_recovery', 'no_crash']:
        assert receipt[field], field
graphics = read('graphics/validation.json')
for field in ['actual_context_recreated', 'context_rebuild_pixels_exact',
              'context_rebuild_pose_exact', 'backend_switch_preserves_state',
              'short_reload_memory_growth_bounded', 'no_crash']:
    assert graphics[field], field
assert graphics['scene_sha256'] == sha(OUT / 'geo_01a-navigation.rcplay')
log = (OUT / 'workspace-tests.log').read_text(encoding='utf-8-sig')
tests = sum(map(int, re.findall(r'test result: ok\. (\d+) passed; 0 failed', log)))
assert tests == 969
assert 'Finished' in (OUT / 'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log', 'diff-check.log']:
    assert not (OUT / name).read_text(encoding='utf-8-sig').strip()
assert 'Verifies' in (OUT / 'apksigner.log').read_text(encoding='utf-8-sig')
assert 'Verification succesful' in (OUT / 'zipalign.log').read_text(encoding='utf-8-sig')
apk = OUT / 'republic-commando-0.14.0.apk'
assert sha(apk) == sha(ROOT / 'android/app/build/outputs/apk/debug/app-debug.apk')
metadata = (OUT / 'apk-metadata.log').read_text(encoding='utf-8-sig')
assert "versionCode='16'" in metadata and "versionName='0.14.0-navigation-scene'" in metadata
elf = []
with ZipFile(apk) as archive:
    assert archive.testzip() is None
    for abi in ['arm64-v8a', 'x86_64']:
        for name in ['librc_native.so', 'librc_bridge.so']:
            path = ROOT / 'android/app/src/main/jniLibs' / abi / name
            data = path.read_bytes()
            assert archive.read(f'lib/{abi}/{name}') == data
            assert data[:6] == b'\x7fELF\x02\x01'
            offset = struct.unpack_from('<Q', data, 32)[0]
            size, count = struct.unpack_from('<HH', data, 54)
            alignments = []
            for i in range(count):
                at = offset + i * size
                if struct.unpack_from('<I', data, at)[0] == 1:
                    file_offset, virtual = struct.unpack_from('<QQ', data, at + 8)
                    alignment = struct.unpack_from('<Q', data, at + 48)[0]
                    assert alignment >= 16384 and file_offset % 16384 == virtual % 16384
                    alignments.append(alignment)
            assert alignments
            elf.append(dict(abi=abi, name=name, sha256=sha(path), load_alignments=alignments))

# Extend the previous source inventory; retain all older pinned deliveries.
with ZipFile(ROOT / 'analysis/milestone3e/source-snapshot.zip') as previous:
    names = set(previous.namelist())
for folder in ['crates', 'docs', 'scripts']:
    for path in (ROOT / folder).rglob('*'):
        if path.is_file() and path.suffix in ['.rs', '.md', '.py', '.ps1', '.java', '.toml']:
            names.add(path.relative_to(ROOT).as_posix())
sources = {name: sha(ROOT / name) for name in sorted(names)}
with ZipFile(OUT / 'source-snapshot.zip', 'w', ZIP_DEFLATED) as archive:
    for name in sources:
        archive.write(ROOT / name, name)
with ZipFile(OUT / 'source-snapshot.zip') as archive:
    for name, expected in sources.items():
        assert hashlib.sha256(archive.read(name)).hexdigest() == expected
historical = ROOT / 'analysis/milestone3'
assert sha(historical / 'republic-commando-0.11.0.apk') == '13ac4d57f7ee37d364c8a7f5e62e2b2c74ba15f6a92d4ce38fa056f90f2549b7'
assert sha(historical / 'geo_01a-combat.rcplay') == '7e01e75d61fcb5609e1894dc064aabc4b56c33672c79fb2ddd5e3977c9cca161'
originals = json.loads((ROOT / 'analysis/milestone3e/validation-manifest.json').read_text())['originals']
for path, expected in originals.items():
    assert sha(Path(path)) == expected
game = Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData')
for name in ['Maps/geo_01a.ctm','System/ctgame.dll']:
    originals[str(game / name)] = sha(game / name)
assert (OUT / 'goals.c').is_file() and (OUT / 'melee-goal.asm').is_file()
for name in ['Weapon.uc','Projectile.uc','Ammunition.uc','PlayerController.uc']:
    path=ROOT/'analysis/reports/sources/System__engine.u'/name
    originals[str(path)]=sha(path)
path=ROOT/'analysis/reports/sources/System__ctgame.u/CTPlayer.uc'
originals[str(path)]=sha(path)
previous=json.loads((ROOT/'analysis/milestone3e/validation-manifest.json').read_text())
for name in ['geo_01a-ballistics.rcplay','republic-commando-0.13.0.apk']:
    path=ROOT/'analysis/milestone3e'/name
    assert sha(path)==previous['evidence'][path.relative_to(ROOT).as_posix()]['sha256']
evidence = {p.relative_to(ROOT).as_posix(): dict(bytes=p.stat().st_size, sha256=sha(p))
            for p in OUT.rglob('*') if p.is_file() and p.name != 'validation-manifest.json'}
manifest = dict(version='0.14.0-navigation-scene', version_code=16,
                scope='Playable original-map walking graph and selected AI goal host; Dijkstra/motion/attack count are host adapters; M3 remains open; physical ARM64 acceptance pending',
                checks=dict(workspace_library_tests=tests, clippy=True, format=True,
                            legacy_replay_commands=1002, android_gpu=True, android_cpu=True,
                            graphics_context_restore=True, abi=True, source_snapshot_verified=True),
                sources=sources, evidence=evidence, originals=originals, elf=elf, navigation_nodes=37, navigation_edges=68,
                next_step='Lunge motion and feasibility, then remaining Bot/Goal fields, GameRules and aim-error/assistance integration')
(OUT / 'validation-manifest.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
print(f'Verified {tests} tests, both Android paths and {len(sources)} archived source files')
