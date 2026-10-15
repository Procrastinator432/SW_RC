"""Bind the original Reviving health ramp, fixtures and APK without rewriting prior evidence."""
import hashlib
import json
import re
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/mission11-revival'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def read(p):return json.loads((OUT/p).read_text(encoding='utf-8-sig'))
prior=json.loads((ROOT/'analysis/mission10-squad/validation-manifest.json').read_text())
for name,row in prior['evidence'].items():
    p=ROOT/name;assert p.stat().st_size==row['bytes'] and sha(p)==row['sha256'],name
for name,digest in prior['originals'].items():assert sha(Path(name))==digest,name
scene=OUT/'geo_01a-revival.rcplay';apk=OUT/'republic-commando-0.29.1.apk'
assert sha(scene)==prior['scene_sha256']
fixture=read('behavior/validation.json');ramp=read('behavior/revival-ramp.json')
android_fixture=read('android-fixture/binding.json')
assert fixture['passed'] and ramp['passed'] and android_fixture['passed']
assert ramp==read('android-fixture/revival-ramp.json')
assert len(ramp['scorch'])==325 and len(ramp['player'])==383
for name,maximum,final in [('scorch',400,200),('player',100,50)]:
    rows=ramp[name];assert rows[0]==[0.,1]
    assert all(a[1]<=b[1]for a,b in zip(rows,rows[1:]))
    assert all(abs(health-(1+maximum/12*elapsed))<1.001 for elapsed,health in rows)
    assert rows[-1][1]==(182 if name=='scorch' else 55)
    if name=='scorch':assert fixture['scorch_revive']['health']==final and fixture['scorch_revive']['clip']=='ActionBreathe'
assert fixture['player_revive']['player_revives']==1
assert android_fixture['digests']['fixture']==sha(OUT/'android-revival-check')
assert android_fixture['digests']['scene.rcplay']==sha(scene)
assert fixture['scorch_revive']['scorch_revives']==1
native=read('native/validation.json');android=read('android/native-replay.json');ui=read('android/validation.json')
assert native['passed'] and android['passed']
assert native['scene_sha256']==sha(scene) and android['scene.rcplay_sha256']==sha(scene)
for i,name in enumerate(['playthrough/input-replay.bin','live/squad-assist-input.bin']):
    assert native['runs'][i]['input_sha256']==sha(OUT/name)
    assert native['runs'][i]['status'][4]==android['runs'][i]['phase']
    assert native['runs'][i]['status'][0]==android['runs'][i]['health']==20
    assert native['runs'][i]['inputs']==android['runs'][i]['inputs']
assert android['runs'][0]['kills']==8 and android['runs'][1]['scorch_kills']==1
assert android['librc_native.so_sha256']==sha(ROOT/'android/app/src/main/jniLibs/x86_64/librc_native.so')
for key in ['passed','apk_scene_picker','gpu_render','touch_fire','touch_use','touch_crouch_height_change','pause']:assert ui[key],key
log=(OUT/'workspace-tests.log').read_text(encoding='utf-8-sig');assert 'FAILED'not in log
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed',log)));assert tests==1071,tests
for name in ['format.log','source-diff-check.log']:assert not (OUT/name).read_text(encoding='utf-8-sig').strip()
log=(OUT/'clippy.log').read_text(encoding='utf-8-sig');assert 'Finished'in log and 'error:'not in log
assert 'BUILD SUCCESSFUL'in(OUT/'android-build.log').read_text(encoding='utf-8-sig')
assert "versionName '0.29.1-revival'"in(ROOT/'android/app/build.gradle').read_text()
with ZipFile(apk)as archive:
    assert archive.testzip()is None
    for arch in ['arm64-v8a','x86_64']:
        for lib in ['librc_native.so','librc_bridge.so']:assert archive.read(f'lib/{arch}/{lib}')==(ROOT/f'android/app/src/main/jniLibs/{arch}/{lib}').read_bytes()
names=set(prior['sources'])
for folder in ['crates','docs','scripts','android/app/src/main/java']:
    names.update(p.relative_to(ROOT).as_posix()for p in(ROOT/folder).rglob('*')if p.is_file()and p.suffix in['.rs','.md','.py','.ps1','.java','.toml','.c'])
source='analysis/reports/sources/System__engine.u/Pawn.uc';names.add(source)
sources={name:sha(ROOT/name)for name in sorted(names)}
with ZipFile(OUT/'source-snapshot.zip','w',ZIP_DEFLATED)as archive:
    for name in sources:archive.write(ROOT/name,name)
with ZipFile(OUT/'source-snapshot.zip')as archive:
    assert archive.testzip()is None
    for name,digest in sources.items():assert hashlib.sha256(archive.read(name)).hexdigest()==digest
evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p))for p in sorted(OUT.rglob('*'))if p.is_file()and p.name!='validation-manifest.json'}
result=dict(passed=True,selected_revival_health_milestone_complete=True,version='0.29.1',version_code=33,library_tests=tests,
    scene_sha256=sha(scene),scene_identical_to_prior=True,apk_sha256=sha(apk),evidence=evidence,sources=sources,originals=prior['originals'],
    prior_evidence_unchanged=len(prior['evidence']),original_reviving_source=dict(path=source,sha256=sha(ROOT/source)),
    scorch_health_samples=325,player_health_samples=383,windows_android_health_timelines_equal=True,
    native_mission_and_squad_replays=True,android_apk_gpu_touch_audio_smoke=True,
    entire_first_mission_complete=False,next_map_loaded=False,physical_arm64_acceptance=False,complete_manual_touch_playthrough=False,
    limitations=['Selected original Reviving health arithmetic and animation finish; not whole native Pawn state/TakeDamage exception parity',
        'Integer health synchronization is a port boundary; player uses existing mission MaxHealth100, Scorch original class MaxHealth400',
        'Isolated revival fixtures explicitly assign actor positions and health; separate mission regressions use normal inputs only',
        'General original AI/squad, dynamic pawn avoidance, full inscenation, GEO_01B loading and state transfer remain open'])
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
print(f'Frozen revival update: {tests} tests, {len(evidence)} evidence files; Windows/Android health timelines match; previous delivery unchanged')
