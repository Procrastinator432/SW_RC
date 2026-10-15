"""Separate native-process checkpoint restore, then APK process-loss and recovery acceptance."""
import importlib.util,json,subprocess,time,struct,hashlib,sys,argparse,base64,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser();parser.add_argument('--output-dir',type=Path,default=ROOT/'analysis/mission30-checkpoint-recovery/delivery/android');parser.add_argument('--apk',type=Path,default=ROOT/'analysis/mission30-checkpoint-recovery/delivery/republic-commando-0.47.0.apk');parser.add_argument('--native-probe',type=Path,default=ROOT/'analysis/mission29-checkpoints/delivery/android-checkpoint-replay');parser.add_argument('--ui-only',action='store_true');args=parser.parse_args()
OUT=args.output_dir;OUT.mkdir(parents=True,exist_ok=True)
spec=importlib.util.spec_from_file_location('playable',ROOT/'scripts/Test-PlayableEmulator.py');ui=importlib.util.module_from_spec(spec);spec.loader.exec_module(ui);ui.OUT=OUT
ADB=ui.ADB

def raw(*args,input=None):
 for attempt in range(6):
  r=subprocess.run([str(ADB),'-s','emulator-5554',*map(str,args)],input=input,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=120)
  if r.returncode==0:return r.stdout
  if attempt==5:raise RuntimeError(r.stdout.decode(errors='replace')+r.stderr.decode(errors='replace'))
  subprocess.run([str(ADB),'reconnect','offline'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);time.sleep(1)
def adb(*args):return raw(*args).decode('utf-8',errors='replace')
def tree():
 for attempt in range(6):
  try:return ui.ui()
  except (subprocess.CalledProcessError,Exception):
   if attempt==5:raise
   subprocess.run([str(ADB),'reconnect','offline'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);time.sleep(.5)
def find(text):
 for n in tree().iter('node'):
  if n.get('text','').casefold()==text.casefold():return n
 raise LookupError(text)
def tap(text):ui.tap_node(find(text))
def wait(predicate,seconds=40):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  if predicate():return
  time.sleep(.4)
 raise AssertionError('Timed out waiting for expected app state')
def has(text):return any(text.casefold() in n.get('text','').casefold() for n in tree().iter('node'))
def put(path,bytes):
 assert re.fullmatch(r'[a-z0-9_./-]+',path) and '..' not in path
 raw('shell','-T','run-as','org.rcport.diagnostic','tee','files/checkpoint-test-upload.b64',input=base64.b64encode(bytes))
 adb('shell',f"run-as org.rcport.diagnostic sh -c 'base64 -d files/checkpoint-test-upload.b64 > {path}'")
 assert file(path)==bytes,'Incomplete ADB test fixture transfer'
def file(path):return raw('exec-out','run-as','org.rcport.diagnostic','cat',path)
def slot(name='resume.save'):
 b=file('files/checkpoints/'+name);assert b[:8]==b'RCSLOT01';n=struct.unpack_from('>I',b,8)[0];body=b[44:];assert n==len(body) and hashlib.sha256(body).digest()==b[12:44]
 at=0
 def utf():
  nonlocal at
  n=struct.unpack_from('>H',body,at)[0];at+=2;s=body[at:at+n].decode('utf-8');at+=n;return s
 scene=utf();at+=8;title=utf();n=struct.unpack_from('>I',body,at)[0];at+=4;checkpoint=body[at:at+n];assert checkpoint[:8]==b'RCSAVE01';return json.loads(checkpoint[20:]),scene,title

def launch():
 adb('shell','am','force-stop','org.rcport.diagnostic');adb('shell','am','start','-W','-n','org.rcport.diagnostic/.MainActivity','--ez','rcProfile','true');wait(lambda:has('Spielabschnitt geo_01a öffnen'))
def choose(name):
 wait(lambda:has(name));tap(name)
def open_scene(name):
 tap('Szene öffnen');wait(lambda:any(n.get('package')=='com.google.android.documentsui' for n in tree().iter('node')))
 try:tap(name)
 except LookupError:
  for _ in range(6):ui.swipe(540,650,540,1550,150)
  ui.select_file(name)

remote='/data/local/tmp/rc-checkpoint-test';adb('shell','mkdir','-p',remote)
if '--ui-only' not in sys.argv:
 for p,name in [(args.native_probe,'replay'),(ROOT/'android/app/src/main/jniLibs/x86_64/librc_native.so','librc_native.so'),(ROOT/'analysis/mission28-first-mission-complete/delivery/geo_03a.rcplay','factory.rcplay'),(ROOT/'analysis/mission28-first-mission-complete/delivery/playthrough/factory-input.bin','controls.bin'),(ROOT/'analysis/mission28-first-mission-complete/delivery/native/map3-handoff.json','handoff.json')]:adb('push',p,remote+'/'+name)
 adb('shell','am','force-stop','org.rcport.diagnostic');adb('shell',f'chmod 700 {remote}/replay')
 saved=json.loads(adb('shell',f'cd {remote} && LD_LIBRARY_PATH=. ./replay save factory.rcplay controls.bin handoff.json'))
 resumed=json.loads(adb('shell',f'cd {remote} && LD_LIBRARY_PATH=. ./replay resume factory.rcplay controls.bin handoff.json'));assert saved['passed'] and resumed['passed']
 (OUT/'native-validation.json').write_text(json.dumps(resumed,indent=2));adb('pull',remote+'/checkpoint.bin',OUT/'checkpoint.bin')

adb('install','-r',args.apk)
adb('shell','run-as','org.rcport.diagnostic','mkdir','-p','files')
put('files/geo_01c-handoff.json',(ROOT/'analysis/mission28-first-mission-complete/delivery/native/map3-handoff.json').read_bytes())
for name in ['geo_03a.rcplay','geo_01c.rcplay']:adb('push',ROOT/'analysis/mission28-first-mission-complete/delivery'/name,'/sdcard/Download/'+name)
launch();tap('Spielabschnitt geo_01a öffnen');wait(lambda:has('Szene öffnen'));open_scene('geo_03a.rcplay');wait(lambda:has('GEO_03A'))
# Ordinary touch input, then allow a periodic checkpoint without pressing save.
node=next(n for n in tree().iter('node') if n.get('content-desc','').startswith('Links Joystick'))
left,top,right,bottom=ui.bounds(node);r=min(right-left,bottom-top)*.145
ui.swipe(left+r*1.6,bottom-r*1.7,left+r*1.6,bottom-r*2.5,900)
ui.swipe(right-r*1.3,bottom-r*1.7,right-r*1.3,bottom-r*1.7,500)
wait(lambda:slot()[0]['state']['statistics']['ticks']>=600,seconds=35)
auto=slot()[0];assert auto['state']['statistics']['shots']>0
adb('shell','input','keyevent','3');time.sleep(2)
background=slot()[0];assert background['state']['statistics']['ticks']>=auto['state']['statistics']['ticks']
launch();tap('Fortsetzen');wait(lambda:has('GEO_03A') and has('Fortsetzen'))
tap('Checkpoint speichern');time.sleep(1);restored=slot()[0]
assert restored['state']==background['state'],'Process-loss resume changed saved simulation state'
ui.capture('continued-after-process-loss')
# Keep the preceding valid slot, corrupt the primary, and restore through the visible recovery choice.
previous=slot('previous.save')[0];launch();put('files/checkpoints/resume.save',b'truncated checkpoint');tap('Fortsetzen');wait(lambda:has('Spielstand wiederherstellen'))
ui.capture('corrupt-checkpoint-recovery');tap('Vorherigen Checkpoint laden');wait(lambda:has('GEO_03A') and has('Fortsetzen'))
# A repaired primary must exist immediately, without a save button or onPause.
wait(lambda:slot()[0]['state']==previous['state']);fallback=slot()[0]
for cycle in range(3):
 launch();tap('Fortsetzen');wait(lambda:has('GEO_03A') and has('Fortsetzen'))
 assert slot()[0]['state']==fallback['state'],'Repeated cold Continue changed the recovered state'
ui.capture('recovered-checkpoint-cold-continue')
# A real filesystem write error must preserve both committed generations.
primary_before=file('files/checkpoints/resume.save');backup_before=file('files/checkpoints/previous.save')
adb('shell','run-as','org.rcport.diagnostic','mkdir','-p','files/checkpoints/resume.save.tmp')
put('files/checkpoints/resume.save.tmp/keep',b'write-error fixture')
tap('Fortsetzen');time.sleep(1);tap('Checkpoint speichern');time.sleep(1)
assert file('files/checkpoints/resume.save')==primary_before and file('files/checkpoints/previous.save')==backup_before
assert 'save failed' in adb('logcat','-d','-s','RCCheckpoint:I','*:S')
adb('shell','run-as','org.rcport.diagnostic','rm','files/checkpoints/resume.save.tmp/keep');adb('shell','run-as','org.rcport.diagnostic','rmdir','files/checkpoints/resume.save.tmp')
tap('Pause');tap('Checkpoint speichern');wait(lambda:file('files/checkpoints/resume.save')!=primary_before)
# Rejecting an unrelated scene must retain the last usable recovery scene.
last_scene=file('files/checkpoints/last-scene')
invalid=OUT/'invalid-scene.rcplay';invalid.write_bytes(b'not a playable scene')
adb('push',invalid,'/sdcard/Download/invalid-scene.rcplay');open_scene('invalid-scene.rcplay');wait(lambda:has('Spielstand wiederherstellen'))
assert file('files/checkpoints/last-scene')==last_scene
ui.capture('rejected-scene-recovery');tap('Abbrechen')
# A bad legacy handoff must provide a usable scene-start action, rather than a permanent loading error.
original_handoff=file('files/geo_01b-handoff.json')
put('files/geo_01b-handoff.json',b'broken transition');open_scene('geo_01c.rcplay');wait(lambda:has('Spielstand wiederherstellen'))
ui.capture('corrupt-handoff-recovery');tap('Ohne gespeicherte Übergabe starten');wait(lambda:has('GEO_01C'))
assert file('files/geo_01b-handoff.json.corrupt')==b'broken transition'
put('files/geo_01b-handoff.json',original_handoff)
if has('Pause'):tap('Pause')
log=adb('logcat','-d','-s','RCPlay:I','RCCheckpoint:I','AndroidRuntime:E','*:S');(OUT/'app.log').write_text(log)
assert 'FATAL EXCEPTION' not in log
(OUT/'validation.json').write_text(json.dumps(dict(passed=True,periodic_autosave=True,background_save=True,actual_force_stop_and_continue_exact_state=True,private_scene_cache_without_picker=True,corrupt_primary_previous_checkpoint_recovery=True,previous_recovery_immediately_promoted=True,repeated_cold_continue_cycles=3,actual_write_failure_preserves_both_generations=True,retry_after_write_failure=True,rejected_scene_preserves_recovery_scene=True,corrupt_handoff_fresh_start_recovery=True,corrupt_files_quarantined=True,pause=True,background_ticks=background['state']['statistics']['ticks'],background_health=background['state']['health'],background_ammo=background['state']['ammo'],limitations=['Automated x86_64 test device; physical ARM64 acceptance remains open','Write errors injected with blocked staging paths; actual disk exhaustion and power loss not tested','In-flight audio playback position is not persisted']),indent=2))
print('Android process-loss Continue, automatic/background save and both visible corruption-recovery paths passed')
