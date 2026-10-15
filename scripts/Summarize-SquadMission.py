"""Freeze the selected enemies/Scorch delivery with explicit evidence boundaries."""
import hashlib
import json
import re
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/mission10-squad'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def read(name):return json.loads((OUT/name).read_text(encoding='utf-8-sig'))
prior=json.loads((ROOT/'analysis/mission9-ai/validation-manifest.json').read_text())
for name,row in prior['evidence'].items():
    p=ROOT/name;assert p.stat().st_size==row['bytes'] and sha(p)==row['sha256'],name
originals=prior['originals'].copy()
for name,digest in originals.items():assert sha(Path(name))==digest,name
scene=OUT/'geo_01a-squad.rcplay';apk=OUT/'republic-commando-0.29.0.apk'
raw=scene.read_bytes();assert raw[:8]==b'RCPLAY12'
size=int.from_bytes(raw[8:12],'little');profile=json.loads(raw[12:12+size])
assert raw[12+size:]==(ROOT/'analysis/mission9-ai/geo_01a-warrior-ai.rcplay').read_bytes()
assert profile['squad']['damage']==20 and profile['squad']['projectile_speed']==6000
assert profile['squad']['max_health']==400 and profile['squad']['health']==100
assert profile['squad']['shield_rate']==15 and profile['squad']['shield_delay']==4
assert len(profile['squad']['sounds'])==1
normal=read('playthrough/validation.json');live=read('live/validation.json')
behavior=read('behavior/validation.json');motion=read('motion/validation.json')
for value in [normal,live,behavior,motion]:assert value['passed']
state=normal['complete_state'];assert state['phase']=='Won' and state['health']>0
assert state['statistics']['kills']+state['squad']['kills']==8
assert normal['restart_exact'] and normal['released_barricade_clear_sweep']
assert live['normal_inputs_only'] and live['after']['squad']['kills']>0
assert behavior['player_revive']['player_revives']==1 and behavior['scorch_revive']['scorch_revives']==1
assert behavior['both_incapacitated_loses'] and behavior['enemy_selects_scorch']
assert len(motion['lunges'])==2 and motion['jump']['clips']==['TakeOff','FallBreathe','Land']
native=read('native/validation.json');android=read('android/native-replay.json');ui=read('android/validation.json')
assert native['passed'] and android['passed'] and native['guarded_buffers'] and android['guarded_buffers']
assert native['scene_sha256']==sha(scene) and android['scene.rcplay_sha256']==sha(scene)
for i,name in enumerate(['playthrough/input-replay.bin','live/squad-assist-input.bin']):
    assert native['runs'][i]['input_sha256']==sha(OUT/name)
    assert native['runs'][i]['inputs']==android['runs'][i]['inputs']
    assert native['runs'][i]['status'][4]==android['runs'][i]['phase']
    assert native['runs'][i]['squad'][9]==android['runs'][i]['scorch_kills']
assert android['runs'][1]['scorch_sound_events']>0
assert android['librc_native.so_sha256']==sha(ROOT/'android/app/src/main/jniLibs/x86_64/librc_native.so')
for flag in ['passed','apk_scene_picker','gpu_render','touch_fire','touch_use','touch_crouch_height_change','pause']:assert ui[flag],flag
for mode in ['known','unknown']:
    row=read(f'scene-reader-{mode}.json');assert row['passed'] and row['sha256']==sha(scene) and row['bytes']==len(raw) and row['heap_limit']==112*1024*1024
log=(OUT/'workspace-tests.log').read_text(encoding='utf-8-sig');assert 'FAILED' not in log
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed',log)));assert tests>=1068
clippy=(OUT/'clippy.log').read_text(encoding='utf-8-sig');assert 'Finished' in clippy and 'error:' not in clippy
for name in ['format.log','source-diff-check.log']:assert not (OUT/name).read_text(encoding='utf-8-sig').strip()
assert 'BUILD SUCCESSFUL' in (OUT/'android-build.log').read_text(encoding='utf-8-sig')
assert "versionName '0.29.0-squad'" in (ROOT/'android/app/build.gradle').read_text()
with ZipFile(apk)as archive:
    assert archive.testzip() is None
    for arch in ['arm64-v8a','x86_64']:
        for lib in ['librc_native.so','librc_bridge.so']:assert archive.read(f'lib/{arch}/{lib}')==(ROOT/f'android/app/src/main/jniLibs/{arch}/{lib}').read_bytes()
names=set(prior['sources'])
for folder in ['crates','docs','scripts','android/app/src/main/java']:
    names.update(p.relative_to(ROOT).as_posix() for p in (ROOT/folder).rglob('*')if p.is_file() and p.suffix in ['.rs','.md','.py','.ps1','.java','.toml','.c'])
names.update(['README.md','Cargo.toml','Cargo.lock','android/app/build.gradle','android/app/src/main/AndroidManifest.xml'])
sources={name:sha(ROOT/name)for name in sorted(names)}
with ZipFile(OUT/'source-snapshot.zip','w',ZIP_DEFLATED)as archive:
    for name in sources:archive.write(ROOT/name,name)
with ZipFile(OUT/'source-snapshot.zip')as archive:
    assert archive.testzip()is None
    for name,digest in sources.items():assert hashlib.sha256(archive.read(name)).hexdigest()==digest
evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p))for p in sorted(OUT.rglob('*'))if p.is_file() and p.name!='validation-manifest.json'}
result=dict(passed=True,selected_enemies_scorch_milestone_complete=True,version='0.29.0',version_code=32,library_tests=tests,
    entire_first_mission_complete=False,next_map_loaded=False,physical_arm64_acceptance=False,complete_manual_touch_playthrough=False,
    normal_inputs=state['statistics']['ticks'],normal_health=state['health'],normal_total_kills=8,
    squad_assist_inputs=live['inputs'],squad_assist_shots=live['after']['squad']['shots'],squad_assist_hits=live['after']['squad']['hits'],squad_assist_kills=live['after']['squad']['kills'],
    isolated_squad_fixtures=True,original_size_lunge_jump_probes=True,windows_android_input_replays=True,android_apk_gpu_touch_audio_smoke=True,
    scene_sha256=sha(scene),apk_sha256=sha(apk),evidence=evidence,sources=sources,originals=originals,prior_evidence_unchanged=len(prior['evidence']),
    limitations=['Selected Scorch host, not general four-member/native CTBot/GoalQueue/cover/grenade/perch parity',
        'Static collision, explicit navigation/perception/targeting/random-start/ballistic scheduling adapters; dynamic pawn avoidance remains open',
        'Selected projectile/blast damage hosts; full Game.ReduceDamage/team/friendly-fire parity and original Reviving health ramp remain open',
        'Mission player health uses existing MaxHealth100; Scorch original class Health100/MaxHealth400, Shields100/MaxShields50',
        'Existing travel request stops selected section without a general squad travel host; Scorch can remain behind at the exit',
        'Full introduction/inscenation, GEO_01B loading and state transfer, full touch and physical ARM64 acceptance remain open'])
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
print(f'Frozen enemies/Scorch milestone: {tests} tests, {len(evidence)} evidence files, {len(sources)} sources; previous evidence unchanged')
