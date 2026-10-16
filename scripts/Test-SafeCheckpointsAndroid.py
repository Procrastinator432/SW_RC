"""Safe fallback, explicit manual save, process loss and repeated quick-death recovery.

Transition fixtures are genuine map-end snapshots from the normal full replay.
They exercise the APK travel path without another complete manual touch run.
"""
import argparse,base64,hashlib,json,re,struct,subprocess,time,zipfile
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser()
parser.add_argument('--apk',type=Path,default=ROOT/'build/phone-blockers/android-build/artifacts/republic-commando-0.50.0-phone-fixes.apk')
parser.add_argument('--package',type=Path,default=ROOT/'build/phone-blockers/package-build/artifacts/mission1-scenes.rcpack.zip')
parser.add_argument('--checkpoints',type=Path,default=ROOT/'build/phone-blockers/runtime-validation')
parser.add_argument('--output',type=Path,default=ROOT/'build/phone-blockers/checkpoint-android')
parser.add_argument('--entry-failure-only',action='store_true')
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


def entry_failure():
 launch();previous=file('files/checkpoints/entry.save')
 path='files/checkpoints/entry.save.tmp'
 adb('shell','run-as',APP,'mkdir',path)
 raw('shell','-T','run-as',APP,'tee',path+'/test-obstruction',input=b'entry write failure fixture')
 try:
  tap('Neue Mission starten');wait(lambda:has('Neue Mission starten?'));tap('Starten')
  wait(lambda:has('GEO_01A') and has('Menü') and not has('Spielstand wiederherstellen'),seconds=90)
  assert file('files/checkpoints/entry.save')==previous,'Failed entry write replaced committed entry'
  tap('Menü');wait(lambda:has('Checkpoint speichern'));tap('Schließen');capture('entry-write-failure-playable')
  log=adb('logcat','-d','-s','RCCheckpoint:I','AndroidRuntime:E','*:S');assert 'entry save failed' in log and 'FATAL EXCEPTION' not in log
  (OUT/'entry-failure.log').write_text(log)
 finally:
  adb('shell','run-as',APP,'rm',path+'/test-obstruction');adb('shell','run-as',APP,'rmdir',path)
 return True

if args.entry_failure_only:
 adb('install','-r',args.apk);adb('logcat','-c')
 report=dict(passed=entry_failure(),apk_sha256=hashlib.sha256(args.apk.read_bytes()).hexdigest(),entry_write_failure_preserves_entry_and_scene_usable=True)
 (OUT/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps(report),flush=True);raise SystemExit(0)

# The companion campaign test performs the real ZIP/native import first.
adb('install','-r',args.apk);adb('logcat','-c');launch()
with zipfile.ZipFile(args.package) as z:
 props=dict(line.split('=',1) for line in z.read('campaign.properties').decode().splitlines() if '=' in line)
 b_scene=z.read('geo_01b.rcplay');n=struct.unpack_from('<I',b_scene,8)[0];b_assets=json.loads(b_scene[12:12+n])
fixture('factory-substep.rcsave',props['GEO_03A.sha256']);tap('Fortsetzen');wait(lambda:has('GEO_03A') and has('Fortsetzen'))
tap('Fortsetzen');time.sleep(7);manual=pause_save()
assert exists('files/checkpoints/safe.save'),'No safe fallback after quiet grounded play'
safe=file('files/checkpoints/safe.save');explicit=file('files/checkpoints/manual.save')
capture('safe-and-manual-created')
tap('Fortsetzen');box,layout=controls();fire=next(k for k in layout['keys'] if k[0]==2);tap_xy(box[0]+fire[1],box[1]+fire[2])
adb('shell','input','keyevent','3');time.sleep(1.2);background=slot()
assert file('files/checkpoints/safe.save')==safe and file('files/checkpoints/manual.save')==explicit
adb('shell','am','kill',APP);wait(lambda:not adb('shell',f'pidof {APP} || true').strip())
adb('shell','am','start','-W','-a','android.intent.action.MAIN','-c','android.intent.category.LAUNCHER','-f','0x10200000','-n',APP+'/.MainActivity')
wait(lambda:has('GEO_03A') and has('Fortsetzen') and has('Menü'));capture('background-restored')
# Do not manually save here: that would intentionally replace the explicit slot.
tap('Menü');reveal('Checkpoint laden');tap('Checkpoint laden');tap('Manuell gespeichert');wait(lambda:has('GEO_03A') and has('Fortsetzen') and not has('Spielstand auswählen'))
current=slot()
for field in ['health','ammo','reserve','pawn','statistics']:assert current['state'][field]==manual['state'][field],field
assert file('files/checkpoints/manual.save')==explicit
# Controlled threat fixture: a genuine B state with 1 HP and one original live
# droid moved next to the player. The ordinary APK combat tick causes defeat.
# No mocked JNI result, forced Lost phase or hand-written death answer is used.
launch();adb('shell','run-as',APP,'rm','-f','files/checkpoints/quick-deaths')
raw_state=(args.checkpoints/'B-10000.rcsave').read_bytes();snapshot=json.loads(raw_state[20:]);st=snapshot['state'];st['health']=1
factory=st['map']['gameplay']['enemy_factories']
i=max((i for i,e in enumerate(st['enemies']) if e['health']>0 and not b_assets['gameplay']['factories'][factory[i]]['warrior']),key=lambda i:st['enemies'][i]['health'])
st['enemies'][i]['position']=st['pawn']['body']['position'].copy();st['enemies'][i]['position'][0]+=20;st['enemies'][i]['cooldown']=0
payload=json.dumps(snapshot,separators=(',',':')).encode();checksum=0xcbf29ce484222325
for value in payload:checksum=((checksum^value)*0x100000001b3)&0xffffffffffffffff
native=b'RCSAVE01'+struct.pack('<IQ',len(payload),checksum)+payload
utf=lambda s:struct.pack('>H',len(s.encode()))+s.encode()
body=utf(props['GEO_01B.sha256']+'.rcplay')+struct.pack('>q',int(time.time()*1000))+utf('GEO_01B controlled threat')+struct.pack('>I',len(native))+native
put('files/checkpoints/resume.save',b'RCSLOT01'+struct.pack('>I',len(body))+hashlib.sha256(body).digest()+body)
tap('Fortsetzen');wait(lambda:has('GEO_01B') and has('Fortsetzen') and has('Menü'));tap('Fortsetzen');wait(lambda:has('Gefallen · Spielstand auswählen'))
assert file('files/checkpoints/quick-deaths').endswith(b':1');assert file('files/checkpoints/safe.save')==safe
capture('first-quick-death');launch();tap('Fortsetzen');wait(lambda:has('GEO_01B') and has('Fortsetzen') and not has('Spielstand auswählen'))
tap('Fortsetzen');wait(lambda:has('Wiederholt kurz nach dem Laden gefallen'))
assert file('files/checkpoints/quick-deaths').endswith(b':2');assert file('files/checkpoints/safe.save')==safe and file('files/checkpoints/manual.save')==explicit
capture('repeated-death-fallback');tap('Sicherer Checkpoint');wait(lambda:has('GEO_03A') and has('Fortsetzen') and not has('Wiederholt'))
# A damaged resume is recoverable through the same ordinary safe-slot chooser.
launch();damaged=bytearray(file('files/checkpoints/resume.save'));damaged[-1]^=1;put('files/checkpoints/resume.save',damaged)
tap('Fortsetzen');wait(lambda:has('Spielstand wiederherstellen'));tap('Sichere und manuelle Spielstände auswählen');tap('Sicherer Checkpoint')
wait(lambda:has('GEO_03A') and has('Fortsetzen') and has('Menü'));capture('corrupt-resume-safe-recovery')
log=adb('logcat','-d','-s','RCPlay:I','RCCheckpoint:I','RCAudio:I','AndroidRuntime:E','*:S');(OUT/'app.log').write_text(log);assert 'FATAL EXCEPTION' not in log
report=dict(passed=True,apk_sha256=hashlib.sha256(args.apk.read_bytes()).hexdigest(),quiet_grounded_play_creates_safe_checkpoint=True,background_and_combat_preserve_safe_and_manual=True,manual_load_restores_explicit_choice=True,android_background_process_kill=True,repeated_real_combat_deaths_counted_persistently=True,ordinary_defeat_safe_choice=True,corrupt_resume_safe_recovery=True,limits=['x86_64 emulator, not physical ARM64 acceptance','Threat fixture modifies HP and one live droid position in a genuine B checkpoint; actual native attack causes loss'])
(OUT/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps(report),flush=True)
