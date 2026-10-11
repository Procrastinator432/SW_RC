"""Validate milestone-1 receipts and bind the final sources/APK to their evidence."""
import hashlib
import json
import platform
import re
import statistics
import struct
import zipfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/milestone1'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def read(name):return json.loads((OUT/name).read_text(encoding='utf-8'))
def records(path):
    return [json.loads(line[line.index('{'):]) for line in path.read_text(encoding='utf-8').splitlines() if '{' in line]
def stats(values):
    values=sorted(values);assert values
    def percentile(p):
        x=(len(values)-1)*p;i=int(x);return values[i]+(values[min(i+1,len(values)-1)]-values[i])*(x-i)
    return dict(samples=len(values),median=statistics.median(values),p95=percentile(.95),p99=percentile(.99))

def main():
    graphics=read('graphics/validation.json');inputs=read('input/validation.json');abi=read('abi-validation.json')
    assert graphics['context_rebuild_pixels_exact'] and graphics['short_reload_memory_growth_bounded']
    assert graphics['cpu_gpu_spawn_channels_within_two']>.98 and graphics['cpu_gpu_spawn_mean_channel_error']<2
    assert inputs['simultaneous_move_look_fire'] and inputs['resume_preserves_state_and_clears_inputs'] and inputs['picker_cancel_recovers']
    assert abi['restart_exact'] and abi['released_handle_rejected']
    assert read('replay-check.json')['exact_replay']
    trace=json.loads((ROOT/'analysis/milestone0/replay.json').read_text(encoding='utf-8'))
    bundle=ROOT/'analysis/playable/geo_01a.rcplay';assert trace['bundle_sha256']==sha(bundle) and len(trace['entries'])==1002
    baseline=ROOT/'analysis/baselines/0.8.0-playable-scene'
    old=json.loads((baseline/'analysis/playable/validation/validation.json').read_text(encoding='utf-8'))
    new=read('legacy/validation.json');old.pop('render_seconds_640x400');new.pop('render_seconds_640x400');assert old==new
    previous=json.loads((baseline/'analysis/playable/validation-manifest.json').read_text(encoding='utf-8'))
    assert all(sha(OUT/'legacy'/name)==expected for name,expected in previous['frame_sha256'].items())
    tests_log=(ROOT/'analysis/milestone1-tests.log').read_text(encoding='utf-8')
    assert 'FAILED' not in tests_log
    tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',tests_log)));assert tests==936
    assert 'Finished' in (ROOT/'analysis/milestone1-clippy.log').read_text(encoding='utf-8')
    assert not (ROOT/'analysis/milestone1-format.log').read_text(encoding='utf-8').strip()
    assert 'BUILD SUCCESSFUL' in (ROOT/'analysis/milestone1-build.log').read_text(encoding='utf-8')
    assert 'Verified using v2 scheme (APK Signature Scheme v2): true' in (ROOT/'analysis/milestone1-apksigner.log').read_text(encoding='utf-8')
    assert 'Verification succes' in (ROOT/'analysis/milestone1-zipalign.log').read_text(encoding='utf-8')
    performance={}
    gameplay={}
    for backend,directory in [('gpu','android'),('cpu','cpu-android')]:
        check=read(directory+'/validation.json');assert check['backend']==backend and check['three_kills'] and check['no_crash']
        gameplay[backend]=check
        frames=[r for r in records(OUT/directory/'performance.log') if r['type']=='frame'];assert len(frames)>30
        assert all(f['backend']==backend for f in frames)
        assert {0,1,2}.issubset({f['phase'] for f in frames})
        active=[f for f in frames if not f['still'] and f['interval_ns']>0]
        result=dict(frame_samples=len(frames),active_submission_interval_ns=stats([f['interval_ns'] for f in active]),
            jni_worker_ns=stats([f['jni_wall_ns'] for f in frames]),ui_submission_ns=stats([f['bitmap_submit_ns'] for f in frames]),
            simulation_ns=stats([f['ns'][0] for f in frames if f['calls'][0]]),
            max_discarded_seconds=max(f['discarded_seconds'] for f in frames),
            peak_java_heap_bytes=max(f['java_heap_bytes'] for f in frames),peak_native_heap_bytes=max(f['native_heap_bytes'] for f in frames))
        if backend=='gpu':
            assert all(f['calls'][3]==0 and f['calls'][7]==0 for f in frames),'CPU world draw or full pixel copy in GPU path'
            draws=[r for r in records(OUT/directory/'gpu.log') if r['type']=='draw'];assert draws
            result['gl_cpu_submission_ns']=stats([r['submit_ns'] for r in draws])
        else:result['cpu_scene_draw_ns']=stats([f['ns'][3] for f in frames if f['calls'][3]])
        performance[backend]=result
    performance['environment']=dict(host=platform.platform(),android_api=35,abi='x86_64',resolution=[640,400],
        build='0.9.0-gpu-scene: Rust release, Java debug',driver='Emulator software GPU; renderer identity in android/gpu.log',
        scope='Submission intervals and GL CPU-call time, not GPU timer queries or actual display FPS. Two normal UI playthroughs; physical ARM64 and sustained thermal acceptance open.')
    (OUT/'performance-summary.json').write_text(json.dumps(performance,indent=2),encoding='utf-8')
    apk=ROOT/'android/app/build/outputs/apk/debug/app-debug.apk';elf={}
    with zipfile.ZipFile(apk) as archive:
        assert not any(n.lower().endswith(('.ctm','.usx','.utx','.rcplay','.rcscene')) for n in archive.namelist())
        for architecture,machine in [('arm64-v8a',183),('x86_64',62)]:
            for library in ['librc_native.so','librc_bridge.so']:
                name=f'lib/{architecture}/{library}';binary=archive.read(name)
                assert binary[:6]==b'\x7fELF\x02\x01' and struct.unpack_from('<H',binary,18)[0]==machine
                offset=struct.unpack_from('<Q',binary,32)[0];size,count=struct.unpack_from('<HH',binary,54);assert size==56
                alignments=[]
                for i in range(count):
                    kind,_,file_offset,address,_,_,_,alignment=struct.unpack_from('<IIQQQQQQ',binary,offset+i*size)
                    if kind==1:
                        assert alignment>=16384 and file_offset%alignment==address%alignment;alignments.append(alignment)
                assert alignments;elf[name]=alignments
    sources=[]
    for parent,suffixes in [('crates',{'.rs','.toml'}),('scripts',{'.py','.ps1','.java','.xml'}),('docs',{'.md'}),('android',{'.java','.xml','.c','.gradle'})]:
        sources.extend(p for p in (ROOT/parent).rglob('*') if p.suffix in suffixes and not any(x in p.parts for x in ['build','.gradle','jniLibs','__pycache__']))
    sources += [ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'README.md',ROOT/'.gitignore']
    artifacts=[p for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json']
    artifacts += [p for p in (ROOT/'analysis').glob('milestone1-*.log') if p.name!='milestone1-summary.log']
    artifacts += [apk,bundle,ROOT/'target/release/rc_native.dll',ROOT/'target/release/rc-play-replay.exe',ROOT/'analysis/milestone0/replay.json']
    manifest=dict(schema=1,milestone='1: immutable presentation and OpenGL ES scene',version='0.9.0-gpu-scene',
        rust_tests=tests,replay_commands=1002,legacy_states_and_five_frames_exact=True,
        graphics=graphics,input=inputs,gameplay=gameplay,abi=abi,performance=performance,elf_alignments=elf,
        source_sha256={p.relative_to(ROOT).as_posix():sha(p) for p in sorted(set(sources))},
        artifact_sha256={p.relative_to(ROOT).as_posix():sha(p) for p in sorted(set(artifacts))},
        remaining='Physical ARM64 10-minute p95 frame-time acceptance and real touch hardware. Original animated actors/material/combat parity remain later milestones.')
    (OUT/'validation-manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
    print(json.dumps(dict(tests=tests,graphics=graphics,performance=performance),indent=2))

if __name__=='__main__':main()
