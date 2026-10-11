"""Complete original Bacta, crash-site and grenade pickup with normal Android picker and touch controls.
Read-only profiling poses steer the test; no position, health or objective injection.
"""
import argparse, hashlib, importlib.util, json, math, re, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);p.add_argument('--software',action='store_true');p.add_argument('--scene',type=Path);args=p.parse_args()
OUT=args.output;OUT.mkdir(parents=True,exist_ok=True)
spec=importlib.util.spec_from_file_location('play_ui',ROOT/'scripts/Test-PlayableEmulator.py');ui=importlib.util.module_from_spec(spec);spec.loader.exec_module(ui);ui.OUT=OUT
scene=args.scene or ROOT/'analysis/mission3-playable/geo_01a-grenades.rcplay'
raw_scene=scene.read_bytes();mission=json.loads(raw_scene[12:12+int.from_bytes(raw_scene[8:12],'little')]);crash=mission['crash']
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
 for _ in range(16):
  x,y,z,yaw,pitch=pose();wanted=math.atan2(target[1]-y,target[0]-x)
  delta=(yaw-wanted+math.pi)%math.tau-math.pi
  if abs(delta)<.025 and abs(pitch)<.025:return
  delta=max(-.7,min(.7,delta))
  ui.swipe(left+w*.66,top+h*.35,left+w*.66+delta*w/3,top+h*.35+pitch*h/2,120)
  time.sleep(.08)
 raise AssertionError(('Touch look failed to align', target, pose()))
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
deadline=time.monotonic()+8
while time.monotonic()<deadline:
 log=ui.logs('healed')
 if 'hp=100' in log:break
 time.sleep(.1)
else:raise AssertionError('Bacta hold failed')
assert telemetry()[1]['phase']==0
# Let the original advisor and barrier timers run, then advance through the contact triggers.
time.sleep(6.2)
crash_visited=[]
vulnerable=False
def fight():
 for _ in range(12):
  raw,f=telemetry()
  assert f['phase']!=2, ('Defeated', f)
  if f['phase']==1:return
  living=[a for a in f.get('actors',[]) if a['health']>0 and a['visible']]
  if not living:return
  x,y,z,yaw,pitch=f['pose']
  a=min(living,key=lambda a:math.dist(a['position'],[x,y,z]))
  tx,ty,tz=a['position'];wanted=math.atan2(ty-y,tx-x)
  aim=math.atan2(tz+15-(z+84*.65),math.hypot(tx-x,ty-y))
  delta=(yaw-wanted+math.pi)%math.tau-math.pi
  while abs(delta)>.7:
   step=max(-.7,min(.7,delta))
   ui.swipe(left+w*.66,top+h*.35,left+w*.66+step*w/3,top+h*.35,100)
   delta-=step
  ui.swipe(left+w*.66,top+h*.35,left+w*.66+delta*w/3,top+h*.35-(aim-pitch)*h/2,100)
  fx,fy=right-r*1.3,bottom-r*1.7
  ui.swipe(fx,fy,fx,fy,450)
  ui.logs('combat')
  if telemetry()[1]['shots']>=19:ui.tap(right-r*1.3,bottom-r*3.5);time.sleep(1.8)
for target in crash['route']:
 for attempt in range(40):
  f=telemetry()[1]
  assert f['phase']!=2, ('Defeated on route', f)
  if f['phase']==1:break
  # The original dialog contact switches the five actors out of protection.
  # The read-only player pose is compared with the original contact cylinder.
  trigger=crash['flow']['vulnerability'];px,py,pz=f['pose'][:3]
  vulnerable |= math.hypot(px-trigger['position'][0],py-trigger['position'][1]) < trigger['radius']+34 and abs(pz-trigger['position'][2]) < trigger['height']+84
  if vulnerable:
   fight()
   if telemetry()[1]['phase']==1:break
  current=pose();d=math.dist(current[:3],target)
  if d<65:break
  turn_to(target)
  duration=max(100,min(500,int(d/300*650)))
  ui.swipe(left+r*1.6,bottom-r*2.7,left+r*1.6,bottom-r*2.7,duration)
  time.sleep(.06)
 else:raise AssertionError(('Crash route stuck',target,pose()))
 crash_visited.append(dict(target=target,pose=pose(),attempts=attempt))
 if telemetry()[1]['phase']==1:break
fight()
raw,crash_complete=telemetry();assert crash_complete['phase']==0,crash_complete
assert all(a['health']==0 for a in crash_complete['actors'])
ui.capture('crash-secured')
grenade_visited=[]
for index,target in enumerate(mission['grenades']['route']):
 if index==mission['grenades']['barrier_waypoint']+1:time.sleep(15)
 for attempt in range(45):
  current=pose();d=math.dist(current[:3],target)
  if d<55:break
  turn_to(target)
  duration=max(100,min(500,int(d/300*650)))
  ui.swipe(left+r*1.6,bottom-r*2.7,left+r*1.6,bottom-r*2.7,duration)
  time.sleep(.06)
 else:raise AssertionError(('Grenade route stuck',index,target,pose()))
 grenade_visited.append(dict(target=target,pose=pose(),attempts=attempt))
 if index==mission['grenades']['barrier_waypoint']:ui.capture('connecting-barrier')
ui.capture('pickup')
time.sleep(6)
raw,complete=telemetry();assert complete['phase']==1,complete
assert len(complete['actors'])==5 and all(a['health']==0 for a in complete['actors'])
ui.logs('complete');tree=ui.ui('complete')
assert any(n.attrib.get('text','').startswith('Originalziele abgeschlossen') for n in tree.iter('node'))
ui.capture('complete')
audio=ui.adb('logcat','-d','-s','RCAudio:I','*:S');(OUT/'audio.log').write_text(audio,encoding='utf-8')
assert 'bank sounds='+str(len(mission['sounds'])) in audio and all(re.search(r'sound='+str(i)+r'\b',audio) for i in range(13))
assert 'play failed' not in audio and 'sample load failed' not in audio
weapon_events=len(re.findall(r'sound='+str(mission.get('weapon_fire_sound',-1))+r'\b',audio))
if 'weapon_fire_sound' in mission:assert weapon_events==complete['shots'] and weapon_events>=15,(weapon_events,complete)
ui.adb('logcat','-c');ui.tap_node(ui.find(tree,'NEUSTART'));time.sleep(.2)
ui.tap_node(ui.find(ui.ui(),'PAUSE'))
reset=ui.logs('restart');assert 'hp=25 ammo=20 alive=0 phase=0 ticks=0' in reset
ui.capture('restart')
crashlog=ui.adb('logcat','-d','-b','crash');assert 'FATAL EXCEPTION' not in crashlog and 'Fatal signal' not in crashlog
raw,frame=telemetry();(OUT/'performance.log').write_text(raw,encoding='utf-8')
assert frame['backend']==('cpu' if args.software else 'gpu')
result=dict(passed=True,scene_sha256=hashlib.sha256(scene.read_bytes()).hexdigest(),backend=frame['backend'],normal_picker=True,physical_touch_walk=True,visited=visited,crash_visited=crash_visited,hold_and_release=True,three_original_objectives_complete=True,grenades=5,grenade_visited=grenade_visited,five_factory_actors_and_kills=True,all_thirteen_sound_roles=True,weapon_fire_samples=weapon_events,weapon_samples_match_shots=(weapon_events==complete['shots']),pause_exact=True,restart=True,no_crash=True,scope='API35 x86_64 emulator; ordinary touch controls, read-only pose and actor telemetry. Selected original objectives 0, 1 and 2; bounded wall resting pose and selected connecting scripts; provisional combat host. Physical ARM64 pending.')
(OUT/'validation.json').write_text(json.dumps(result,indent=2),encoding='utf-8');print(json.dumps({k:v for k,v in result.items() if k not in ['visited','crash_visited']}))
