"""Freeze the selected landing staging and explicitly retain original-cinematic gaps."""
import hashlib
import json
import re
from pathlib import Path
from zipfile import ZipFile,ZIP_DEFLATED
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/mission12-opening'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def read(name):return json.loads((OUT/name).read_text(encoding='utf-8-sig'))
prior=json.loads((ROOT/'analysis/mission11-revival/validation-manifest.json').read_text())
for name,row in prior['evidence'].items():
    p=ROOT/name;assert p.stat().st_size==row['bytes']and sha(p)==row['sha256'],name
for name,digest in prior['originals'].items():assert sha(Path(name))==digest,name
scene=OUT/'geo_01a-opening.rcplay';apk=OUT/'republic-commando-0.30.0.apk'
raw=scene.read_bytes();assert raw[:8]==b'RCPLAY13';n=int.from_bytes(raw[8:12],'little');profile=json.loads(raw[12:12+n])
assert raw[12+n:]==(ROOT/'analysis/mission11-revival/geo_01a-revival.rcplay').read_bytes()
assert profile['initial_health']==200 and len(profile['sounds'])==11
assert sorted(p['original']for p in profile['programs'])==[12,497,523]
check=read('playthrough/validation.json');assert check['passed']and check['normal_inputs_only']
assert check['opening_ticks']==1925 and check['injury_tick']==1397
for flag in ['locked_controls_tested','non_waiting_sound_overlap','restart_exact_state_frame','opening_144hz_events_equal']:assert check[flag],flag
state=check['complete_state'];assert state['phase']=='Won'and state['health']==20
assert state['statistics']['ticks']==18565 and state['statistics']['kills']+state['squad']['kills']==8
native=read('native/validation.json');android=read('android/native-replay.json');ui=read('android/validation.json')
for result in [native,android]:assert result['passed']and result['opening_injury_health_checked']and result['guarded_buffers']and result['restart']
assert native['scene_sha256']==sha(scene)and android['scene.rcplay_sha256']==sha(scene)
for i,name in enumerate(['playthrough/input-replay.bin','live/squad-assist-input.bin']):
    assert native['runs'][i]['input_sha256']==sha(OUT/name)
    assert native['runs'][i]['inputs']==android['runs'][i]['inputs']
    assert native['runs'][i]['status'][4]==android['runs'][i]['phase']
    assert native['runs'][i]['status'][0]==android['runs'][i]['health']==20
    assert native['runs'][i]['sound_counts'][65:76]==[1]*11
assert android['runs'][1]['scorch_kills']==1
assert android['librc_native.so_sha256']==sha(ROOT/'android/app/src/main/jniLibs/x86_64/librc_native.so')
for flag in ['passed','apk_scene_picker','gpu_render','touch_fire','touch_use','touch_crouch_height_change','pause']:assert ui[flag],flag
assert(OUT/'android/opening-complete.png').is_file()
for mode in ['known','unknown']:
    row=read(f'scene-reader-{mode}.json');assert row['passed']and row['sha256']==sha(scene)and row['bytes']==len(raw)and row['heap_limit']==112*1024*1024
log=(OUT/'workspace-tests.log').read_text(encoding='utf-8-sig');assert'FAILED'not in log
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed',log)));assert tests==1074,tests
for name in ['format.log','source-diff-check.log']:assert not(OUT/name).read_text(encoding='utf-8-sig').strip()
log=(OUT/'clippy.log').read_text(encoding='utf-8-sig');assert'Finished'in log and'error:'not in log
assert'BUILD SUCCESSFUL'in(OUT/'android-build.log').read_text(encoding='utf-8-sig')
assert"versionName '0.30.0-opening'"in(ROOT/'android/app/build.gradle').read_text()
with ZipFile(apk)as archive:
    assert archive.testzip()is None
    for arch in ['arm64-v8a','x86_64']:
        for lib in ['librc_native.so','librc_bridge.so']:assert archive.read(f'lib/{arch}/{lib}')==(ROOT/f'android/app/src/main/jniLibs/{arch}/{lib}').read_bytes()
names=set(prior['sources'])
for folder in ['crates','docs','scripts','android/app/src/main/java']:
    names.update(p.relative_to(ROOT).as_posix()for p in(ROOT/folder).rglob('*')if p.is_file()and p.suffix in['.rs','.md','.py','.ps1','.java','.toml','.c'])
sources={name:sha(ROOT/name)for name in sorted(names)}
with ZipFile(OUT/'source-snapshot.zip','w',ZIP_DEFLATED)as archive:
    for name in sources:archive.write(ROOT/name,name)
with ZipFile(OUT/'source-snapshot.zip')as archive:
    assert archive.testzip()is None
    for name,digest in sources.items():assert hashlib.sha256(archive.read(name)).hexdigest()==digest
evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p))for p in sorted(OUT.rglob('*'))if p.is_file()and p.name!='validation-manifest.json'}
result=dict(passed=True,selected_landing_briefing_injury_milestone_complete=True,version='0.30.0',version_code=34,library_tests=tests,
    scene_sha256=sha(scene),apk_sha256=sha(apk),evidence=evidence,sources=sources,originals=prior['originals'],prior_evidence_unchanged=len(prior['evidence']),
    opening_inputs=1925,scripted_injury_tick=1397,total_inputs=18565,total_kills=8,normal_health=20,sound_bank=76,new_original_samples=11,
    windows_android_opening_mission_and_squad_replays=True,android_apk_opening_to_bacta_gpu_touch_audio=True,
    entire_intro_complete=False,entire_first_mission_complete=False,next_map_loaded=False,physical_arm64_acceptance=False,complete_manual_touch_playthrough=False,
    limitations=['Selected scripts12/523/497 and actor580 animation host; no general script VM',
        'Stationary collision-checked exterior port camera; original rope/EntranceRope600 camera and player motion remain open',
        'Intro Warrior, decorative LZ troopers, retreat/overflight, general spawn/inventory/collision/invulnerability actor effects remain open',
        'Random timer uses deterministic midpoint; explosion sprites, health blink and comment-derived captions are explicit host rules; full visor/particle/light/intro notify parity remains open',
        'Temporary original200HP minus175 damage reaches25; existing post-injury mission MaxHealth100 retained',
        'General AI/squad parity and GEO_01B loading/state transfer remain open'])
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
print(f'Frozen opening milestone: {tests} tests, {len(evidence)} evidence files; prior delivery unchanged')
