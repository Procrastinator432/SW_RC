"""Resume genuine completed-playthrough checkpoints in the Android APK, without replaying inputs."""
import hashlib
import copy
import base64
import re
import importlib.util
import json
import struct
import subprocess
import time
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/mission30-checkpoint-recovery/delivery/android-mission'
OUT.mkdir(parents=True,exist_ok=True)
spec=importlib.util.spec_from_file_location('playable',ROOT/'scripts/Test-PlayableEmulator.py')
ui=importlib.util.module_from_spec(spec);spec.loader.exec_module(ui);ui.OUT=OUT

def adb(*args,input=None):
 for attempt in range(6):
  result=subprocess.run([str(ui.ADB),'-s','emulator-5554',*map(str,args)],input=input,capture_output=True,timeout=120)
  if not result.returncode:return result.stdout
  if attempt==5:raise RuntimeError(result.stderr.decode(errors='replace'))
  subprocess.run([str(ui.ADB),'reconnect','offline'],capture_output=True);time.sleep(1)

def read(name):return adb('exec-out','run-as','org.rcport.diagnostic','cat','files/checkpoints/'+name)
def put(name,data):
 assert re.fullmatch(r'[a-z0-9./-]+',name) and '..' not in name
 # Windows adb's stdin may treat a binary 0x1a as EOF. Transfer ASCII and
 # decode on the device, then verify the complete fixture before launching.
 adb('shell','-T','run-as','org.rcport.diagnostic','tee','files/checkpoints/upload.b64',input=base64.b64encode(data))
 adb('shell',f"run-as org.rcport.diagnostic sh -c 'base64 -d files/checkpoints/upload.b64 > files/checkpoints/{name}'")
 assert read(name)==data,'ADB checkpoint fixture transfer was incomplete'
def tree():
 for attempt in range(6):
  try:return ui.ui()
  except Exception:
   if attempt==5:raise
   adb('reconnect','offline');time.sleep(1)

def wait_text(text):
 end=time.monotonic()+45
 while time.monotonic()<end:
  for node in tree().iter('node'):
   if node.get('text','').casefold()==text.casefold():return node
  time.sleep(.3)
 raise AssertionError('Missing UI: '+text)

def tap(text):ui.tap_node(wait_text(text))
def launch():
 adb('shell','am','force-stop','org.rcport.diagnostic')
 adb('shell','am','start','-W','-n','org.rcport.diagnostic/.MainActivity','--ez','rcProfile','true')
 tap('Fortsetzen')
 end=time.monotonic()+45
 while time.monotonic()<end:
  nodes=list(tree().iter('node'))
  if any('GEO_01C' in n.get('text','') for n in nodes) and any(n.get('text','').casefold()=='fortsetzen' for n in nodes):return
  time.sleep(.3)
 raise AssertionError('Mission checkpoint did not finish loading paused')

def utf(s):
 b=s.encode('ascii');return struct.pack('>H',len(b))+b

def checkpoint(slot):
 assert slot[:8]==b'RCSLOT01'
 body=slot[44:];assert len(body)==struct.unpack_from('>I',slot,8)[0] and hashlib.sha256(body).digest()==slot[12:44]
 at=0
 for gap in [8,0]:
  length=struct.unpack_from('>H',body,at)[0];at+=2+length+gap
 length=struct.unpack_from('>I',body,at)[0];state=body[at+4:at+4+length]
 assert state[:8]==b'RCSAVE01'
 return json.loads(state[20:])

# These snapshots were captured at real ticks of the full A -> B -> C run.
# The scene is already cached by Test-CheckpointsAndroid.py's visible C load.
scene=ROOT/'analysis/mission28-first-mission-complete/delivery/geo_01c.rcplay'
name=hashlib.sha256(scene.read_bytes()).hexdigest()+'.rcplay'
assert hashlib.sha256(read('scenes/'+name)).hexdigest()+'.rcplay'==name
results=[]
for tick in [3400,7300,10000]:
 adb('shell','am','force-stop','org.rcport.diagnostic')
 source=ROOT/f'analysis/mission29-checkpoints/delivery/runtime/C-{tick}.rcsave'
 state=source.read_bytes();expected=json.loads(state[20:])
 body=utf(name)+struct.pack('>q',int(time.time()*1000))+utf(f'GEO_01C original replay tick {tick}')+struct.pack('>I',len(state))+state
 put('resume.save',b'RCSLOT01'+struct.pack('>I',len(body))+hashlib.sha256(body).digest()+body)
 put('last-scene',name.encode('ascii'))
 launch();tap('Checkpoint speichern');time.sleep(.7)
 restored=checkpoint(read('resume.save'))
 # The production UI intentionally discards historical sound events on restore.
 # Native replay snapshots retain these because their audio bank wasn't drained.
 gameplay=copy.deepcopy(expected['state'])
 for module in ['map','combat','mission']:
  if gameplay.get(module) is not None:gameplay[module]['events']=[]
 assert restored['state']==gameplay,f'C-{tick}: restored gameplay state differs'
 assert restored['initial']==expected['initial'],f'C-{tick}: entry state differs'
 ui.capture(f'C-{tick}-restored')
 # Real time in the paused UI must not advance scripts, combat or clocks.
 time.sleep(1);tap('Checkpoint speichern');time.sleep(.5)
 assert checkpoint(read('resume.save'))['state']==restored['state']
 # Continue ordinary gameplay briefly, background-save, kill, and cold-restore.
 tap('Fortsetzen');time.sleep(1)
 adb('shell','input','keyevent','3');time.sleep(1)
 background=checkpoint(read('resume.save'))
 assert background['state']['statistics']['ticks']>tick
 launch();tap('Checkpoint speichern');time.sleep(.7)
 assert checkpoint(read('resume.save'))['state']==background['state']
 results.append(dict(source=str(source.relative_to(ROOT)),tick=tick,exact_state_after_stale_audio_drain=True,exact_entry=True,paused_clock_stable=True,background_ticks=background['state']['statistics']['ticks'],actual_kill_and_continue_exact=True))
 print(f'C-{tick}: restored, continued and cold-restored exactly',flush=True)
(OUT/'validation.json').write_text(json.dumps(dict(passed=True,actual_mission_checkpoints=results,platform='Android x86_64 APK',snapshot_abi='rcport-state-0.46.0-1',limitations=['Physical ARM64 not exercised','Source checkpoints are genuine replay states supplied through the private test store, not manually reached with touch controls']),indent=2))
print('Three genuine mid-mission C checkpoints: exact paused state, ordinary continuation, background-save and cold process recovery passed')
