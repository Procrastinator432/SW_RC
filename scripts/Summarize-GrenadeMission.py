"""Verify and bind the playable original objective 0/1/2 delivery to its evidence."""
import hashlib
import json
import re
import struct
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission3-playable'

def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def read(name):
    return json.loads((OUT / name).read_text(encoding='utf-8-sig'))

def require(row, fields):
    for field in fields:
        assert row[field], field

scene = OUT / 'geo_01a-grenades.rcplay'
apk = OUT / 'republic-commando-0.21.0.apk'
check = read('check/validation.json')
require(check, ['passed', 'input_driven_all_routes', 'objective2_completed',
                'connecting_barrier_completed', 'weapon_events_match_shots',
                'five_original_factory_instances', 'bacta_and_crash_continue_without_victory',
                'original_counter_completes', 'restart_state_and_frame_exact',
                'static_gpu_packet_immutable', 'transient_barrier_removed', 'unknown_actor_unchanged'])
assert check['kills'] == 5 and check['grenades'] == 5
assert set(check['thirteen_sound_roles']) == set(map(str, range(13)))
for backend in ['gpu', 'cpu']:
    row = read(f'android-{backend}/validation.json')
    assert row['backend'] == backend and row['scene_sha256'] == sha(scene)
    require(row, ['passed', 'normal_picker', 'physical_touch_walk', 'hold_and_release',
                  'three_original_objectives_complete', 'five_factory_actors_and_kills',
                  'all_thirteen_sound_roles', 'weapon_samples_match_shots', 'pause_exact', 'restart', 'no_crash'])
    assert row['grenades'] == 5 and len(row['grenade_visited']) == 20
    assert len(row['visited']) == 7 and row['crash_visited'] and row['weapon_fire_samples'] >= 15
abi = read('abi-validation.json')
assert abi['scene_sha256'] == sha(scene) and abi['grenades'] == 5
require(abi, ['passed', 'observer_short_buffers_and_canaries', 'invalid_handles_and_flags',
              'physical_walk_three_objectives', 'five_factory_actors_and_kills',
              'all_original_audio_roles_consumed_once', 'weapon_fire_events_match_shots', 'restart_exact'])
graphics = read('graphics/validation.json')
assert graphics['scene_sha256'] == sha(scene)
require(graphics, ['abi_short_buffers_and_canaries', 'query_cache_invalidates_on_advance_and_restart',
                   'released_handle_rejected', 'backend_switch_preserves_state', 'actual_context_recreated',
                   'context_rebuild_pixels_exact', 'context_rebuild_pose_exact', 'short_reload_memory_growth_bounded', 'no_crash'])
assert graphics['cpu_gpu_spawn_mean_channel_error'] < 2
assert graphics['cpu_gpu_spawn_channels_within_two'] > .99
assert graphics['repeated_scene_loads'] == 3
tests_log = (OUT / 'workspace-tests.log').read_text(encoding='utf-8-sig')
assert 'FAILED' not in tests_log
tests = sum(map(int, re.findall(r'test result: ok\. (\d+) passed; 0 failed', tests_log)))
assert tests == 999, tests
assert 'Finished' in (OUT / 'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log', 'diff-check.log']:
    assert not (OUT / name).read_text(encoding='utf-8-sig').strip(), name
replay = read('legacy-replay.json')
assert replay['entries'] == 1002 and len(replay['runs']) == 6
assert all(run['exact_replay'] for run in replay['runs'])
assert read('landing-regression/validation.json')['passed']
assert read('crash-regression/validation.json')['passed']
assert 'Verifies' in (OUT / 'apksigner.log').read_text(encoding='utf-8-sig')
assert 'Verification succesful' in (OUT / 'zipalign.log').read_text(encoding='utf-8-sig')
metadata = (OUT / 'apk-metadata.log').read_text(encoding='utf-8-sig')
assert "versionCode='24'" in metadata and "versionName='0.21.0-original-grenade-pickup'" in metadata
assert sha(apk) == sha(ROOT / 'android/app/build/outputs/apk/debug/app-debug.apk')
assert sha(scene) == sha(ROOT / 'analysis/mission3-rebuild/geo_01a-grenades.rcplay')
(OUT / 'rebuild-validation.json').write_bytes((json.dumps(dict(
    scene_sha256=sha(scene), byte_exact_rebuild=True,
    source='Original game packages, frozen 0.20.1 scene, pickup foundation and selected original-anchor route'), indent=2) + '\n').encode())
elf = []
with ZipFile(apk) as archive:
    assert archive.testzip() is None
    for architecture in ['arm64-v8a', 'x86_64']:
        for name in ['librc_native.so', 'librc_bridge.so']:
            path = ROOT / 'android/app/src/main/jniLibs' / architecture / name
            data = path.read_bytes()
            assert archive.read(f'lib/{architecture}/{name}') == data
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
            elf.append(dict(abi=architecture, name=name, sha256=sha(path), load_alignments=alignments))
originals = {}
previous_count = 0
for folder in ['mission2-av', 'mission3-pickup']:
    previous = json.loads((ROOT / f'analysis/{folder}/validation-manifest.json').read_text())
    for name, row in previous['evidence'].items():
        path = ROOT / name
        assert path.stat().st_size == row['bytes'] and sha(path) == row['sha256'], name
    previous_count += len(previous['evidence'])
    originals.update(previous['originals'])
for name, digest in originals.items():
    assert sha(Path(name)) == digest, name
raw = scene.read_bytes()
mission = json.loads(raw[12:12 + int.from_bytes(raw[8:12], 'little')])
assert len(mission['sounds']) == 13 and mission['weapon_fire_sound'] == 4
assert {o['original'] for o in mission['grenades']['objects']} == {909, 910, 912, 913, 964}
assert all(o.get('released') for o in mission['grenades']['objects'] if o['original'] != 964)
assert len(mission['grenades']['route']) == 20
assert mission['grenades']['flow']['initial_ammo'] == 0
names = set(previous['sources'])
for folder in ['crates', 'docs', 'scripts', 'android/app/src/main/java']:
    names.update(p.relative_to(ROOT).as_posix() for p in (ROOT / folder).rglob('*')
                 if p.is_file() and p.suffix in ['.rs', '.md', '.py', '.ps1', '.java', '.toml'])
sources = {name: sha(ROOT / name) for name in sorted(names)}
with ZipFile(OUT / 'source-snapshot.zip', 'w', ZIP_DEFLATED) as archive:
    for name in sources:
        archive.write(ROOT / name, name)
with ZipFile(OUT / 'source-snapshot.zip') as archive:
    assert archive.testzip() is None
    for name, digest in sources.items():
        assert hashlib.sha256(archive.read(name)).hexdigest() == digest
evidence = {p.relative_to(ROOT).as_posix(): dict(bytes=p.stat().st_size, sha256=sha(p))
            for p in OUT.rglob('*') if p.is_file() and p.name != 'validation-manifest.json'
            and 'route-rest' not in p.parts and p.name != 'route-rest.log'}
for name in ['analysis/mission2-av/validation-manifest.json', 'analysis/mission3-pickup/validation-manifest.json']:
    path = ROOT / name
    evidence[name] = dict(bytes=path.stat().st_size, sha256=sha(path))
manifest = dict(version='0.21.0-original-grenade-pickup', version_code=24,
    scope='Playable contiguous selected original geo_01a objectives 0/1/2: Bacta, five crash-site droids, connecting barricade, five grenades. Original textured pickup, 13 sound roles and selected script timing/contact data. Four walls use bounded tipped resting poses, not full Karma dynamics. Connecting trooper/droid staging and full effects omitted. Provisional combat/AI; cinematic, complete first mission, throwing/objective3 and physical ARM64 acceptance remain open.',
    checks=dict(workspace_library_tests=tests, clippy=True, format=True, abi=True,
                original_objectives=[0, 1, 2], kills=5, grenades=5, audio_roles=13,
                grenade_route_points=20, android_gpu_cpu_touch=True, graphics_context_restore=True,
                previous_evidence_verified=previous_count, source_snapshot_verified=True),
    originals=originals, sources=sources, evidence=evidence, elf=elf,
    next_step='Original objective3: grenade throwing, detonation, damage and gun emplacement destruction; physical ARM64 visual/audio acceptance.')
(OUT / 'validation-manifest.json').write_bytes((json.dumps(manifest, indent=2) + '\n').encode())
print(f'Verified {tests} tests, Android GPU/CPU, {len(evidence)} evidence files and {len(sources)} sources; objectives 0/1/2 complete')
