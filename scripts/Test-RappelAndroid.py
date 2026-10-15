"""Android-native input replay plus normal APK file-picker/GPU/control smoke test.

The replay exercises the Android engine ABI; it does not claim a manual touch
playthrough of the complete mission or physical ARM64-device acceptance.
"""
import importlib.util
import hashlib
import json
from pathlib import Path
import subprocess
import time
import sys

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/mission13-rappel/delivery/android'
OUT.mkdir(parents=True,exist_ok=True)
spec=importlib.util.spec_from_file_location('playable',ROOT/'scripts/Test-PlayableEmulator.py')
ui=importlib.util.module_from_spec(spec)
spec.loader.exec_module(ui)
ui.OUT=OUT
adb=ui.adb
if '--ui-only' not in sys.argv:
    remote='/data/local/tmp/rc-rappel-test'
    adb('shell','mkdir','-p',remote)
    for source,name in [(OUT.parent/'android-rappel-replay','android-rappel-replay'),(ROOT/'android/app/src/main/jniLibs/x86_64/librc_native.so','librc_native.so'),(OUT.parent/'geo_01a-rappel.rcplay','scene.rcplay'),(OUT.parent/'playthrough/input-replay.bin','input-replay.bin'),(OUT.parent/'live/squad-assist-input.bin','squad-assist-input.bin')]:
        adb('push',source,f'{remote}/{name}')
    command=f'cd {remote} && chmod 700 android-rappel-replay && LD_LIBRARY_PATH=. ./android-rappel-replay scene.rcplay input-replay.bin squad-assist-input.bin'
    raw=subprocess.run([str(ui.ADB),'-s','emulator-5554','shell',command],capture_output=True,text=True,check=True,timeout=240).stdout
    result=json.loads(raw)
    assert result['passed'] and result['runs'][0]['phase']==1 and result['runs'][1]['scorch_kills']>0
    for local,name in [(OUT.parent/'geo_01a-rappel.rcplay','scene.rcplay'),(ROOT/'android/app/src/main/jniLibs/x86_64/librc_native.so','librc_native.so'),(OUT.parent/'playthrough/input-replay.bin','input-replay.bin'),(OUT.parent/'live/squad-assist-input.bin','squad-assist-input.bin')]:
        digest=hashlib.sha256(local.read_bytes()).hexdigest();assert adb('shell','sha256sum',f'{remote}/{name}').split()[0]==digest;result[name+'_sha256']=digest
    (OUT/'native-replay.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    adb('pull',f'{remote}/complete.argb',OUT/'native-complete.argb')
    adb('pull',f'{remote}/assist.argb',OUT/'native-assist.argb')
ui.bootstrap(profile=True,scene=OUT.parent/'geo_01a-rappel.rcplay')
tree=ui.ui('paused')
assert ui.find(tree,'Pausiert') is not None
ui.capture('paused')
control=next(n for n in tree.iter('node') if n.attrib.get('content-desc','').startswith('Links Joystick'))
left,top,right,bottom=ui.bounds(control)
r=min(right-left,bottom-top)*.145
ui.tap_node(ui.find(tree,'FORTSETZEN'))
# Let the original briefing/injury finish through the normal application clock.
opening_deadline=time.monotonic()+90
while time.monotonic()<opening_deadline:
    current=ui.ui()
    if any(n.attrib.get('text','').startswith('Bacta erreichen') for n in current.iter('node')):break
    time.sleep(.2)
else:raise AssertionError('Opening never reached Bacta gameplay')
ui.capture('opening-complete')
ui.swipe(right-r*1.3,bottom-r*1.7,right-r*1.3,bottom-r*1.7,400)
time.sleep(.2)
ui.capture('gpu-fire')
ui.swipe(right-r*3.1,bottom-r*5.0,right-r*3.1,bottom-r*5.0,900)
time.sleep(.2)
ui.capture('gpu-duck-control')
ui.swipe(right-r*3.1,bottom-r*3.5,right-r*3.1,bottom-r*3.5,500)
time.sleep(.2)
ui.tap_node(ui.find(ui.ui(),'PAUSE'))
log=adb('logcat','-d','-s','RCPlay:I','RCGpu:I','RCPerf:I','RCAudio:I','AndroidRuntime:E','*:S')
(OUT/'app.log').write_text(log,encoding='utf-8')
assert 'loaded geo_01a' in log and 'upload textures=' in log and '"type":"draw"' in log
assert 'Graphics failure' not in log and 'FATAL EXCEPTION' not in log
assert any(f'shots={n}' in log for n in range(1,20)), 'Touch fire produced no shot'
assert 'bank sounds=80' in log and ' sound=4 ' in log
assert all(f' sound={role} ' in log for role in range(76,80)), 'Missing live rappel Foley'
assert 'sample load failed' not in log and 'play failed' not in log
records=[]
for line in log.splitlines():
    if 'RCPerf' in line and '{' in line:
        try:
            row=json.loads(line[line.index('{'):])
            if row.get('type')=='frame': records.append(row)
        except json.JSONDecodeError: pass
crouched=[r for r in records if r.get('buttons',0)&64]
assert any(r.get('buttons',0)&16 for r in records),'Touch use produced no use input'
assert crouched, 'Touch duck produced no crouch input'
first=next(i for i,r in enumerate(records) if r.get('buttons',0)&64)
standing=[r for r in records[:first] if r.get('buttons',0)==0]
assert standing
reference_z=standing[-1]['pose'][2]
assert min(r['pose'][2] for r in crouched)<reference_z-20, 'Player height did not decrease'
(OUT/'crouch-input-validation.json').write_text(json.dumps(dict(passed=True,standing_z=reference_z,crouched_min_z=min(r['pose'][2] for r in crouched),frames=len(crouched)),indent=2))
ui.ui('paused-after-fire')
ui.capture('paused-after-fire')
(OUT/'validation.json').write_text(json.dumps(dict(passed=True,api=adb('shell','getprop','ro.build.version.sdk').strip(),
    abi=adb('shell','getprop','ro.product.cpu.abi').strip(),android_native_contiguous_input_replay=True,
    apk_scene_picker=True,gpu_render=True,touch_fire=True,touch_use=True,touch_crouch_height_change=True,pause=True,
    limitations=['Complete mission replay uses Android native ABI inputs, not a complete touch-UI playthrough',
                 'Physical ARM64 device acceptance remains with the user']),indent=2),encoding='utf-8')
print('Android objectives0–6 replay, original encounter and artillery audio, APK/GPU, touch fire/crouch and pause passed')
