"""Freeze and bind the completed selected gun-position delivery and its evidence."""
import hashlib
import json
import re
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/mission4-complete'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def read(name):return json.loads((OUT/name).read_text(encoding='utf-8-sig'))
prior=json.loads((ROOT/'analysis/mission4-live-throw/validation-manifest.json').read_text())
for name,row in prior['evidence'].items():
    p=ROOT/name
    assert p.stat().st_size==row['bytes'] and sha(p)==row['sha256'],name
originals=prior['originals'].copy()
game=Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData')
for name in ['Properties/cteffects.u','Properties/physicalmaterials.u','Textures/EffectTex.utx','StaticMeshes/Arena_HR.usx']:
    p=game/name
    originals[str(p)]=sha(p)
for name,digest in originals.items():assert sha(Path(name))==digest,name
v=read('playthrough/validation.json')
assert v['passed'] and v['input_driven_objectives']==[0,1,2,3]
assert v['complete_state']['phase']=='Won' and v['complete_state']['health']==20
assert v['complete_state']['statistics']['kills']==5
assert v['complete_state']['dynamic']==[True]*10
for name in ['thermal-explosion','bunker-explosion']:
    assert read(f'playthrough/{name}-visibility.json')['visible_pixels']>0
scene=OUT/'geo_01a-gunner.rcplay'
assert scene.read_bytes()[:8]==b'RCPLAY06'
assert read('native/abi-validation.json')['scene_sha256']==sha(scene)
assert read('native/abi-validation.json')['restart_exact']
assert read('android/validation.json')['passed']
android=read('android/native-replay.json')
assert android['passed'] and android['phase']==1 and android['health']==20 and android['kills']==5
assert android['scene.rcplay_sha256']==sha(scene)
assert android['librc_native.so_sha256']==sha(ROOT/'android/app/src/main/jniLibs/x86_64/librc_native.so')
assert read('legacy-05-regression/validation.json')['passed']
assert read('legacy-04-regression/validation.json')['won_tick']==5030
assert len(android['sound_counts'])==20
for role in [13,14,17,18,19]:assert android['sound_counts'][role]==1
log=(OUT/'workspace-tests.log').read_text(encoding='utf-8-sig')
assert 'FAILED' not in log
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed',log)))
assert tests==1036,tests
assert 'Finished' in (OUT/'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log','source-diff-check.log']:assert not (OUT/name).read_text(encoding='utf-8-sig').strip()
assert 'BUILD SUCCESSFUL' in (OUT/'android-build.log').read_text(encoding='utf-8-sig')
apk=OUT/'republic-commando-0.23.0.apk'
with ZipFile(apk) as archive:
    assert archive.testzip() is None
    for arch in ['arm64-v8a','x86_64']:
        for lib in ['librc_native.so','librc_bridge.so']:
            assert archive.read(f'lib/{arch}/{lib}')==(ROOT/f'android/app/src/main/jniLibs/{arch}/{lib}').read_bytes()
assert 'versionCode 26' in (ROOT/'android/app/build.gradle').read_text()
names=set(prior['sources'])
for folder in ['crates','docs','scripts','android/app/src/main/java']:
    names.update(p.relative_to(ROOT).as_posix() for p in (ROOT/folder).rglob('*') if p.is_file() and p.suffix in ['.rs','.md','.py','.ps1','.java','.toml','.c'])
names.add('android/app/build.gradle')
sources={name:sha(ROOT/name) for name in sorted(names)}
with ZipFile(OUT/'source-snapshot.zip','w',ZIP_DEFLATED) as archive:
    for name in sources:archive.write(ROOT/name,name)
with ZipFile(OUT/'source-snapshot.zip') as archive:
    assert archive.testzip() is None
    for name,digest in sources.items():assert hashlib.sha256(archive.read(name)).hexdigest()==digest
evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json'}
result=dict(passed=True,version='0.23.0',version_code=26,library_tests=tests,scene_sha256=sha(scene),apk_sha256=sha(apk),
            evidence=evidence,sources=sources,originals=originals,prior_evidence_unchanged=len(prior['evidence']),
            completed_original_objectives=[0,1,2,3],android_build_abis=['arm64-v8a','x86_64'],
            android_native_mission_replay=True,android_apk_gpu_touch_audio_smoke=True,
            android_complete_touch_playthrough=False,physical_arm64_acceptance=False,
            limitations=['Selected gun-position section, not the entire first original mission',
                         'Bounded combat, dry projectile/collision and resting Karma adapters',
                         'Deterministic original sprites with cutout; no full original particles, lights, impulse or material selection',
                         'Following SpawnSmallBunker/Conversation04 section not integrated'])
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(f'Frozen {len(evidence)} evidence files, {len(sources)} sources, {len(originals)} originals; {tests} tests; scene {sha(scene)}')
