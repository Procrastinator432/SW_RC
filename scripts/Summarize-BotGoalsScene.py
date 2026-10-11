"""Verify and bind M3i delivery to its local source and test receipts."""
import hashlib
import json
import re
import struct
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/milestone3i'


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def read(name):
    return json.loads((OUT / name).read_text(encoding='utf-8-sig'))


difficulty = read('check/game-rules-validation.json')
assert difficulty['passed']
assert len(difficulty['levels']) == 3
for row, factor, expected in zip(difficulty['levels'], [0.25, 1., 1.75], [31.25, 125., 218.75]):
    assert row['factor'] == factor and row['first_hit']['damage'] == expected
    assert row['first_hit']['tick'] == 38
    for field in ['three_kills_9_hits','restart_exact','cadence_exact','neutral_player_and_enemy','damage_error_rolls_back']:
        assert row[field], field
assert read('legacy-lunge/lunge-validation.json')['passed']
lunge = read('lunge-regression/lunge-validation.json')
for field in ['passed','original_root_positions_exact_in_host','death_and_hit_interrupt_settle',
              'lunge2_completed','runtime_collision_cancels_and_settles','cadence_120_30_10_equal',
              'reset_rng_and_motion_exact','projectile_three_kills','defeat']:
    assert lunge[field],field
assert lunge['state']['completed'] >= 1 and lunge['height'] > 5
assert read('legacy-navigation/navigation-validation.json')['passed']
assert read('legacy-ballistics/melee-validation.json')['passed']
assert read('legacy-replay.json')['exact_replay']
assert read('legacy-combat/combat-validation.json')['all_three_enemies_15_hits']
assert read('legacy-melee/melee-validation.json')['passed']
assert read('m2-regression/animation-validation.json')['restart_exact']
assert read('abi-validation.json')['short_buffers_and_canaries']
for level in ['easy','hard']:
    assert read(f'abi-{level}.json')['short_buffers_and_canaries']
android_difficulty = read('android-difficulty/validation.json')
assert android_difficulty['passed'] and len(android_difficulty['levels']) == 3
for actual, expected in zip(android_difficulty['levels'], difficulty['levels']):
    assert actual['level'] == expected['level'] and actual['defeat_tick'] == expected['defeat_tick']
    assert actual['scene_sha256'] == sha(OUT / f"geo_01a-{actual['level']}.rcplay")
    for field in ['original_headless_tick_exact','normal_picker','restart','no_crash']:
        assert actual[field], field
assert read('bot-check/bot-goals-validation.json')['passed']
for row in read('bot-check/bot-goals-validation.json')['levels']:
    assert row['original_budget'] == 1 and row['max_attack_time'] == 2.
    assert row['native_goal_seen'] and row['synthetic_three_uses_six_seconds'] and row['restart_exact']
assert read('bot-goals-receipt.json')['burst_count'] == 1
assert read('bot-goals-receipt.json')['max_attack_time'] == 2.
assert read('legacy-difficulty/game-rules-validation.json')['passed']
provenance = json.loads((ROOT/'analysis/milestone3h/game-rules-receipt.json').read_text())['provenance']
assert provenance['player_defense_defaults']['MyDamageVariance']['value']['value']['path'].lower() == 'properties.dv_organicctplayerdamage'
assert provenance['player_defense_defaults']['ShieldRechargeDelay']['value']['value'] == 3.
assert provenance['player_defense_defaults']['ShieldRechargeRate']['value']['value'] == 20.
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
assert graphics['scene_sha256'] == sha(OUT / 'geo_01a-normal.rcplay')
log = (OUT / 'workspace-tests.log').read_text(encoding='utf-8-sig')
tests = sum(map(int, re.findall(r'test result: ok\. (\d+) passed; 0 failed', log)))
assert tests == 976
assert 'Finished' in (OUT / 'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log', 'diff-check.log']:
    assert not (OUT / name).read_text(encoding='utf-8-sig').strip()
assert 'Verifies' in (OUT / 'apksigner.log').read_text(encoding='utf-8-sig')
assert 'Verification succesful' in (OUT / 'zipalign.log').read_text(encoding='utf-8-sig')
apk = OUT / 'republic-commando-0.17.0.apk'
assert sha(apk) == sha(ROOT / 'android/app/build/outputs/apk/debug/app-debug.apk')
for name in ['android-gpu/validation.json', 'android-cpu/validation.json',
             'graphics/validation.json', 'android-difficulty/validation.json',
             'abi-validation.json', 'abi-easy.json', 'abi-hard.json',
             'apksigner.log', 'zipalign.log', 'apk-metadata.log']:
    assert (OUT / name).stat().st_mtime >= apk.stat().st_mtime, f'Stale binary receipt: {name}'
metadata = (OUT / 'apk-metadata.log').read_text(encoding='utf-8-sig')
assert "versionCode='19'" in metadata and "versionName='0.17.0-bot-goals-scene'" in metadata
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
with ZipFile(ROOT / 'analysis/milestone3h/source-snapshot.zip') as previous:
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
originals = json.loads((ROOT / 'analysis/milestone3h/validation-manifest.json').read_text())['originals']
for path, expected in originals.items():
    assert sha(Path(path)) == expected
game = Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData')
for name in ['Maps/geo_01a.ctm','System/ctgame.dll']:
    originals[str(game / name)] = sha(game / name)
for name in ['System/user.ini', 'System/system.ini']:
    originals[str(game / name)] = sha(game / name)
for folder, name in [('System__ctcharacters.u','PlayerCommando.uc'), ('System__engine.u','GameInfo.uc'), ('System__engine.u','GameRules.uc')]:
    path=ROOT/'analysis/reports/sources'/folder/name
    originals[str(path)]=sha(path)
for name in ['Weapon.uc','Projectile.uc','Ammunition.uc','PlayerController.uc']:
    path=ROOT/'analysis/reports/sources/System__engine.u'/name
    originals[str(path)]=sha(path)
path=ROOT/'analysis/reports/sources/System__ctgame.u/CTPlayer.uc'
originals[str(path)]=sha(path)
previous=json.loads((ROOT/'analysis/milestone3h/validation-manifest.json').read_text())
for folder, name in [('System__ctgame.u','CTBot.uc'), ('System__ctgame.u','GOAL_MeleeAttack.uc'), ('System__ctcharacters.u','GeonosianWarriorBot.uc')]:
    path=ROOT/'analysis/reports/sources'/folder/name
    originals[str(path)]=sha(path)
for name in ['geo_01a-normal.rcplay','geo_01a-easy.rcplay','geo_01a-hard.rcplay','republic-commando-0.16.0.apk']:
    path=ROOT/'analysis/milestone3h'/name
    assert sha(path)==previous['evidence'][path.relative_to(ROOT).as_posix()]['sha256']
evidence = {p.relative_to(ROOT).as_posix(): dict(bytes=p.stat().st_size, sha256=sha(p))
            for p in OUT.rglob('*') if p.is_file() and p.name != 'validation-manifest.json'}
manifest = dict(version='0.17.0-bot-goals-scene', version_code=19,
                scope='Selected Geonosian BurstCount and prepared melee-goal float clock/decision order; bounded scheduler and geometry adapters; no full GoalQueue/defensive jump/RNG parity; M3 remains open; physical ARM64 acceptance pending',
                checks=dict(workspace_library_tests=tests, clippy=True, format=True,
                            legacy_replay_commands=1002, android_gpu=True, android_cpu=True,
                            graphics_context_restore=True, abi=True, source_snapshot_verified=True, android_all_three_difficulties=True),
                sources=sources, evidence=evidence, originals=originals, elf=elf, difficulty_levels=3,
                next_step='Remaining Goal/RNG dependencies and defensive jump, then aim-error/assistance and FX/audio')
(OUT / 'validation-manifest.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
print(f'Verified {tests} tests, both Android paths and {len(sources)} archived source files')
