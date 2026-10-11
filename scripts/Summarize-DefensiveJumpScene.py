"""Verify the bounded playable M3l delivery and preserve its exact source inventory."""
import hashlib
import json
import re
import struct
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/milestone3l'


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def read(name):
    return json.loads((OUT / name).read_text(encoding='utf-8-sig'))


def required(receipt, fields):
    for field in fields:
        assert receipt[field], field


rules = read('check/game-rules-validation.json')
assert rules['passed'] and [r['defeat_tick'] for r in rules['levels']] == [1904, 114, 38]
for row in rules['levels']:
    required(row, ['three_kills_9_hits', 'restart_exact', 'cadence_exact',
                   'neutral_player_and_enemy', 'damage_error_rolls_back'])
for folder in ['jump-check', 'jump-easy']:
    jump = read(folder + '/jump-validation.json')
    required(jump, ['passed', 'cadence_exact', 'restart_exact', 'reservations_exclusive',
                    'runtime_collision_failure_settles', 'incomplete_collision_rolls_back',
                    'no_destination_graceful', 'three_kills_9_hits'])
    assert set(jump['stages']) == {'Start', 'Flight', 'Land', 'Sleep'}
    assert jump['natural_post_melee_jump']['completed'] > 0
    assert jump['natural_post_melee_jump']['returns'] > 0
    assert jump['natural_post_melee_jump']['max_height'] > 50
    for row in jump['interruptions']:
        assert row['state']['interrupted'] > 0 and not any(row['state']['motions'])
assert read('legacy-replay.json')['exact_replay']
assert read('legacy-lunge/lunge-validation.json')['passed']
assert [r['defeat_tick'] for r in read('legacy-difficulty/game-rules-validation.json')['levels']] == [360, 159, 38]
trajectory = read('trajectory-validation.json')
assert trajectory['passed'] and trajectory['cases'] == 4240
defensive = read('defensive-validation.json')
assert defensive['passed'] and (defensive['searches'], defensive['plans'], defensive['post_melee']) == (1798, 392, 224)
landing = read('landing-validation.json')
assert landing['passed'] and landing['cases'] == 1504 and landing['translations'] == 2048
assert landing['provenance'] == read('landing-oracle.json')['provenance']
assert landing['translation_provenance'] == read('translation-oracle.json')['provenance']

for backend in ['gpu', 'cpu']:
    receipt = read(f'android-{backend}/validation.json')
    assert receipt['backend'] == backend
    assert sha(Path(receipt['scene'])) == sha(OUT / 'geo_01a-easy.rcplay')
    required(receipt, ['normal_picker_load', 'touch_aim_and_fire', 'three_kills',
                      'touch_move_and_exit_victory', 'touch_jump', 'enemy_damage_and_defeat',
                      'touch_reload', 'restart', 'pause_frames_exact',
                      'invalid_file_and_recovery', 'no_crash'])
graphics = read('graphics/validation.json')
required(graphics, ['actual_context_recreated', 'context_rebuild_pixels_exact',
                    'context_rebuild_pose_exact', 'backend_switch_preserves_state',
                    'short_reload_memory_growth_bounded', 'no_crash'])
assert graphics['scene_sha256'] == sha(OUT / 'geo_01a-easy.rcplay')
android = read('android-difficulty/validation.json')
assert android['passed'] and len(android['levels']) == 3
for actual, expected in zip(android['levels'], rules['levels']):
    assert actual['level'] == expected['level'] and actual['defeat_tick'] == expected['defeat_tick']
    assert actual['scene_sha256'] == sha(OUT / f"geo_01a-{actual['level']}.rcplay")
    required(actual, ['original_headless_tick_exact', 'normal_picker', 'restart', 'no_crash'])
jump_android = read('android-jump/validation.json')
required(jump_android, ['passed', 'native_trace_matches_headless', 'return_completed',
                        'pause_exact', 'restart_resets', 'no_crash'])
assert jump_android['natural_phases'] == [0, 1, 2, 3, 4]
assert jump_android['scene_sha256'] == sha(OUT / 'geo_01a-easy.rcplay')
assert jump_android['performance']['discarded_seconds'] == 0
assert jump_android['performance']['jni_max_ms'] < 100
for name in ['abi-validation.json', 'abi-easy.json', 'abi-hard.json']:
    assert read(name)['short_buffers_and_canaries']

tests = sum(map(int, re.findall(r'test result: ok\. (\d+) passed; 0 failed',
                               (OUT / 'workspace-tests.log').read_text(encoding='utf-8-sig'))))
assert tests == 985
assert 'Finished' in (OUT / 'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log', 'diff-check.log']:
    assert not (OUT / name).read_text(encoding='utf-8-sig').strip()
assert 'Verifies' in (OUT / 'apksigner.log').read_text(encoding='utf-8-sig')
assert 'Verification succesful' in (OUT / 'zipalign.log').read_text(encoding='utf-8-sig')
metadata = (OUT / 'apk-metadata.log').read_text(encoding='utf-8-sig')
assert "versionCode='20'" in metadata and "versionName='0.18.0-defensive-jump-scene'" in metadata
apk = OUT / 'republic-commando-0.18.0.apk'
assert sha(apk) == sha(ROOT / 'android/app/build/outputs/apk/debug/app-debug.apk')
for name in ['android-gpu/validation.json', 'android-cpu/validation.json', 'graphics/validation.json',
             'android-difficulty/validation.json', 'android-jump/validation.json',
             'abi-validation.json', 'abi-easy.json', 'abi-hard.json',
             'apksigner.log', 'zipalign.log', 'apk-metadata.log']:
    assert (OUT / name).stat().st_mtime >= apk.stat().st_mtime, f'Stale binary receipt: {name}'
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

previous_counts = {}
originals = {}
for stage in ['milestone3i', 'milestone3j', 'milestone3k']:
    previous = json.loads((ROOT / 'analysis' / stage / 'validation-manifest.json').read_text())
    for name, row in previous['evidence'].items():
        path = ROOT / name
        assert path.stat().st_size == row['bytes'] and sha(path) == row['sha256'], name
    originals.update(previous['originals'])
    previous_counts[stage] = len(previous['evidence'])
for name, digest in originals.items():
    assert sha(Path(name)) == digest, name
game = Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData')
for name in ['Maps/geo_01a.ctm', 'Animations/geonosian.ukx', 'System/engine.dll', 'System/core.dll', 'System/ctgame.dll']:
    originals[str(game / name)] = sha(game / name)
assert sha(game / 'System/ctgame.dll') == landing['provenance']['binary_sha256']
assert sha(game / 'System/engine.dll') == landing['translation_provenance']['binary_sha256']
assert sha(ROOT / 'analysis/milestone3j/defensive.asm') == landing['provenance']['assembly_sha256']
assert sha(OUT / 'animation-translation.asm') == landing['translation_provenance']['assembly_sha256']
for name, digest in trajectory['provenance']['assembly_sha256'].items():
    assert sha(ROOT / name) == digest, name

with ZipFile(ROOT / 'analysis/milestone3k/source-snapshot.zip') as previous:
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
    for name, digest in sources.items():
        assert hashlib.sha256(archive.read(name)).hexdigest() == digest
evidence = {p.relative_to(ROOT).as_posix(): dict(bytes=p.stat().st_size, sha256=sha(p))
            for p in OUT.rglob('*') if p.is_file() and p.name != 'validation-manifest.json'}
manifest = dict(version='0.18.0-defensive-jump-scene', version_code=20,
    scope='Bounded original JumpDest planning, TakeOff/flight/Land execution and port-policy return arc; prepared native landing/translation branches. No general GoalQueue, Perch, native physFalling, global RNG or full-game parity. M3 remains open; physical ARM64 acceptance pending.',
    checks=dict(workspace_library_tests=tests, native_cases=10206, landing_cases=1504,
                translation_cases=2048, trajectory_cases=4240, defensive_cases=2414,
                legacy_replay_commands=1002, legacy_m3i=True, clippy=True, format=True,
                android_gpu_cpu_interactive_easy=True, android_all_difficulties_passive=True,
                android_natural_jump_trace=True, graphics_context_restore=True, abi=True,
                previous_artifacts_verified=previous_counts, source_snapshot_verified=True),
    performance_emulator=jump_android['performance'], originals=originals, sources=sources,
    evidence=evidence, elf=elf, difficulty_levels=3,
    next_step='Selected DC-17m aim-error/assistance and FX/audio; retain explicit Goal/Perch/physics gaps')
(OUT / 'validation-manifest.json').write_bytes((json.dumps(manifest, indent=2) + '\n').encode())
print(f'Verified {tests} tests, 10206 prepared native cases, Android delivery and {len(sources)} source files')
