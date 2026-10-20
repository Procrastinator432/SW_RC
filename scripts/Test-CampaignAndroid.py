"""Real package picker/import, compact touch controls and automatic campaign travel.

Transition fixtures are genuine map-end snapshots from the normal full replay.
They exercise the APK travel path without another complete manual touch run.
"""
import argparse,base64,hashlib,json,re,struct,subprocess,time,zipfile
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser()
parser.add_argument('--apk',type=Path,default=ROOT/'build/artifacts/republic-commando-0.49.0-campaign.apk')
parser.add_argument('--package',type=Path,default=ROOT/'build/artifacts/mission1-scenes.rcpack.zip')
parser.add_argument('--checkpoints',type=Path,default=ROOT/'build/scenes/runtime-validation')
parser.add_argument('--output',type=Path,default=ROOT/'build/campaign/android')
args=parser.parse_args();OUT=args.output;OUT.mkdir(parents=True,exist_ok=True)
ADB=ROOT/'tools/android-sdk/platform-tools/adb.exe';APP='org.rcport.diagnostic'

def raw(*params,input=None):
 for attempt in range(6):
  result=subprocess.run([str(ADB),'-s','emulator-5554',*map(str,params)],input=input,capture_output=True,timeout=120)
  if not result.returncode:return result.stdout
  if attempt==5:raise RuntimeError(result.stderr.decode(errors='replace'))
  subprocess.run([str(ADB),'reconnect','offline'],capture_output=True);time.sleep(1)
def adb(*params):return raw(*params).decode('utf-8',errors='replace')
def tree():
 for attempt in range(5):
  try:
   adb('shell','uiautomator','dump','/data/local/tmp/rc-campaign-ui.xml')
   return ET.fromstring(adb('shell','cat','/data/local/tmp/rc-campaign-ui.xml'))
  except Exception:
   if attempt==4:raise
   time.sleep(.5)
def bounds(node):return list(map(int,re.findall(r'\d+',node.get('bounds',''))))
def find(text):
 for node in tree().iter('node'):
  if node.get('text','').casefold()==text.casefold() and len(bounds(node))==4 and bounds(node)[3]>bounds(node)[1]:return node
 raise LookupError(text)
def has(text):return any(text.casefold() in n.get('text','').casefold() for n in tree().iter('node'))
def wait(predicate,seconds=60):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  if predicate():return
  time.sleep(.4)
 raise AssertionError('Expected campaign state did not appear')
def tap_xy(x,y):adb('shell','input','tap',round(x),round(y))
def tap(text):
 n=find(text);x1,y1,x2,y2=bounds(n);tap_xy((x1+x2)/2,(y1+y2)/2)
def swipe(x,y,x2,y2,ms=400):adb('shell','input','swipe',round(x),round(y),round(x2),round(y2),ms)
def reveal(text):
 for _ in range(8):
  try:find(text);return
  except LookupError:
   scroll=next(n for n in tree().iter('node') if n.get('class')=='android.widget.ScrollView');x1,y1,x2,y2=bounds(scroll)
   swipe((x1+x2)/2,y2-15,(x1+x2)/2,y1+15,250)
 raise LookupError(text)
def capture(name):
 (OUT/f'{name}.png').write_bytes(raw('exec-out','screencap','-p'))
 (OUT/f'{name}.xml').write_text(ET.tostring(tree(),encoding='unicode'))
def file(path):return raw('exec-out','run-as',APP,'cat',path)
def exists(path):
 assert re.fullmatch(r'[a-z0-9_./-]+',path) and '..' not in path
 return adb('shell',f"run-as {APP} sh -c 'if [ -f {path} ]; then echo yes; fi'").strip()=='yes'
def put(path,data):
 assert re.fullmatch(r'[a-z0-9_./-]+',path) and '..' not in path
 raw('shell','-T','run-as',APP,'tee','files/campaign-test-upload.b64',input=base64.b64encode(data))
 adb('shell',f"run-as {APP} sh -c 'base64 -d files/campaign-test-upload.b64 > {path}'")
 assert file(path)==data
def slot():
 b=file('files/checkpoints/resume.save');assert b[:8]==b'RCSLOT01';body=b[44:]
 assert len(body)==struct.unpack_from('>I',b,8)[0] and hashlib.sha256(body).digest()==b[12:44]
 at=0
 for gap in [8,0]:at+=2+struct.unpack_from('>H',body,at)[0]+gap
 size=struct.unpack_from('>I',body,at)[0];return json.loads(body[at+24:at+4+size])
def fixture(name,scene_hash):
 state=(args.checkpoints/name).read_bytes()
 def utf(s):b=s.encode('ascii');return struct.pack('>H',len(b))+b
 body=utf(scene_hash+'.rcplay')+struct.pack('>q',int(time.time()*1000))+utf('GEO_'+('03A' if name.startswith('factory') else '01'+name[0])+' fixture')+struct.pack('>I',len(state))+state
 put('files/checkpoints/resume.save',b'RCSLOT01'+struct.pack('>I',len(body))+hashlib.sha256(body).digest()+body)
 return json.loads(state[20:])
def launch():
 adb('shell','am','force-stop',APP);adb('shell','am','start','-W','-a','android.intent.action.MAIN','-c','android.intent.category.LAUNCHER','-f','0x10200000','-n',APP+'/.MainActivity');wait(lambda:has('Neue Mission starten'))
def choose(name):
 wait(lambda:any(n.get('package')=='com.google.android.documentsui' for n in tree().iter('node')))
 for _ in range(18):
  try:tap(name);return
  except LookupError:
   # Downloads is available in the roots sidebar on this landscape test device.
   try:tap('Downloads')
   except LookupError:pass
   swipe(700,440,700,160,180)
 capture('picker-failure');raise AssertionError('Package missing from DocumentsUI')
def pause_save():
 tap('Menü');reveal('Checkpoint speichern');tap('Checkpoint speichern');time.sleep(.5);return slot()
def controls():
 node=next(n for n in tree().iter('node') if n.get('content-desc','').startswith('Links Joystick'))
 log=adb('logcat','-d','-s','RCTouch:I','*:S');layout=json.loads(re.findall(r'RCTouch\s*:\s*(\{.*\})',log)[-1]);return bounds(node),layout
def angle(a,b):
 import math
 return math.atan2(math.sin(a-b),math.cos(a-b))

size_before=adb('shell','wm','size');density_before=adb('shell','wm','density')
try:
 adb('shell','wm','size','960x540');adb('shell','wm','density','240')
 adb('install','-r',args.apk);adb('logcat','-c')
 adb('push',args.package,'/sdcard/Download/mission1-scenes.rcpack.zip')
 launch();capture('small-start-menu')
 previous=file('files/checkpoints/resume.save') if exists('files/checkpoints/resume.save') else None
 tap('Szenenpaket importieren');choose('mission1-scenes.rcpack.zip');wait(lambda:has('Szenenpaket bereit'),seconds=150)
 assert (file('files/checkpoints/resume.save')==previous if previous is not None else not exists('files/checkpoints/resume.save')),'Import modified checkpoint'
 print('Package import and checkpoint preservation passed',flush=True)
 capture('small-import-complete')
 with zipfile.ZipFile(args.package) as z:
  props=dict(line.split('=',1) for line in z.read('campaign.properties').decode().splitlines() if '=' in line)
 # The package survives a cold start and starting A needs no picker.
 launch();assert find('Neue Mission starten').get('enabled')=='true'
 tap('Neue Mission starten');wait(lambda:has('Neue Mission starten?') or has('GEO_01A'))
 if has('Neue Mission starten?'):tap('Starten')
 wait(lambda:has('GEO_01A'),seconds=90);capture('small-first-map')
 # Android may recreate the original new-game intent after killing a background
 # process. It must restore the checkpoint, not start A again from tick zero.
 adb('shell','input','keyevent','3');time.sleep(1.5);background=slot()
 adb('shell','am','kill',APP);wait(lambda:not adb('shell',f'pidof {APP} || true').strip())
 adb('shell','am','start','-W','-a','android.intent.action.MAIN','-c','android.intent.category.LAUNCHER','-f','0x10200000','-n',APP+'/.MainActivity')
 wait(lambda:has('GEO_01A') and has('Fortsetzen') and has('Menü'),seconds=60)
 restored=pause_save();assert restored['state']==background['state'],'Recreated new-game activity lost its saved progress'
 capture('background-process-recreation');print('Original new-game activity restored exactly after Android background kill',flush=True)
 tap('Menü');reveal('Hauptmenü');tap('Hauptmenü');wait(lambda:has('Neue Mission starten'))
 # A rejected incomplete package must preserve both installed index and saves.
 index_before=file('files/campaign/package.properties');save_before=file('files/checkpoints/resume.save')
 bad=OUT/'incomplete.rcpack.zip'
 with zipfile.ZipFile(bad,'w') as z:z.writestr('campaign.properties','format=RCCAMPAIGN1\n')
 adb('push',bad,'/sdcard/Download/incomplete.rcpack.zip');tap('Szenenpaket importieren');choose('incomplete.rcpack.zip');wait(lambda:has('Import fehlgeschlagen'))
 assert file('files/campaign/package.properties')==index_before and file('files/checkpoints/resume.save')==save_before
 capture('rejected-package');tap('OK')
 # Set and retain high sensitivity, then use an actual grounded factory snapshot.
 reveal('Steuerung');tap('Steuerung');seek=next(n for n in tree().iter('node') if n.get('class')=='android.widget.SeekBar');x1,y1,x2,y2=bounds(seek)
 swipe(x1+12,(y1+y2)/2,x2-4,(y1+y2)/2);tap('Speichern');time.sleep(.5)
 settings=ET.fromstring(file('shared_prefs/controls.xml'));gain=float(next(n.get('value') for n in settings if n.get('name')=='sensitivity'));assert gain>=1.9
 launch();fixture('factory-substep.rcsave',props['GEO_03A.sha256']);tap('Fortsetzen');wait(lambda:has('GEO_03A') and has('Fortsetzen'))
 capture('small-touch-paused');box,layout=controls();density=layout['density'];assert layout['width']/density==640 and abs(layout['height']/density-312)<1
 for role,x,y,r in layout['keys']:assert r*2/density>=48 and 0<=x-r and x+r<=layout['width'] and 0<=y-r and y+r<=layout['height']
 baseline=pause_save();tap('Fortsetzen');swipe(box[0]+layout['width']*.52,box[1]+layout['height']*.30,box[0]+layout['width']*.52+120,box[1]+layout['height']*.30,400)
 fast=pause_save();delta_fast=abs(angle(fast['state']['yaw'],baseline['state']['yaw']));assert abs(delta_fast-120/density*3/640*gain)<.04
 tap('Menü');reveal('Steuerung');tap('Steuerung');tap('Standard');tap('Fortsetzen')
 swipe(box[0]+layout['width']*.52,box[1]+layout['height']*.30,box[0]+layout['width']*.52+120,box[1]+layout['height']*.30,400)
 normal=pause_save();delta_normal=abs(angle(normal['state']['yaw'],fast['state']['yaw']));assert abs(delta_normal-.375)<.04 and 1.8<delta_fast/delta_normal<2.2
 tap('Fortsetzen');swipe(box[0]+layout['width']*.52,box[1]+layout['height']*.30,box[0]+layout['width']*.52,box[1]+layout['height']*.30+60,400)
 vertical=pause_save();assert abs(abs(vertical['state']['pitch']-normal['state']['pitch'])-.1875)<.04
 print(f'Sensitivity and small-screen geometry passed ({delta_fast:.3f}/{delta_normal:.3f} radians)',flush=True)
 tap('Fortsetzen');jx,jy,jr=layout['joystick'];swipe(box[0]+jx,box[1]+jy,box[0]+jx,box[1]+jy-jr*.65,800)
 for role in [2,3]:
  key=next(k for k in layout['keys'] if k[0]==role);tap_xy(box[0]+key[1],box[1]+key[2]);time.sleep(.3)
 moved=pause_save();assert moved['state']['statistics']['shots']>normal['state']['statistics']['shots'] and moved['state']['statistics']['jumps']>normal['state']['statistics']['jumps']
 assert moved['state']['pawn']['body']['position']!=normal['state']['pawn']['body']['position'];capture('small-touch-after-input')
 energy_checked=moved['state'].get('dc15') is not None
 if energy_checked:
  assert moved['state']['reserve']==0 and moved['state']['reload_remaining']==0
  before_reload=moved['state']['statistics']['reloads']
  assert any('Energieanzeige' in (n.get('content-desc') or '') for n in tree().iter('node'))
  tap('Fortsetzen');key=next(k for k in layout['keys'] if k[0]==4);tap_xy(box[0]+key[1],box[1]+key[2])
  energy_save=pause_save();assert energy_save['state']['statistics']['reloads']==before_reload and energy_save['state']['reload_remaining']==0
  capture('energy-indicator-reload-inert')
 # Genuine won snapshots select B/C/factory automatically, with actual native carry.
 transitions=[]
 for source,label,target in [('GEO_01A','A','GEO_01B'),('GEO_01B','B','GEO_01C'),('GEO_01C','C','GEO_03A')]:
  completed=[]
  for path in args.checkpoints.glob(label+'-*.rcsave'):
   snapshot=json.loads(path.read_bytes()[20:])
   if snapshot['state']['phase']=='Won':completed.append(path)
  assert completed, f'No completed {label} checkpoint in {args.checkpoints}'
  name=max(completed,key=lambda p:int(p.stem.split('-')[1])).name
  launch();old=fixture(name,props[source+'.sha256']);tap('Fortsetzen');wait(lambda:has(target),seconds=90)
  new=slot();assert new['state']['map']['destination'].startswith(target)
  for field in ['health','ammo','reserve']:assert new['initial'][field]==old['state'][field],(target,field)
  if old['state'].get('dc15') is not None:
   assert new['initial']['dc15']==old['state']['dc15'],target
   assert new['initial']['fire_remaining']==old['state']['fire_remaining'],target
  assert all(n.get('package')!='com.google.android.documentsui' for n in tree().iter('node'))
  capture('automatic-'+target.lower());transitions.append(dict(source=source,destination=target,carried_entry_exact=True,no_file_picker=True))
  print(f'Automatic {source} -> {target} passed',flush=True)
 log=adb('logcat','-d','-s','RCPlay:I','RCCheckpoint:I','RCTouch:I','AndroidRuntime:E','*:S');(OUT/'app.log').write_text(log);assert 'FATAL EXCEPTION' not in log
 (OUT/'validation.json').write_text(json.dumps(dict(passed=True,apk_sha256=hashlib.sha256(args.apk.read_bytes()).hexdigest(),package_sha256=hashlib.sha256(args.package.read_bytes()).hexdigest(),actual_package_picker_and_native_import=True,import_preserves_checkpoint=True,invalid_import_preserves_package_and_save=True,cold_start_keeps_package=True,new_game_without_picker=True,android_background_kill_recreates_original_intent_exactly=True,logical_display=[640,360],viewport_dp=[640,312],all_touch_targets_min_48dp=True,touch_movement_fire_jump=True,sensitivity_saved_and_density_independent=True,both_look_axes_verified=True,dc15_energy_indicator_reload_inert_and_state_carry=energy_checked,high_sensitivity_yaw_delta=delta_fast,normal_sensitivity_yaw_delta=delta_normal,automatic_transitions=transitions,limits=['Android x86_64 emulator; physical small-screen comfort remains for device testing','Map-end and factory checkpoints are genuine replay states inserted into private test storage, not a full manual touch playthrough']),indent=2))
 print('Package import, compact small-screen controls, saved sensitivity and all three automatic map transitions passed')
except Exception:
 import traceback
 (OUT/'failure.txt').write_text(traceback.format_exc())
 try:(OUT/'failure.log').write_text(adb('logcat','-d','-s','RCPlay:I','RCCheckpoint:I','RCAudio:I','AndroidRuntime:E','*:S'))
 except Exception:pass
 try:capture('failure')
 except Exception:pass
 raise
finally:
 old_size=re.search(r'Override size: (\d+x\d+)',size_before);old_density=re.search(r'Override density: (\d+)',density_before)
 adb('shell','wm','size',old_size[1] if old_size else 'reset');adb('shell','wm','density',old_density[1] if old_density else 'reset')
