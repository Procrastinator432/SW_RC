"""Normal Android Easy scene: observe natural jump, return, pause and restart.
Read-only native observer; no gameplay state injection or forced jump command.
"""
import argparse,hashlib,importlib.util,json,re,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('--scene',type=Path,required=True);p.add_argument('--oracle',type=Path,required=True);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
args.output.mkdir(parents=True,exist_ok=True)
spec=importlib.util.spec_from_file_location('play_ui',ROOT/'scripts/Test-PlayableEmulator.py');ui=importlib.util.module_from_spec(spec);spec.loader.exec_module(ui);ui.OUT=args.output
oracle=json.loads(args.oracle.read_text())
def read():
    log=ui.adb('logcat','-d','-s','RCJump:I','*:S')
    return log,[dict(tick=int(t),words=json.loads(w)) for t,w in re.findall(r'ticks=(\d+) (\[[0-9, ]+\])',log)]
ui.bootstrap(profile=True,scene=args.scene)
ui.adb('logcat','-c');ui.tap_node(ui.find(ui.ui(),'NEUSTART'))
deadline=time.monotonic()+90;captured=False;rows=[]
while time.monotonic()<deadline:
    log,rows=read()
    phases={v for row in rows for v in row['words'][8:]}
    if 2 in phases and not captured:ui.capture('natural-flight');captured=True
    if any(row['words'][3]>0 and row['words'][4]>0 for row in rows):break
    time.sleep(.25)
else:raise AssertionError('Natural jump and return did not complete')
assert {1,2,3,4}.issubset(phases),phases
for row in rows:
    expected=next(r for r in reversed(oracle) if r['tick']<=row['tick'])
    assert expected['words']==row['words'],(row,expected)
ui.tap_node(ui.find(ui.ui(),'PAUSE'));time.sleep(.3);_,paused=read();time.sleep(.3);_,after=read();assert paused==after
ui.capture('completed-paused')
perf=ui.adb('logcat','-d','-s','RCPerf:I','*:S')
(args.output/'performance.log').write_text(perf,encoding='utf-8')
frames=[json.loads(line[line.index('{'):]) for line in perf.splitlines() if '{' in line and '"type":"frame"' in line]
latencies=sorted(r['jni_wall_ns']/1e6 for r in frames)
performance=dict(frames=len(frames),jni_max_ms=max(latencies),jni_p95_ms=latencies[int((len(latencies)-1)*.95)],discarded_seconds=max(r['discarded_seconds'] for r in frames))
ui.adb('logcat','-c');ui.tap_node(ui.find(ui.ui(),'NEUSTART'))
deadline=time.monotonic()+4
while time.monotonic()<deadline:
    _,reset=read()
    if any(r['words']==[0]*12 for r in reset):break
    time.sleep(.1)
else:raise AssertionError('Jump reservation/counters not reset')
ui.adb('shell','am','force-stop','org.rcport.diagnostic')
crash=ui.adb('logcat','-d','-b','crash');assert 'FATAL EXCEPTION' not in crash and 'Fatal signal' not in crash
(args.output/'jump.log').write_text(log,encoding='utf-8')
result=dict(passed=True,performance=performance,scene_sha256=hashlib.sha256(args.scene.read_bytes()).hexdigest(),natural_phases=sorted(phases),native_trace_matches_headless=True,frames_observed=len(rows),return_completed=True,pause_exact=True,restart_resets=True,no_crash=True,rows=rows,scope='Easy normal DocumentsUI load and no-input natural post-melee jumps; read-only counters/phase observer. x86_64 emulator, physical ARM64 pending.')
(args.output/'validation.json').write_text(json.dumps(result,indent=2),encoding='utf-8');print(json.dumps({k:v for k,v in result.items() if k!='rows'}))
