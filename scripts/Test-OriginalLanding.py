"""Complete the original Bacta section using normal Android picker and touch controls.
Read-only profiling poses steer the test; no position, health or objective injection.
"""
import argparse, hashlib, importlib.util, json, math, re, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);p.add_argument('--software',action='store_true');args=p.parse_args()
OUT=args.output;OUT.mkdir(parents=True,exist_ok=True)
spec=importlib.util.spec_from_file_location('play_ui',ROOT/'scripts/Test-PlayableEmulator.py');ui=importlib.util.module_from_spec(spec);spec.loader.exec_module(ui);ui.OUT=OUT
scene=ROOT/'analysis/mission1/geo_01a-landing.rcplay'
assets=json.loads((ROOT/'analysis/mission1/mission-receipt.json').read_text())
ui.bootstrap(profile=True,software=args.software,scene=scene)
tree=ui.ui('paused');ui.find(tree,'Pausiert');control=next(n for n in tree.iter('node') if n.attrib.get('content-desc','').startswith('Links Joystick'))
left,top,right,bottom=ui.bounds(control);w=right-left;h=bottom-top;r=min(w,h)*.145
def telemetry():
 raw=ui.adb('logcat','-d','-s','RCPerf:I','*:S')
 frames=[json.loads(line[line.index('{'):]) for line in raw.splitlines() if '{' in line and '"type":"frame"' in line]
 assert frames
 return raw,frames[-1]
def pose():return telemetry()[1]['pose']
def turn_to(target):
 for _ in range(8):
  x,y,z,yaw,pitch=pose();wanted=math.atan2(target[1]-y,target[0]-x)
  delta=(yaw-wanted+math.pi)%math.tau-math.pi
  if abs(delta)<.025 and abs(pitch)<.025:return
  delta=max(-.7,min(.7,delta))
  ui.swipe(left+w*.66,top+h*.35,left+w*.66+delta*w/3,top+h*.35-pitch*h/2,120)
  time.sleep(.08)
 raise AssertionError('Touch look failed to align')
before=ui.adb('exec-out','screencap','-p',binary=True);time.sleep(.3);assert before==ui.adb('exec-out','screencap','-p',binary=True)
ui.tap_node(ui.find(tree,'FORTSETZEN'))
visited=[]
for target in assets['route']:
 for attempt in range(40):
  current=pose();d=math.dist(current[:3],target)
  if d<45:break
  turn_to(target)
  duration=max(100,min(600,int(d/300*650)))
  ui.swipe(left+r*1.6,bottom-r*2.7,left+r*1.6,bottom-r*2.7,duration)
  time.sleep(.12)
 else:raise AssertionError(('Touch walking stuck',target,pose()))
 visited.append(dict(target=target,pose=pose(),attempts=attempt))
 ui.capture('waypoint-'+str(len(visited)))
turn_to(assets['station'])
use=(right-r*3.1,bottom-r*3.5)
ui.swipe(*use,*use,1000)
time.sleep(.1)
partial=ui.logs('partial-heal');assert 'hp=25' in partial and 'hp=100' not in partial
partial_health=int(re.findall(r'state hp=(\d+)',partial)[-1]);assert 25<partial_health<100
ui.capture('partial-heal')
time.sleep(.4)
_,released=telemetry();assert released['phase']==0
assert int(re.findall(r'state hp=(\d+)',ui.logs('released'))[-1])==partial_health
ui.swipe(*use,*use,4000)
deadline=time.monotonic()+5
while time.monotonic()<deadline:
 log=ui.logs('complete')
 if 'hp=100' in log and 'phase=1' in log:break
 time.sleep(.1)
else:raise AssertionError('Bacta hold did not complete original objective')
tree=ui.ui('complete');assert any(n.attrib.get('text','').startswith('Originalziel abgeschlossen') for n in tree.iter('node'))
ui.capture('complete')
audio=ui.adb('logcat','-d','-s','RCAudio:I','*:S');(OUT/'audio.log').write_text(audio,encoding='utf-8')
assert 'bank sounds=3' in audio and 'sound=2' in audio and 'play failed' not in audio and 'sample load failed' not in audio
ui.adb('logcat','-c');ui.tap_node(ui.find(tree,'NEUSTART'));time.sleep(.2)
ui.tap_node(ui.find(ui.ui(),'PAUSE'))
reset=ui.logs('restart');assert 'hp=25 ammo=20 alive=0 phase=0 ticks=0' in reset
ui.capture('restart')
crash=ui.adb('logcat','-d','-b','crash');assert 'FATAL EXCEPTION' not in crash and 'Fatal signal' not in crash
raw,frame=telemetry();(OUT/'performance.log').write_text(raw,encoding='utf-8')
assert frame['backend']==('cpu' if args.software else 'gpu')
result=dict(passed=True,scene_sha256=hashlib.sha256(scene.read_bytes()).hexdigest(),backend=frame['backend'],normal_picker=True,physical_touch_walk=True,visited=visited,hold_and_release=True,original_objective_complete=True,three_advisor_samples=True,pause_exact=True,restart=True,no_crash=True,scope='API35 x86_64 emulator; ordinary touch controls, read-only pose telemetry. Original Bacta objective only; cinematic and later mission are outside this section. Physical ARM64 pending.')
(OUT/'validation.json').write_text(json.dumps(result,indent=2),encoding='utf-8');print(json.dumps({k:v for k,v in result.items() if k!='visited'}))
