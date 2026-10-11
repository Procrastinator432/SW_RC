"""Bind the contiguous original Bacta/crash-site section to tested binaries, evidence and sources."""
import hashlib
import json
import re
import struct
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission2-av'

def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def read(name):
    return json.loads((OUT / name).read_text(encoding='utf-8-sig'))

def require(row, fields):
    for field in fields:
        assert row[field], field

scene = OUT / 'geo_01a-crash.rcplay'
apk = OUT / 'republic-commando-0.20.1.apk'
check = read('check/validation.json')
require(check, ['passed', 'bacta_continues_without_victory', 'five_original_factory_instances',
                'input_driven_both_routes', 'original_counter_completes', 'restart_state_and_frame_exact',
                'static_gpu_packet_immutable', 'transient_barrier_removed', 'unknown_actor_unchanged'])
assert check['kills'] == 5
for backend in ['gpu', 'cpu']:
    row = read(f'android-{backend}/validation.json')
    assert row['backend'] == backend and row['scene_sha256'] == sha(scene)
    require(row, ['passed', 'normal_picker', 'physical_touch_walk', 'hold_and_release',
                  'both_original_objectives_complete', 'five_factory_actors_and_kills',
                  'four_advisor_samples', 'weapon_samples_match_shots', 'pause_exact', 'restart', 'no_crash'])
    assert len(row['visited']) == 7 and row['crash_visited']
    assert row['weapon_fire_samples'] >= 15
abi = read('abi-validation.json')
assert abi['scene_sha256'] == sha(scene)
require(abi, ['passed', 'observer_short_buffers_and_canaries', 'invalid_handles_and_flags',
              'physical_walk_both_objectives', 'five_factory_actors_and_kills',
              'four_audio_events_consumed_once', 'weapon_fire_events_match_shots', 'restart_exact'])

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
assert tests == 990
assert 'Finished' in (OUT / 'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log', 'diff-check.log']:
    assert not (OUT / name).read_text(encoding='utf-8-sig').strip()
assert read('legacy-replay.json')['exact_replay']
assert read('landing-regression/validation.json')['passed']
assert 'Verifies' in (OUT / 'apksigner.log').read_text(encoding='utf-8-sig')
assert 'Verification succesful' in (OUT / 'zipalign.log').read_text(encoding='utf-8-sig')
metadata = (OUT / 'apk-metadata.log').read_text(encoding='utf-8-sig')
assert "versionCode='23'" in metadata and "versionName='0.20.1-textures-and-audio'" in metadata
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
previous = json.loads((ROOT / 'analysis/mission2-playable/validation-manifest.json').read_text())
for name, row in previous['evidence'].items():
    path = ROOT / name
    assert path.stat().st_size == row['bytes'] and sha(path) == row['sha256'], name
landing = json.loads((ROOT / 'analysis/mission1/validation-manifest.json').read_text())
for name, row in landing['evidence'].items():
    path = ROOT / name
    assert path.stat().st_size == row['bytes'] and sha(path) == row['sha256'], name
originals = previous['originals'].copy()
for name, digest in originals.items():
    assert sha(Path(name)) == digest, name
game = Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData')
for name in ['Maps/geo_01a.ctm', 'Animations/hudarms.ukx', 'Animations/bactadispensers.ukx',
             'Sounds/GEO_Voice.uax', 'Properties/MarkerProperties.u', 'System/gameplay.u',
             'System/ctgame.u', 'System/engine.u', 'StaticMeshes/Arena_HR.usx', 'Textures/Arena_Textures.utx', 'Sounds/GEN_Sound.uax', 'Properties/Properties.u']:
    originals[str(game / name)] = sha(game / name)
repair = read('repair-receipt.json')
assert repair['bsp_triangles_textured'] == 557 and repair['mesh_triangles_repaired'] == 8176
assert repair['untextured_surface_triangles'] == 0 and not repair['remaining_errors']
assert repair['volumetric_effect_triangles_omitted'] == 1264
assert repair['fire_sound'] == 'GEN_Sound.Weapons.wep_GEN_pistolFire_01'
assert repair['fire_sound_default']['declaring_class'] == 'properties.DC15sDefaults'
assert len([m for m in repair['materials'] if m['source_channel'] == 1]) == 3
scene_report=json.loads((ROOT/'analysis/scenes/geo_01a-textured/scene.json').read_text())
for mesh in {i['mesh'].split('.')[0] for i in scene_report['instances']}:
    path=next(p for p in (game/'StaticMeshes').iterdir() if p.stem.lower()==mesh.lower() and p.suffix.lower()=='.usx')
    originals[str(path)]=sha(path)
with ZipFile(ROOT / 'analysis/mission2-playable/source-snapshot.zip') as archive:
    names = set(archive.namelist())
names.update(['Cargo.toml','Cargo.lock','README.md','android/app/build.gradle','android/native/bridge.c'])
for folder in ['crates', 'docs', 'scripts','android/app/src/main/java']:
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
for name in ['analysis/scenes/geo_01a-textured/scene.json','analysis/mission2/original-crosscheck.json','analysis/mission2/check/validation.json','analysis/mission1/validation-manifest.json','analysis/mission2/validation-manifest.json','analysis/mission2-playable/validation-manifest.json']:
    path=ROOT/name
    evidence[name]=dict(bytes=path.stat().st_size,sha256=sha(path))

manifest = dict(version='0.20.1-textures-and-audio', version_code=23,
    scope='Surface repair: all retained base scene triangles textured, original UV1 materials; unsupported volumetric falloff effects omitted. DC15s original FireSound per accepted shot. Contiguous selected original geo_01a objectives 0 and 1. Prepared entry after cinematic, held Bacta use, timed barrier release, original five factories and invulnerability contacts, original death counter and exit barrier. Original BattleDroid/DC15s visuals; combat uses provisional hitscan and enemy approach/fire, not native AI/weapon parity. Full first mission, decorative scripts, visual parity and physical ARM64 acceptance remain open.',
    checks=dict(workspace_library_tests=tests, clippy=True, format=True, abi=True,
                original_objectives=[0, 1], original_factory_ids=[133,134,277,441,836],
                physical_guide_points=13, android_gpu_cpu_touch=True, graphics_context_restore=True,
                surface_texture_gaps=0, dc15s_fire_audio=True, previous_evidence_verified=len(previous['evidence'])+len(landing['evidence']), source_snapshot_verified=True),
    originals=originals, sources=sources, evidence=evidence, elf=elf,
    next_step='Physical device visual/audio acceptance before further mission expansion; remaining lighting/falloff effects and native combat parity remain open.')
(OUT / 'validation-manifest.json').write_bytes((json.dumps(manifest, indent=2) + '\n').encode())
print(f'Verified {tests} tests, Android GPU/CPU, {len(evidence)} evidence files and {len(sources)} sources')
