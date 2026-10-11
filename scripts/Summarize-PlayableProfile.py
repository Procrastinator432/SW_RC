"""Consolidate actual Android CPU-stage observations and milestone acceptance receipts."""
import hashlib
import json
import math
import platform
import re
import statistics
import struct
import zipfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/milestone0'
CATEGORIES=['simulation','collision','actor_geometry','scene_draw','raster','frame_buffers','hud','native_pixel_copy','jni_arrays','bundle_load']


def sha(path):
    value=hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda:stream.read(1024*1024),b''):value.update(block)
    return value.hexdigest()


def percentile(values):
    values=sorted(values)
    assert values
    return {'n':len(values),'p50_ms':statistics.median(values)/1e6,
            'p95_ms':values[math.ceil(len(values)*0.95)-1]/1e6,
            'p99_ms':values[math.ceil(len(values)*0.99)-1]/1e6}


def records(path):
    result=[]
    for line in path.read_text(encoding='utf-8').splitlines():
        if '{' in line: result.append(json.loads(line[line.index('{'):]))
    return result


def main():
    cpu=json.loads((OUT/'cpu-summary.json').read_text())
    android=json.loads((OUT/'android/validation.json').read_text())
    inputs=json.loads((OUT/'input/validation.json').read_text())
    abi=json.loads((OUT/'abi-validation.json').read_text())
    for report in [android,inputs]:
        assert all(v for v in report.values() if isinstance(v,bool)),report
    assert abi['profile_frame_and_restart_exact'] and abi['read_only_pose_and_real_movement']
    observations=records(OUT/'android/performance.log')
    frames=[r for r in observations if r['type']=='frame']
    load=[r for r in observations if r['type']=='load']
    assert len(frames)>=30 and load
    assert all(len(f['ns'])==10 and len(f['calls'])==10 and f['calls'][4]==0 for f in frames)
    assert {0,1,2}.issubset({f['phase'] for f in frames})
    active=[f for f in frames if not f['still'] and f['interval_ns']>0]
    stages={name:percentile([f['ns'][i] for f in frames if f['calls'][i]])
            for i,name in enumerate(CATEGORIES) if any(f['calls'][i] for f in frames)}
    combined=sum(f['ns'][0]+f['ns'][2]+f['ns'][3]+f['ns'][6]+f['ns'][7]+f['ns'][8] for f in frames)
    draw=sum(f['ns'][3] for f in frames)
    assert combined>0 and draw>0
    android_summary={'android_api':android['android_api'],'abi':android['abi'],'build':'APK 0.8.1-profiled-scene; Rust release, Java debug',
        'host_environment':platform.platform(),'resolution':[640,400],'frame_samples':len(frames),
        'load_read':percentile([r['read_ns'] for r in load]),'load_create':percentile([r['create_ns'] for r in load]),
        'cpu_stages':stages,'host':{name:percentile([f[name] for f in frames]) for name in
            ['jni_wall_ns','worker_wait_ns','ui_wait_ns','bitmap_submit_ns','submit_latency_ns']},
        'active_submission_interval':percentile([f['interval_ns'] for f in active]),
        'scene_draw_share_of_nonoverlapping_native_stages':draw/combined,
        'peak_java_heap_bytes':max(f['java_heap_bytes'] for f in frames),
        'peak_native_heap_bytes':max(f['native_heap_bytes'] for f in frames),
        'max_discarded_seconds_within_session':max(f['discarded_seconds'] for f in frames),
        'raster_detail_measured':False,
        'scope':'Software GPU emulator; CPU draw and bitmap submission measured, actual display presentation/GPU time and physical ARM64 not measured. Native stages inclusive; collision is inside simulation, frame buffers inside scene_draw. Raster subdivision uses separate desktop detailed capture.'}
    (OUT/'android-summary.json').write_text(json.dumps(android_summary,indent=2),encoding='utf-8')
    test_log=(ROOT/'analysis/reports/milestone0-workspace-tests.log').read_text()
    assert 'test result: FAILED' not in test_log
    tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',test_log)))
    assert tests>=934
    assert 'Finished' in (ROOT/'analysis/reports/milestone0-clippy.log').read_text()
    assert not (ROOT/'analysis/reports/milestone0-format.log').read_text().strip()
    assert 'BUILD SUCCESSFUL' in (ROOT/'analysis/reports/milestone0-android-build.log').read_text()
    assert 'Verified using v2 scheme (APK Signature Scheme v2): true' in (ROOT/'analysis/reports/milestone0-apksigner.log').read_text()
    assert 'Verification succes' in (ROOT/'analysis/reports/milestone0-zipalign.log').read_text()
    apk=ROOT/'android/app/build/outputs/apk/debug/app-debug.apk'
    elf={}
    with zipfile.ZipFile(apk) as archive:
        assert not any(n.lower().endswith(('.ctm','.usx','.utx','.rcplay','.rcscene')) for n in archive.namelist())
        for architecture,machine in [('arm64-v8a',183),('x86_64',62)]:
            for library in ['librc_native.so','librc_bridge.so']:
                name=f'lib/{architecture}/{library}'; binary=archive.read(name)
                assert binary[:6]==b'\x7fELF\x02\x01' and struct.unpack_from('<H',binary,18)[0]==machine
                offset=struct.unpack_from('<Q',binary,32)[0]
                size,count=struct.unpack_from('<HH',binary,54)
                assert size==56 and 0<count<1024
                alignments=[]
                for i in range(count):
                    kind,_,file_offset,address,_,_,_,alignment=struct.unpack_from('<IIQQQQQQ',binary,offset+i*size)
                    if kind==1:
                        assert alignment>=16384 and file_offset%alignment==address%alignment
                        alignments.append(alignment)
                assert alignments
                elf[name]={'machine':machine,'load_alignments':alignments}
    files=list(OUT.rglob('*.json'))+list(OUT.rglob('*.log'))+list(OUT.rglob('*.png'))+list(OUT.rglob('*.txt'))
    files=[p for p in files if p.name!='validation-manifest.json']
    # This script may be redirected to that log; hashing it before stdout flush
    # would create a manifest which invalidates itself immediately.
    files += [p for p in (ROOT/'analysis/reports').glob('milestone0-*.log') if p.name!='milestone0-summary.log']
    files += [apk, OUT/'input-probe/input-probe.apk']
    files += [ROOT/'target/release/rc_native.dll',ROOT/'target/release/rc-play-replay.exe',ROOT/'target/debug/rc-play-replay.exe']
    sources=[]
    for parent,suffixes in [('crates',{'.rs','.toml'}),('scripts',{'.py','.ps1','.java','.xml'}),('docs',{'.md'}),('android',{'.java','.xml','.c','.gradle'})]:
        sources.extend(p for p in (ROOT/parent).rglob('*') if p.suffix in suffixes and
            not any(x in p.parts for x in ['build','.gradle','jniLibs','__pycache__']))
    sources += [ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'README.md',ROOT/'.gitignore']
    manifest={'schema':1,'milestone':'0: reproducible reference, input replay and measured bottleneck',
        'rust_tests':tests,'baseline':'analysis/baselines/0.8.0-playable-scene/manifest.json',
        'cpu':cpu,'android':android_summary,'android_gameplay':android,'android_input':inputs,
        'abi':abi,'apk_elf':elf,'source_sha256':{p.relative_to(ROOT).as_posix():sha(p) for p in sorted(set(sources))},
        'artifact_sha256':{p.relative_to(ROOT).as_posix():sha(p) for p in sorted(set(files))},
        'next_step':'Milestone 1: simulation/render data boundary and minimal GPU world drawing. CPU raster is the main measured cost; JNI and bitmap submission are secondary.',
        'remaining':'Physical ARM64, physical simultaneous touch and sustained-device thermal/performance acceptance remain open.'}
    (OUT/'validation-manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
    print(json.dumps({'rust_tests':tests,'android_samples':len(frames),'android_scene_draw':stages['scene_draw'],
        'android_bitmap':android_summary['host']['bitmap_submit_ns'],
        'scene_draw_share':draw/combined,'multitouch':inputs['simultaneous_move_look_fire'],
        'focus_resume':inputs['resume_preserves_state_and_clears_inputs'],'manifest':str(OUT/'validation-manifest.json')},indent=2))


if __name__=='__main__':main()
