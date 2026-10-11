"""Bind the first original Bacta objective to tested binaries, evidence and sources."""
import hashlib
import json
import re
import struct
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission1'

def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def read(name):
    return json.loads((OUT / name).read_text(encoding='utf-8-sig'))

def require(row, fields):
    for field in fields:
        assert row[field], field

scene = OUT / 'geo_01a-landing.rcplay'
apk = OUT / 'republic-commando-0.19.0.apk'
check = read('check/validation.json')
require(check, ['passed', 'distant_use_rejected', 'full_physical_walk', 'hold_heals',
                'release_stops_healing', 'objective_completed', 'restart_exact', 'movement_120hz'])
assert (check['route_points'], check['walked_ticks'], check['original_objective']) == (7, 640, 0)
for backend in ['gpu', 'cpu']:
    row = read(f'android-{backend}/validation.json')
    assert row['backend'] == backend and row['scene_sha256'] == sha(scene)
    require(row, ['passed', 'normal_picker', 'physical_touch_walk', 'hold_and_release',
                  'original_objective_complete', 'three_advisor_samples', 'pause_exact', 'restart', 'no_crash'])
    assert len(row['visited']) == 7
abi = read('abi-validation.json')
assert abi['scene_sha256'] == sha(scene)
require(abi, ['passed', 'short_buffers_and_canaries', 'invalid_flags_and_handles',
              'physical_walk_and_completion', 'three_audio_events_consumed_once', 'restart_exact'])
graphics = read('graphics/validation.json')
assert graphics['scene_sha256'] == sha(scene)
require(graphics, ['abi_short_buffers_and_canaries', 'query_cache_invalidates_on_advance_and_restart',
                   'released_handle_rejected', 'backend_switch_preserves_state', 'actual_context_recreated',
                   'context_rebuild_pixels_exact', 'context_rebuild_pose_exact', 'short_reload_memory_growth_bounded', 'no_crash'])
assert graphics['cpu_gpu_spawn_mean_channel_error'] < 2
assert graphics['cpu_gpu_spawn_channels_within_two'] > .99
assert graphics['repeated_scene_loads'] == 3
tests = sum(map(int, re.findall(r'test result: ok\. (\d+) passed; 0 failed',
                               (OUT / 'workspace-tests.log').read_text(encoding='utf-8-sig'))))
assert tests == 986
assert 'Finished' in (OUT / 'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log', 'diff-check.log']:
    assert not (OUT / name).read_text(encoding='utf-8-sig').strip()
assert read('legacy-replay.json')['exact_replay']
assert read('legacy-difficulty/game-rules-validation.json')['passed']
assert read('legacy-jump/jump-validation.json')['passed']
assert 'Verifies' in (OUT / 'apksigner.log').read_text(encoding='utf-8-sig')
assert 'Verification succesful' in (OUT / 'zipalign.log').read_text(encoding='utf-8-sig')
metadata = (OUT / 'apk-metadata.log').read_text(encoding='utf-8-sig')
assert "versionCode='21'" in metadata and "versionName='0.19.0-original-landing'" in metadata
assert sha(apk) == sha(ROOT / 'android/app/build/outputs/apk/debug/app-debug.apk')
for name in ['android-gpu/validation.json', 'android-cpu/validation.json', 'graphics/validation.json',
             'abi-validation.json', 'apksigner.log', 'zipalign.log', 'apk-metadata.log']:
    assert (OUT / name).stat().st_mtime >= apk.stat().st_mtime, f'Stale binary receipt: {name}'
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
previous = json.loads((ROOT / 'analysis/milestone3l/validation-manifest.json').read_text())
for name, row in previous['evidence'].items():
    path = ROOT / name
    assert path.stat().st_size == row['bytes'] and sha(path) == row['sha256'], name
originals = previous['originals'].copy()
for name, digest in originals.items():
    assert sha(Path(name)) == digest, name
game = Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData')
for name in ['Maps/geo_01a.ctm', 'Animations/hudarms.ukx', 'Animations/bactadispensers.ukx',
             'Sounds/GEO_Voice.uax', 'Properties/MarkerProperties.u', 'System/gameplay.u',
             'System/ctgame.u', 'System/engine.u']:
    originals[str(game / name)] = sha(game / name)
objects = read('geo_01a-original.json')['objects']
lists = [o for o in objects if o['class'].split('.')[-1] in ['ScriptedTrigger', 'ScriptedSequence']]
assert len(lists) == 92
assert all('Actions' in o['arrays'] for o in lists)
objective = next(o for o in objects if o['index'] == 16)
assert len(objective['arrays']['MissionObj']) == 7
assert 'Use bacta to heal' in json.dumps(objective)
assert 'BactaDispenserLZ' in json.dumps(next(o for o in objects if o['index'] == 736))
assert '175' in json.dumps(next(o for o in objects if o['index'] == 2397))
with ZipFile(ROOT / 'analysis/milestone3l/source-snapshot.zip') as archive:
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
for name in ['System__gameplay.u/ScriptedTrigger.uc', 'System__gameplay.u/ACTION_ObjectiveUpdate.uc',
             'System__gameplay.u/ACTION_HurtActor.uc', 'System__ctgame.u/BactaDispenser.uc',
             'System__engine.u/MissionObjectives.uc']:
    path = ROOT / 'analysis/reports/sources' / name
    evidence[path.relative_to(ROOT).as_posix()] = dict(bytes=path.stat().st_size, sha256=sha(path))
manifest = dict(version='0.19.0-original-landing', version_code=21,
    scope='First original geo_01a objective only: prepared post-cinematic entry, physical walk, held Bacta use, objective0 completion. Not the full first mission. Physical ARM64 acceptance pending. ramp-refusal.json preserves a diagnostic before the slope fix.',
    checks=dict(workspace_library_tests=tests, clippy=True, format=True, abi=True,
                original_script_lists=92, original_objectives=7, physical_route_points=7,
                android_gpu_cpu_touch=True, graphics_context_restore=True,
                previous_evidence_verified=len(previous['evidence']), source_snapshot_verified=True),
    originals=originals, sources=sources, evidence=evidence, elf=elf,
    next_step='Original objective1: Secure area around crash; barriers, tracker targets and original enemy factories.')
(OUT / 'validation-manifest.json').write_bytes((json.dumps(manifest, indent=2) + '\n').encode())
print(f'Verified {tests} tests, Android GPU/CPU, {len(evidence)} evidence files and {len(sources)} sources')
