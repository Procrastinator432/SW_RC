"""Bind the optional live throw delivery and preserve the preceding evidence."""
import hashlib
import json
import re
import struct
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/mission4-live-throw'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(name):return json.loads((OUT/name).read_text(encoding='utf-8-sig'))
prior=json.loads((ROOT/'analysis/mission4-world-flight/validation-manifest.json').read_text())
for name,row in prior['evidence'].items():
    p=ROOT/name
    assert p.stat().st_size==row['bytes'] and sha(p)==row['sha256'],name
for name,digest in prior['originals'].items():assert sha(Path(name))==digest,name
scene=OUT/'geo_01a-throw.rcplay'
raw=scene.read_bytes()
assert raw[:8]==b'RCPLAY05'
at=12+struct.unpack_from('<I',raw,8)[0]
old=(ROOT/'analysis/mission4-world-flight/geo_01a-grenades-arena.rcplay').read_bytes()
mission_size=12+struct.unpack_from('<I',old,8)[0]
assert raw[at:at+mission_size]==old[:mission_size]
def static(data):
    while data[:8] in [b'RCPLAY05',b'RCPLAY04',b'RCPLAY02']:
        data=data[12+struct.unpack_from('<I',data,8)[0]:]
    assert data[:8]==b'RCPLAY01'
    return data[12+struct.unpack_from('<I',data,8)[0]:]
assert static(raw)==static(old)
v=read('playthrough/validation.json')
for field in ['passed','input_driven_bacta_crash_pickup_gun_route','queued_short_press','held_button_no_repeat',
              'fire_reload_blocked','five_consumptions_then_empty','live_world_contact621','first_fuse_once',
              'suspend_clears_pending','restart_state_frame_exact','flight_error_atomic','static_gpu_packet_immutable']:
    assert v[field],field
assert v['release_tick']==35 and v['end_tick']==57
assert v['release_visible_pixels']>0 and v['grenade_world_pass_faces']==352
assert v['complete_state']['phase']=='Playing'
assert v['complete_state']['thermal']['released']==5
assert v['complete_state']['mission']['grenades']['flow']['ammunition']==0
assert v['sound_counts']['13']==v['sound_counts']['14']==5
assert v['sound_counts']['15']>0 and v['sound_counts']['16']==1
legacy=read('legacy-regression/validation.json')
assert legacy['passed'] and legacy['won_tick']==5030 and legacy['restart_state_and_frame_exact']
abi=read('abi/live-abi-validation.json')
assert abi['passed'] and abi['scene_sha256']==sha(scene)
assert all(value for key,value in abi.items() if key!='scene_sha256')
assert read('abi/abi-validation.json')['restart_exact']
log=(OUT/'workspace-tests.log').read_text(encoding='utf-8-sig')
assert 'FAILED' not in log
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed',log)))
assert tests==1033,tests
assert 'Finished' in (OUT/'clippy.log').read_text(encoding='utf-8-sig')
assert not (OUT/'format.log').read_text(encoding='utf-8-sig').strip()
assert not (OUT/'source-diff-check.log').read_text(encoding='utf-8-sig').strip()
assert 'BUILD SUCCESSFUL' in (OUT/'android-build.log').read_text(encoding='utf-8-sig')
apk=OUT/'republic-commando-0.22.0.apk'
with ZipFile(apk) as archive:
    assert archive.testzip() is None
    for arch in ['arm64-v8a','x86_64']:
        for lib in ['librc_native.so','librc_bridge.so']:
            actual=archive.read(f'lib/{arch}/{lib}')
            assert actual==(ROOT/f'android/app/src/main/jniLibs/{arch}/{lib}').read_bytes()
    assert any(b'GRANATE' in archive.read(name) for name in archive.namelist() if name.endswith('.dex'))
assert "versionCode 25" in (ROOT/'android/app/build.gradle').read_text()
assert '0.22.0-original-grenade-throw' in (ROOT/'android/app/build.gradle').read_text()
names=set(prior['sources'])
for folder in ['crates','docs','scripts','android/app/src/main/java']:
    names.update(p.relative_to(ROOT).as_posix() for p in (ROOT/folder).rglob('*')
                 if p.is_file() and p.suffix in ['.rs','.md','.py','.ps1','.java','.toml'])
names.add('android/app/build.gradle')
sources={name:sha(ROOT/name) for name in sorted(names)}
with ZipFile(OUT/'source-snapshot.zip','w',ZIP_DEFLATED) as archive:
    for name in sources:archive.write(ROOT/name,name)
with ZipFile(OUT/'source-snapshot.zip') as archive:
    assert archive.testzip() is None
    for name,digest in sources.items():assert hashlib.sha256(archive.read(name)).hexdigest()==digest
evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p))
          for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json'}
result=dict(passed=True,version='0.22.0',version_code=25,library_tests=tests,
            scene_sha256=sha(scene),apk_sha256=sha(apk),evidence=evidence,sources=sources,
            originals=prior['originals'],prior_evidence_unchanged=len(prior['evidence']),
            mission_metadata_byte_exact=True,static_snapshot_byte_exact=True,
            android_build_abis=['arm64-v8a','x86_64'],android_runtime_tested=False,
            limitations=['No bunker/hatch/collision destruction integration or section victory',
                         'No thermal explosion FX/audio or radius damage selection',
                         'Hint/confirmation sound dispatch still pending',
                         'Inherited bounded dry flight/static blocker/rest adapter; no original Karma/AI parity'])
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(f'Bound {len(evidence)} evidence files, {len(sources)} sources, {len(prior["originals"])} originals; {tests} tests; scene {sha(scene)}')
