"""Bind the playable selected artillery encounter delivery and its explicit parity limits."""
import hashlib
import json
import re
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/mission8-artillery'
GAME=Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData')
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def read(name): return json.loads((OUT/name).read_text(encoding='utf-8-sig'))

previous=[json.loads((ROOT/f'analysis/{name}/validation-manifest.json').read_text())
          for name in ['mission7-spire']]
for prior in previous:
    for name,row in prior['evidence'].items():
        path=ROOT/name
        assert path.stat().st_size==row['bytes'] and sha(path)==row['sha256'],name
originals=previous[-1]['originals'].copy()
for name in ['Animations/HudArms.ukx','StaticMeshes/MarkerIcons.usx','Textures/MarkerIconTextures.utx',
             'Sounds/GEN_Sound.uax','Sounds/GEO_Voice.uax', 'Animations/vehicles.ukx',
             'Animations/Geonosian.ukx','StaticMeshes/Weapons.usx','Textures/EffectTex.utx',
             'Textures/VehicleTextures.utx','Textures/WeaponTextures.utx']:
    path=GAME/name;originals.setdefault(str(path),sha(path))
for name,digest in originals.items(): assert sha(Path(name))==digest,name

scene=OUT/'geo_01a-artillery.rcplay'
apk=OUT/'republic-commando-0.27.0.apk'
assert scene.read_bytes()[:8]==b'RCPLAY10' and scene.stat().st_size==92212438
build=read('build-validation.json')
assert build['passed'] and build['sounds']==64 and len(build['spawns'])==3
v=read('playthrough/validation.json');s=v['complete_state'];a=s['artillery']
assert v['passed'] and v['input_driven_objectives']==list(range(7))
for flag in ['restart_exact','restart_frame_and_closed_collision','static_gpu_packet_unchanged',
             'closed_barricade_blocks','released_barricade_clear_sweep','closed_door_blocks',
             'released_door_clear_sweep','original_exit_trigger_entered','bombardment_defeat_normal_inputs']:
    assert v[flag],flag
assert s['phase']=='Won' and s['health']==20 and s['statistics']['kills']==8
assert s['statistics']['ticks']==16648 and s['spire']['flow']['completed']
assert a['warrior_factory']['spawned']==3 and a['projectile_factory']['spawned']==4
assert len(a['warriors'])==3 and all(w['health']==0 for w in a['warriors'])
assert len(a['impacts'])==4 and not a['missiles']
for effect,count in [('SpiderDroidDestroyed',1),('AttackWarriorDead',3),('CTEffects.SpiderDroid_BeamAutoDestroy',2)]:
    assert sum(e==effect for _,e in a['effects'])==count,effect
assert a['destroyed_tick']-a['attack_tick']==948
assert not any(d['victim']==-1 for d in a['damage'])
danger=read('playthrough/danger-validation.json')
assert danger['passed'] and danger['normal_inputs_only'] and danger['phase']=='Lost'
assert danger['health']==0 and danger['ticks']==13899
assert sum(d['amount'] for d in danger['damage'] if d['victim']==-1)==139
legacy=read('legacy-09-regression/validation.json')
assert legacy['passed'] and legacy['restart_exact'] and legacy['complete_state']['statistics']['ticks']==14746
native=read('native/validation.json');android=read('android/native-replay.json')
for row in [native,android]:
    assert row['passed'] and row['inputs']==16648 and row['bombardment_defeat_replay']
    sounds=row['sound_counts'];assert len(sounds)==64
    assert sounds[20:33]==[1]*13
    assert all(sounds[i]==1 for i in range(33,62) if i!=38)
    assert sounds[38]==20 and sounds[62:64]==[4,4]
assert native['scene_sha256']==sha(scene)
assert native['input_sha256']==sha(OUT/'playthrough/input-replay.bin')
for name,path in [('scene.rcplay',scene),('input-replay.bin',OUT/'playthrough/input-replay.bin'),
                  ('danger-input-replay.bin',OUT/'playthrough/danger-input-replay.bin'),
                  ('librc_native.so',ROOT/'android/app/src/main/jniLibs/x86_64/librc_native.so')]:
    assert android[name+'_sha256']==sha(path),name
assert android['health']==20 and android['phase']==1 and android['kills']==8
ui=read('android/validation.json')
for flag in ['passed','apk_scene_picker','gpu_render','touch_use','touch_fire','touch_crouch_height_change','pause']:
    assert ui[flag],flag
for mode in ['known','unknown']:
    row=read(f'scene-reader-{mode}.json')
    assert row['passed'] and row['sha256']==sha(scene) and row['bytes']==scene.stat().st_size
    assert row['heap_limit']==112*1024*1024
    assert row['malformed_inputs_rejected'] and row['temporary_files_removed']
log=(OUT/'workspace-tests.log').read_text(encoding='utf-8-sig')
assert 'FAILED' not in log
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed',log)))
assert tests==1064,tests
assert 'Finished' in (OUT/'clippy.log').read_text(encoding='utf-8-sig')
assert 'error:' not in (OUT/'clippy.log').read_text(encoding='utf-8-sig')
assert 'BUILD SUCCESSFUL' in (OUT/'android-build.log').read_text(encoding='utf-8-sig')
for name in ['format.log','source-diff-check.log']:
    assert not (OUT/name).read_text(encoding='utf-8-sig').strip(),name
assert 'versionCode 30' in (ROOT/'android/app/build.gradle').read_text()
assert "versionName '0.27.0-artillery'" in (ROOT/'android/app/build.gradle').read_text()
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
result=dict(passed=True,playable_milestone_complete=True,version='0.27.0',version_code=30,library_tests=tests,
    completed_selected_original_objectives=list(range(7)),entire_first_mission_complete=False,
    next_map_request='GEO_01B.ctm#playerstart',next_map_loaded=False,
    scene_sha256=sha(scene),apk_sha256=sha(apk),evidence=evidence,sources=sources,originals=originals,
    prior_evidence_unchanged=sum(len(p['evidence']) for p in previous),
    normal_inputs=16648,bombardment_defeat_inputs=13899,normal_health=20,normal_kills=8,
    android_build_abis=['arm64-v8a','x86_64'],android_native_contiguous_input_replay=True,
    android_apk_gpu_touch_use_fire_crouch_audio_smoke=True,android_complete_touch_playthrough=False,
    physical_arm64_acceptance=False,
    limitations=['Selected artillery encounter; entire first mission and loading GEO_01B remain open',
        'Warrior direct ground chase, melee5/1.4s and selected damage adapters are port policies, not full CTBot parity',
        'Factory delays use random midpoints and spawn round robin; general original factory VM remains open',
        'Projectile uses straight swept world contacts; inherited bounce, fuse and actor-contact parity remain open',
        'Original beam/ribbon textures and attachment notifies use bounded geometry; other effects reuse sprite adapter',
        'Full Gunship introduction, particle, lighting and shader parity remain open',
        'GroundDust, SpawnDemoDroids and SpawnExitArea have no found original map receiver',
        'Door uses original timing and final key pose without continuous Mover interpolation',
        'General squad combat/follow/revive, complete touch playthrough and physical ARM64 acceptance remain open'])
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
print(f'Frozen artillery milestone: {len(evidence)} evidence files, {len(sources)} sources, {len(originals)} originals; {tests} tests; normal win and bombardment loss accepted')
