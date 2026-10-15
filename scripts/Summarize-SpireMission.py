"""Bind the playable selected objective6 delivery and its explicit parity limits."""
import hashlib
import json
import re
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/mission7-spire'
GAME=Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData')
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def read(name): return json.loads((OUT/name).read_text(encoding='utf-8-sig'))

previous=[json.loads((ROOT/f'analysis/{name}/validation-manifest.json').read_text())
          for name in ['mission6-playable']]
for prior in previous:
    for name,row in prior['evidence'].items():
        path=ROOT/name
        assert path.stat().st_size==row['bytes'] and sha(path)==row['sha256'],name
originals=previous[-1]['originals'].copy()
for name in ['Animations/HudArms.ukx','StaticMeshes/MarkerIcons.usx','Textures/MarkerIconTextures.utx',
             'Sounds/GEN_Sound.uax','Sounds/GEO_Voice.uax']:
    path=GAME/name;originals.setdefault(str(path),sha(path))
for name,digest in originals.items(): assert sha(Path(name))==digest,name

scene=OUT/'geo_01a-spire.rcplay'
apk=OUT/'republic-commando-0.26.0.apk'
assert scene.read_bytes()[:8]==b'RCPLAY09' and scene.stat().st_size==80217777
build=read('build-validation.json')
assert build['passed'] and build['sound_count']==57
assert {row['original'] for row in build['objects']}=={34,35,860,861,862}
assert all(row['faces']>0 and row['hulls']>0 for row in build['objects'])
assert all(row['diffuse'] and row['opaque'] for row in build['materials'])
assert read('route-validation.json')['passed']
assets=read('spire-assets.json');assert len(assets['route'])==14
assert assets['flow']['next_url']=='GEO_01B.ctm#playerstart'
assert assets['provenance']['door_timing']['key_position_index']==1
v=read('playthrough/validation.json');s=v['complete_state'];d=s['spire']
assert v['passed'] and v['input_driven_objectives']==list(range(7))
for flag in ['restart_exact','restart_frame_and_closed_collision','static_gpu_packet_unchanged',
             'closed_barricade_blocks','released_barricade_clear_sweep','closed_door_blocks',
             'released_door_clear_sweep','original_exit_trigger_entered']:
    assert v[flag],flag
assert s['phase']=='Won' and s['health']==20 and s['statistics']['kills']==5
assert s['statistics']['ticks']==14746 and d['completed_tick']==14746
assert all(d['flow'][f] for f in ['enabled','entered','barrier_open','door_started','door_open','completed'])
assert d['flow']['conversation']['next']==4 and d['flow']['conversation']['remaining']==0
for actor in [34,35,860,861,862]:
    assert sum(effect=={'Release':actor} for _,effect in d['effects'])==1
assert sum(effect=={'Exit':'GEO_01B.ctm#playerstart'} for _,effect in d['effects'])==1
legacy=read('legacy-08-regression/validation.json')
assert legacy['passed'] and legacy['restart_exact'] and legacy['complete_state']['statistics']['ticks']==13397
native=read('native/validation.json');android=read('android/native-replay.json')
for row in [native,android]:
    assert row['passed'] and row['inputs']==14746
    sounds=row['sound_counts'];assert len(sounds)==57
    assert sounds[20:33]==[1]*13
    assert all(sounds[i]==1 for i in range(33,57) if i!=38)
    assert sounds[38]==20
assert native['scene_sha256']==sha(scene)
assert native['input_sha256']==sha(OUT/'playthrough/input-replay.bin')
assert android['scene.rcplay_sha256']==sha(scene)
assert android['input-replay.bin_sha256']==sha(OUT/'playthrough/input-replay.bin')
assert android['librc_native.so_sha256']==sha(ROOT/'android/app/src/main/jniLibs/x86_64/librc_native.so')
assert android['health']==20 and android['phase']==1 and android['kills']==5
ui=read('android/validation.json')
for flag in ['passed','apk_scene_picker','gpu_render','touch_use','touch_fire','touch_crouch_height_change','pause']:
    assert ui[flag],flag
for mode in ['known','unknown']:
    row=read(f'scene-reader-{mode}.json')
    assert row['passed'] and row['sha256']==sha(scene) and row['bytes']==scene.stat().st_size
    assert row['heap_limit']<2*scene.stat().st_size
    assert row['malformed_inputs_rejected'] and row['temporary_files_removed']
log=(OUT/'workspace-tests.log').read_text(encoding='utf-8-sig')
assert 'FAILED' not in log
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed',log)))
assert tests==1060,tests
assert 'Finished' in (OUT/'clippy.log').read_text(encoding='utf-8-sig')
assert 'error:' not in (OUT/'clippy.log').read_text(encoding='utf-8-sig')
assert 'BUILD SUCCESSFUL' in (OUT/'android-build.log').read_text(encoding='utf-8-sig')
for name in ['format.log','source-diff-check.log']:
    assert not (OUT/name).read_text(encoding='utf-8-sig').strip(),name
assert "versionCode 29" in (ROOT/'android/app/build.gradle').read_text()
assert "versionName '0.26.0-spire-exit'" in (ROOT/'android/app/build.gradle').read_text()
with ZipFile(apk) as archive:
    assert archive.testzip() is None
    for arch in ['arm64-v8a','x86_64']:
        for lib in ['librc_native.so','librc_bridge.so']:
            assert archive.read(f'lib/{arch}/{lib}')==(ROOT/f'android/app/src/main/jniLibs/{arch}/{lib}').read_bytes()
names=set(previous[-1]['sources'])
for folder in ['crates','docs','scripts','android/app/src/main/java']:
    names.update(p.relative_to(ROOT).as_posix() for p in (ROOT/folder).rglob('*')
                 if p.is_file() and p.suffix in ['.rs','.md','.py','.ps1','.java','.toml','.c'])
names.update(['README.md','Cargo.toml','Cargo.lock','android/app/build.gradle','android/app/src/main/AndroidManifest.xml'])
sources={name:sha(ROOT/name) for name in sorted(names)}
with ZipFile(OUT/'source-snapshot.zip','w',ZIP_DEFLATED) as archive:
    for name in sources: archive.write(ROOT/name,name)
with ZipFile(OUT/'source-snapshot.zip') as archive:
    assert archive.testzip() is None
    for name,digest in sources.items(): assert hashlib.sha256(archive.read(name)).hexdigest()==digest,name
evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p))
          for p in sorted(OUT.rglob('*')) if p.is_file() and p.name!='validation-manifest.json'}
result=dict(passed=True,playable_milestone_complete=True,version='0.26.0',version_code=29,library_tests=tests,
    completed_selected_original_objectives=list(range(7)),entire_first_mission_complete=False,
    next_map_request='GEO_01B.ctm#playerstart',next_map_loaded=False,
    scene_sha256=sha(scene),apk_sha256=sha(apk),evidence=evidence,sources=sources,originals=originals,
    prior_evidence_unchanged=sum(len(p['evidence']) for p in previous),
    android_build_abis=['arm64-v8a','x86_64'],android_native_contiguous_input_replay=True,
    android_apk_gpu_touch_use_fire_crouch_audio_smoke=True,android_complete_touch_playthrough=False,
    physical_arm64_acceptance=False,
    limitations=['Selected objective6 host; full first mission parity and loading GEO_01B remain open',
        'GunshipAttack/SpiderDroidAttack/ArialBombardmentEndLevel events logged; full cinematic, factory combat and projectile damage remain open',
        'Door uses original DelayTime/MoveTimes and KeyPos[1], final pose switch without continuous Mover interpolation',
        'Port route guide replaces original animated exit arrows',
        'General squad combat/follow/revive, interrupted NPC setup and full native GoalQueue remain open',
        'Tutorial3552–3555 and SpawnWarriorAttack staging remain outside live factory simulation',
        'SpawnDemoDroids and SpawnExitArea have no receiver in the inspected map inventory',
        'Preplacement hologram HardwareShader and full large-bunker particle system remain open',
        'SoundMultiple selects first original stored child; bounded combat/projectile/Karma/explosion adapters remain'])
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
print(f'Frozen spire milestone: {len(evidence)} evidence files, {len(sources)} sources, {len(originals)} originals; {tests} tests; selected objectives0–6 accepted')
