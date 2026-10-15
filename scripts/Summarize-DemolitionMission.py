"""Bind the playable selected objective5 delivery and its explicit parity limits."""
import hashlib
import json
import re
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/mission6-playable'
GAME=Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData')
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def read(name): return json.loads((OUT/name).read_text(encoding='utf-8-sig'))

previous=[json.loads((ROOT/f'analysis/{name}/validation-manifest.json').read_text())
          for name in ['mission5-playable','mission6-demolition']]
for prior in previous:
    for name,row in prior['evidence'].items():
        path=ROOT/name
        assert path.stat().st_size==row['bytes'] and sha(path)==row['sha256'],name
originals=previous[-1]['originals'].copy()
for name in ['Animations/HudArms.ukx','StaticMeshes/MarkerIcons.usx','Textures/MarkerIconTextures.utx',
             'Sounds/GEN_Sound.uax','Sounds/GEO_Voice.uax']:
    path=GAME/name;originals.setdefault(str(path),sha(path))
for name,digest in originals.items(): assert sha(Path(name))==digest,name

scene=OUT/'geo_01a-demolition.rcplay'
apk=OUT/'republic-commando-0.25.0.apk'
assert scene.read_bytes()[:8]==b'RCPLAY08' and scene.stat().st_size==70374290
assert read('build-validation.json')['passed'] and read('build-validation.json')['sound_count']==49
assert read('build-validation.json')['static_faces_moved']==1165
v=read('playthrough/validation.json');s=v['complete_state'];d=s['demolition']
assert v['passed'] and v['input_driven_objectives']==list(range(6))
for flag in ['restart_exact','restart_frame_and_closed_collision','static_gpu_packet_unchanged',
             'closed_bunker_blocks','released_passage_walked','incorrect_aim_rejected']:
    assert v[flag],flag
assert v['armed_without_fire_seconds']==20 and 49<=v['fire_notify_delay_ticks']<=51
assert s['phase']=='Won' and s['health']==20 and s['statistics']['kills']==5
assert s['statistics']['ticks']==13397
assert d['flow']['objective_completed'] and d['navigation_open'] and d['fired']
assert d['flow']['phase']=='Exploded' and not d['flow']['plunger_selected'] and d['damage']==[]
assert d['complete_tick']==13397 and d['explosion_tick']==13157
legacy=read('legacy-07-regression/validation.json')
assert legacy['passed'] and legacy['restart_exact'] and legacy['complete_state']['statistics']['ticks']==10448

native=read('native/validation.json');android=read('android/native-replay.json')
for row in [native,android]:
    assert row['passed'] and row['inputs']==13397
    sounds=row['sound_counts'];assert len(sounds)==49
    assert sounds[20:33]==[1]*13
    assert all(sounds[i]==1 for i in range(33,49) if i!=38)
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
assert tests==1055,tests
assert 'Finished' in (OUT/'clippy.log').read_text(encoding='utf-8-sig')
assert 'error:' not in (OUT/'clippy.log').read_text(encoding='utf-8-sig')
assert 'BUILD SUCCESSFUL' in (OUT/'android-build.log').read_text(encoding='utf-8-sig')
for name in ['format.log','source-diff-check.log']:
    assert not (OUT/name).read_text(encoding='utf-8-sig').strip(),name
assert "versionCode 28" in (ROOT/'android/app/build.gradle').read_text()
assert "versionName '0.25.0-scorch-demolition'" in (ROOT/'android/app/build.gradle').read_text()
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
result=dict(passed=True,playable_milestone_complete=True,version='0.25.0',version_code=28,library_tests=tests,
    completed_original_objectives=list(range(6)),entire_first_mission_complete=False,
    scene_sha256=sha(scene),apk_sha256=sha(apk),evidence=evidence,sources=sources,originals=originals,
    prior_evidence_unchanged=sum(len(p['evidence']) for p in previous),
    android_build_abis=['arm64-v8a','x86_64'],android_native_contiguous_input_replay=True,
    android_apk_gpu_touch_use_fire_crouch_audio_smoke=True,android_complete_touch_playthrough=False,
    physical_arm64_acceptance=False,
    limitations=['Selected objective5 host; original objective6 and whole first mission remain open',
        'Bounded companion navigation, targeting, equipment readiness/attachment and default setup multiplier',
        'General squad combat/follow/revive, interrupted NPC setup and full native GoalQueue remain open',
        'Tutorial3552–3555 and SpawnWarriorAttack staging remain outside live factory simulation',
        'SpawnDemoDroids has no receiver in the inspected map inventory',
        'Preplacement hologram HardwareShader and full large-bunker particle system remain open',
        'SoundMultiple selects first original stored child; bounded combat/projectile/Karma/explosion adapters remain'])
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
print(f'Frozen demolition milestone: {len(evidence)} evidence files, {len(sources)} sources, {len(originals)} originals; {tests} tests; objectives0–5 accepted')
