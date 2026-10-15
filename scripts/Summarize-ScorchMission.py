"""Freeze selected objective4 delivery, original inputs and explicit acceptance limits."""
import hashlib
import json
import re
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT/'analysis/mission5-playable'
GAME = Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData')
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def read(name): return json.loads((OUT/name).read_text(encoding='utf-8-sig'))

prior = json.loads((ROOT/'analysis/mission4-complete/validation-manifest.json').read_text())
prepared = json.loads((ROOT/'analysis/mission5-transition/validation-manifest.json').read_text())
for manifest in [prior,prepared]:
    for name,row in manifest['evidence'].items():
        p=ROOT/name
        assert p.stat().st_size==row['bytes'] and sha(p)==row['sha256'],name
originals=prepared['originals'].copy()
for folder in ['System','Properties']:
    for p in (GAME/folder).glob('*.u'): originals.setdefault(str(p),sha(p))
for name in ['Animations/Clone.ukx','Textures/CloneTextures.utx','StaticMeshes/Arena_HR.usx']:
    p=GAME/name;originals.setdefault(str(p),sha(p))
for name,digest in originals.items(): assert sha(Path(name))==digest,name

v=read('playthrough/validation.json')
assert v['passed'] and v['input_driven_objectives']==[0,1,2,3,4]
s=v['complete_state'];r=s['rendezvous']
assert s['phase']=='Won' and s['health']==20 and s['statistics']['kills']==5
assert r['motion']==4 and all(r['flow'][k] for k in ['squad_joined','objective_completed','charge_enabled'])
assert r['intro_seen'] and r['behind_seen'] and r['completed_tick']==10448
assert s['bunker']['complete_tick']==7296
assert v['restart_exact'] and v['static_gpu_packet_unchanged'] and v['blocked_uncrouch_checked']
assert read('route-crouched/route-validation.json')['passed']
assert read('build-validation.json')['additional_original_bunker_faces']==1165
assert read('legacy-06-regression/validation.json')['passed']
assert read('legacy-06-regression/validation.json')['complete_state']['bunker']['complete_tick']==6045
assert len(read('rendezvous-bindings.json')['actions'])==55
assert len(read('rendezvous-bindings.json')['sound_bank'])==13
scene=OUT/'geo_01a-scorch.rcplay';apk=OUT/'republic-commando-0.24.0.apk'
assert scene.read_bytes()[:8]==b'RCPLAY07'
native=read('native/validation.json');android=read('android/native-replay.json')
assert native['passed'] and native['scene_sha256']==sha(scene) and native['inputs']==10448
assert native['sound_counts'][20:]==[1]*13
assert android['passed'] and android['phase']==1 and android['health']==20 and android['kills']==5
assert android['inputs']==10448 and android['sound_counts'][20:]==[1]*13
assert android['scene.rcplay_sha256']==sha(scene)
assert android['librc_native.so_sha256']==sha(ROOT/'android/app/src/main/jniLibs/x86_64/librc_native.so')
assert read('android/validation.json')['passed'] and read('android/validation.json')['touch_crouch_height_change']
assert read('android/crouch-input-validation.json')['passed']
log=(OUT/'workspace-tests.log').read_text(encoding='utf-8-sig')
assert 'FAILED' not in log
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed',log)))
assert tests==1044,tests
assert '79 passed; 0 failed' in (OUT/'final-game-tests.log').read_text(encoding='utf-8-sig')
assert 'Finished' in (OUT/'clippy.log').read_text(encoding='utf-8-sig')
assert 'BUILD SUCCESSFUL' in (OUT/'android-build.log').read_text(encoding='utf-8-sig')
for name in ['format.log','source-diff-check.log']: assert not (OUT/name).read_text(encoding='utf-8-sig').strip()
assert 'versionCode 27' in (ROOT/'android/app/build.gradle').read_text()
with ZipFile(apk) as archive:
    assert archive.testzip() is None
    for arch in ['arm64-v8a','x86_64']:
        for lib in ['librc_native.so','librc_bridge.so']:
            assert archive.read(f'lib/{arch}/{lib}')==(ROOT/f'android/app/src/main/jniLibs/{arch}/{lib}').read_bytes()
names=set(prior['sources'])
for folder in ['crates','docs','scripts','android/app/src/main/java']:
    names.update(p.relative_to(ROOT).as_posix() for p in (ROOT/folder).rglob('*') if p.is_file() and p.suffix in ['.rs','.md','.py','.ps1','.java','.toml','.c'])
names.add('android/app/build.gradle')
sources={name:sha(ROOT/name) for name in sorted(names)}
with ZipFile(OUT/'source-snapshot.zip','w',ZIP_DEFLATED) as archive:
    for name in sources: archive.write(ROOT/name,name)
with ZipFile(OUT/'source-snapshot.zip') as archive:
    assert archive.testzip() is None
    for name,digest in sources.items(): assert hashlib.sha256(archive.read(name)).hexdigest()==digest
evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json'}
result=dict(passed=True,version='0.24.0',version_code=27,library_tests=tests,scene_sha256=sha(scene),apk_sha256=sha(apk),
            evidence=evidence,sources=sources,originals=originals,prior_evidence_unchanged=len(prior['evidence'])+len(prepared['evidence']),
            completed_original_objectives=[0,1,2,3,4],entire_first_mission_complete=False,
            android_build_abis=['arm64-v8a','x86_64'],android_native_mission_replay=True,android_apk_gpu_touch_crouch_audio_smoke=True,
            android_complete_touch_playthrough=False,physical_arm64_acceptance=False,
            limitations=['Selected playable rendezvous; demolition command and spire objectives still open',
                         'Bounded companion controller; no general Squad GoalQueue, combat/follow, weapon attachment or revive',
                         'Additional battlefield/intro factory spawning and burning-bunker staging remain emitted requests, not full live simulation',
                         'Procedural rockImpact SoundMultiple remains outside the selected PCM fragment',
                         'Existing bounded combat, projectile, Karma and explosion adapters remain',
                         'Earlier standing/jumping-only route reports are failed diagnostics; accepted path uses crouch'])
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
print(f'Frozen Scorch milestone: {len(evidence)} evidence files, {len(sources)} sources, {len(originals)} originals; {tests} tests; scene {sha(scene)}')
