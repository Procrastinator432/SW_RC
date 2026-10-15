"""Bind the playable mission Warrior AI delivery and its explicit parity limits."""
import hashlib
import json
import re
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/mission9-ai'
GAME=Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData')
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def read(name): return json.loads((OUT/name).read_text(encoding='utf-8-sig'))

previous=[json.loads((ROOT/f'analysis/{name}/validation-manifest.json').read_text())
          for name in ['mission8-artillery']]
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

scene=OUT/'geo_01a-warrior-ai.rcplay'
apk=OUT/'republic-commando-0.28.0.apk'
assert scene.read_bytes()[:8]==b'RCPLAY11' and scene.stat().st_size==94561144
profile=read('ai-assets.json')
assert profile['burst_count']==1
assert len(profile['navigation']['nodes'])==78 and len(profile['navigation']['edges'])==208
assert profile['melee']['melee_range']==350 and profile['melee']['melee_damage']==125
accepted=json.loads((ROOT/'analysis/milestone3l/validation-manifest.json').read_text())
melee_profile_path='analysis/milestone3l/geo_01a-normal.rcplay'
assert sha(ROOT/melee_profile_path)==accepted['evidence'][melee_profile_path]['sha256']
raw=(ROOT/melee_profile_path).read_bytes();n=int.from_bytes(raw[8:12],'little')
assert profile['melee']==json.loads(raw[12:12+n])['melee']
behavior=read('behavior/validation.json')
assert behavior['passed'] and behavior['restart_exact'] and behavior['attack_damage_delayed_to_original_notify']
assert behavior['defeat']['normal_inputs_only']
assert behavior['defeat']['attack_tick']==14728 and behavior['defeat']['tick']==14758
assert behavior['defeat']['state']['phase']=='Lost' and behavior['dodge_state']['health']==20
assert sum(w['ai']['hits'] for w in behavior['defeat']['state']['artillery']['warriors'])==1
assert behavior['detour']['isolated_original_geometry_probe'] and len(behavior['detour']['route'])>1
v=read('playthrough/validation.json');s=v['complete_state'];a=s['artillery']
assert v['passed'] and v['input_driven_objectives']==list(range(7))
for flag in ['restart_exact','restart_frame_and_closed_collision','static_gpu_packet_unchanged',
             'closed_barricade_blocks','released_barricade_clear_sweep','closed_door_blocks',
             'released_door_clear_sweep','original_exit_trigger_entered','bombardment_defeat_normal_inputs']:
    assert v[flag],flag
assert s['phase']=='Won' and s['health']==20 and s['statistics']['kills']==8
assert s['statistics']['ticks']==16640 and s['spire']['flow']['completed']
assert a['warrior_factory']['spawned']==3 and a['projectile_factory']['spawned']==4
assert len(a['warriors'])==3 and all(w['health']==0 for w in a['warriors'])
assert len(a['impacts'])==4 and not a['missiles']
for effect,count in [('SpiderDroidDestroyed',1),('AttackWarriorDead',3),('CTEffects.SpiderDroid_BeamAutoDestroy',2)]:
    assert sum(e==effect for _,e in a['effects'])==count,effect
assert a['destroyed_tick']-a['attack_tick']==948
assert not any(d['victim']==-1 for d in a['damage'])
danger=read('playthrough/danger-validation.json')
assert danger['passed'] and danger['normal_inputs_only'] and danger['phase']=='Lost'
assert danger['health']==0 and danger['ticks']==13930
assert sum(d['amount'] for d in danger['damage'] if d['victim']==-1)>0
legacy=read('legacy-10-regression/validation.json')
assert legacy['passed'] and legacy['restart_exact'] and legacy['complete_state']['statistics']['ticks']==16648
native=read('native/validation.json');android=read('android/native-replay.json')
for row in [native,android]:
    assert row['passed'] and row['inputs']==16640 and row['bombardment_defeat_replay']
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
assert android['ai_behavior_replays']
for file,phase in [('ai-defeat-input.bin',2),('ai-dodge-input.bin',0),('ai-interrupt-input.bin',0)]:
    row=native['ai_behavior'][file]
    assert row['passed'] and row['status'][4]==phase and row['status'][0]==(0 if phase==2 else 20)
    digest=sha(OUT/'behavior'/file)
    assert row['sha256']==digest and android[file+'_sha256']==digest

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
assert tests==1067,tests
assert 'Finished' in (OUT/'clippy.log').read_text(encoding='utf-8-sig')
assert 'error:' not in (OUT/'clippy.log').read_text(encoding='utf-8-sig')
assert 'BUILD SUCCESSFUL' in (OUT/'android-build.log').read_text(encoding='utf-8-sig')
for name in ['format.log','source-diff-check.log']:
    assert not (OUT/name).read_text(encoding='utf-8-sig').strip(),name
assert 'versionCode 31' in (ROOT/'android/app/build.gradle').read_text()
assert "versionName '0.28.0-warrior-ai'" in (ROOT/'android/app/build.gradle').read_text()
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
result=dict(passed=True,playable_milestone_complete=True,version='0.28.0',version_code=31,library_tests=tests,
    completed_selected_original_objectives=list(range(7)),entire_first_mission_complete=False,
    next_map_request='GEO_01B.ctm#playerstart',next_map_loaded=False,
    scene_sha256=sha(scene),apk_sha256=sha(apk),evidence=evidence,sources=sources,originals=originals,
    original_melee_profile_scene=dict(path=melee_profile_path,sha256=sha(ROOT/melee_profile_path)),
    prior_evidence_unchanged=sum(len(p['evidence']) for p in previous),
    normal_inputs=16640,bombardment_defeat_inputs=13930,normal_health=20,normal_kills=8,
    navigation_nodes=78,navigation_edges=208,ai_defeat_inputs=14758,ai_attack_started_tick=14728,
    ai_normal_input_pursuit_dodge_interruption=True,
    android_build_abis=['arm64-v8a','x86_64'],android_native_contiguous_input_replay=True,
    android_apk_gpu_touch_use_fire_crouch_audio_smoke=True,android_complete_touch_playthrough=False,
    physical_arm64_acceptance=False,
    limitations=['Mission Warrior walking/melee controller; no general CTBot or complete game AI parity',
        'Lunge, offensive/defensive jumps and grenade availability remain disabled in this mission controller',
        'LOS2600, eye offsets80/60, memory3s, reaction0.25s, alternating attacks and Dijkstra are explicit host rules',
        'Mission health remains without shields; original ForcePike125 is lethal after previous20HP encounter',
        'General squad combat/follow/revive and original perception/GoalQueue remain open',
        'Previous bounded projectile/effect/door adapters, full introduction and loading GEO_01B remain open',
        'Complete manual touch playthrough and physical ARM64 acceptance remain open'])
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
print(f'Frozen Warrior AI milestone: {len(evidence)} evidence files, {len(sources)} sources, {len(originals)} originals; {tests} tests; mission and AI behavior accepted')
