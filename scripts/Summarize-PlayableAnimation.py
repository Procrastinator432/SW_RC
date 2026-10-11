"""Check M2 receipts and bind original inputs, final sources and deliverables by SHA-256."""
import hashlib
import json
import platform
import re
import statistics
import struct
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/milestone2'
GAME = Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData')

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def read(name):
    return json.loads((OUT / name).read_text(encoding='utf-8'))

def records(path):
    return [json.loads(line[line.index('{'):]) for line in path.read_text(encoding='utf-8').splitlines() if '{' in line]

def stats(values):
    values = sorted(values)
    assert values
    x = (len(values)-1)*.95
    i = int(x)
    return dict(samples=len(values), median=statistics.median(values),
                p95=values[i]+(values[min(i+1,len(values)-1)]-values[i])*(x-i))

def main():
    animation = read('animation-validation.json')
    assert animation['fixed_tick_visual_cadence_exact']
    assert len(animation['clips']) == 9 and all(c['pose_changes'] and c['repeat_exact'] for c in animation['clips'])
    for key in ['instance_isolation','hit_fire_reload_death','death_holds_last_frame','pause_render_read_only',
                'restart_exact','invalid_tick_rollback','weapon_camera_attachment','corrupt_mapping_rejected']:
        assert animation[key], key
    graphics = read('graphics/validation.json')
    assert graphics['context_rebuild_pixels_exact'] and graphics['short_reload_memory_growth_bounded']
    assert graphics['cpu_gpu_spawn_channels_within_two'] > .98 and graphics['cpu_gpu_spawn_mean_channel_error'] < 2
    assert graphics['query_cache_invalidates_on_advance_and_restart']
    inputs = read('input/validation.json')
    assert inputs['final_look_release_exact'] and inputs['simultaneous_move_look_fire'] and inputs['picker_cancel_recovers']
    abi = read('abi-validation.json')
    assert abi['restart_exact'] and abi['short_buffers_and_canaries'] and abi['invalid_input_rollback']
    assert read('legacy-replay-check.json')['exact_replay']
    baseline = ROOT/'analysis/baselines/0.8.0-playable-scene'
    previous = json.loads((baseline/'analysis/playable/validation-manifest.json').read_text(encoding='utf-8'))
    assert all(sha(OUT/'legacy'/name)==expected for name,expected in previous['frame_sha256'].items())
    log = (OUT/'workspace-tests.log').read_text(encoding='utf-8')
    assert 'FAILED' not in log
    tests = sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)))
    assert tests == 937, tests
    assert not (OUT/'format.log').read_text(encoding='utf-8').strip()
    assert 'Finished' in (OUT/'clippy.log').read_text(encoding='utf-8')
    assert not (OUT/'diff-check.log').read_text(encoding='utf-8').strip()
    assert 'BUILD SUCCESSFUL' in (OUT/'android-build.log').read_text(encoding='utf-8')
    assert 'Verified using v2 scheme (APK Signature Scheme v2): true' in (OUT/'apksigner.log').read_text(encoding='utf-8')
    assert 'Verification succes' in (OUT/'zipalign.log').read_text(encoding='utf-8')
    assert "versionCode='12'" in (OUT/'apk-badging.log').read_text(encoding='utf-8')
    performance = {}
    gameplay = {}
    for backend,directory in [('gpu','android'),('cpu','cpu-android')]:
        check = read(directory+'/validation.json')
        assert check['backend']==backend and check['three_kills'] and check['touch_reload'] and check['no_crash']
        gameplay[backend] = check
        frames = [r for r in records(OUT/directory/'performance.log') if r['type']=='frame']
        assert len(frames)>30 and {0,1,2}.issubset({f['phase'] for f in frames})
        active = [f for f in frames if not f['still'] and f['interval_ns']>0]
        result = dict(frame_samples=len(frames),host_submission_interval_ns=stats([f['interval_ns'] for f in active]),
                      jni_worker_ns=stats([f['jni_wall_ns'] for f in frames]),
                      actor_geometry_ns=stats([f['ns'][2] for f in frames if f['calls'][2]]),
                      max_discarded_seconds=max(f['discarded_seconds'] for f in frames))
        if backend=='gpu':
            assert all(f['calls'][3]==0 and f['calls'][7]==0 for f in frames)
            result['gl_cpu_submission_ns'] = stats([r['submit_ns'] for r in records(OUT/directory/'gpu.log') if r['type']=='draw'])
        else:
            result['cpu_scene_draw_ns'] = stats([f['ns'][3] for f in frames if f['calls'][3]])
        performance[backend] = result
    performance['environment'] = dict(host=platform.platform(),android_api=35,abi='x86_64',resolution=[640,400],
        scope='Emulator software GPU; host submission intervals and CPU work, not display FPS/GPU timer queries. Physical ARM64 M2 test and sustained acceptance remain open.')
    (OUT/'performance-summary.json').write_text(json.dumps(performance,indent=2),encoding='utf-8')
    apk = ROOT/'android/app/build/outputs/apk/debug/app-debug.apk'
    elf = {}
    with zipfile.ZipFile(apk) as archive:
        assert not any(n.lower().endswith(('.ctm','.usx','.utx','.rcplay','.rcscene')) for n in archive.namelist())
        for architecture,machine in [('arm64-v8a',183),('x86_64',62)]:
            for library in ['librc_native.so','librc_bridge.so']:
                name=f'lib/{architecture}/{library}'
                binary=archive.read(name)
                assert binary[:6]==b'\x7fELF\x02\x01' and struct.unpack_from('<H',binary,18)[0]==machine
                offset=struct.unpack_from('<Q',binary,32)[0]
                size,count=struct.unpack_from('<HH',binary,54)
                assert size==56
                alignments=[]
                for i in range(count):
                    kind,_,file_offset,address,_,_,_,alignment=struct.unpack_from('<IIQQQQQQ',binary,offset+i*size)
                    if kind==1:
                        assert alignment>=16384 and file_offset%alignment==address%alignment
                        alignments.append(alignment)
                assert alignments
                elf[name]=alignments
    sources=[]
    for parent,suffixes in [('crates',{'.rs','.toml'}),('scripts',{'.py','.ps1','.java','.xml'}),
                            ('docs',{'.md'}),('android',{'.java','.xml','.c','.gradle'})]:
        sources.extend(p for p in (ROOT/parent).rglob('*') if p.suffix in suffixes and
                       not any(x in p.parts for x in ['build','.gradle','jniLibs','__pycache__']))
    sources += [ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'README.md',ROOT/'.gitignore']
    artifacts=[p for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json'
               and 'input-probe' not in p.parts and not p.name.endswith('-summary.log')]
    artifacts += [apk,OUT/'input-probe/input-probe.apk',ROOT/'target/release/rc_native.dll',ROOT/'target/release/rc-play-animation-check.exe',ROOT/'target/release/rc-play-animated.exe',ROOT/'target/release/rc-play-check.exe']
    originals=[GAME/p for p in ['Maps/geo_01a.ctm','Animations/geonosian.ukx','Animations/hudarms.ukx',
                               'Textures/GeonosianTextures.utx','Textures/HudArmsTextures.utx',
                               'System/engine.u','System/ctinventory.u','System/ctcharacters.u']]
    manifest=dict(schema=1,milestone='2: original animated enemy and first-person weapon',version='0.10.0-animated-scene',
        rust_tests=tests,animation=animation,graphics=graphics,input=inputs,gameplay=gameplay,abi=abi,
        legacy_replay_commands=1002,legacy_five_frames_exact=True,performance=performance,elf_alignments=elf,
        original_sha256={str(p):sha(p) for p in originals},
        source_sha256={p.relative_to(ROOT).as_posix():sha(p) for p in sorted(set(sources))},
        artifact_sha256={p.relative_to(ROOT).as_posix():sha(p) for p in sorted(set(artifacts))},
        remaining='Original combat/AI rules, notify/audio execution, weapon digits and full material parity: M3/later. Physical ARM64 M2 and quantitative duration test open.')
    (OUT/'validation-manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
    print(json.dumps(dict(tests=tests,apk_sha256=sha(apk),scene_sha256=sha(OUT/'geo_01a-animated.rcplay'),performance=performance),indent=2))

if __name__=='__main__':
    main()
