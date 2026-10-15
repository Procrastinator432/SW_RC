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

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/mission4-complete/android'
OUT.mkdir(parents=True,exist_ok=True)
spec=importlib.util.spec_from_file_location('playable',ROOT/'scripts/Test-PlayableEmulator.py')
ui=importlib.util.module_from_spec(spec)
spec.loader.exec_module(ui)
ui.OUT=OUT
adb=ui.adb
remote='/data/local/tmp/rc-gunner-test'
adb('shell','mkdir','-p',remote)
for source,name in [(OUT.parent/'android-mission-replay','android-mission-replay'),
                    (ROOT/'android/app/src/main/jniLibs/x86_64/librc_native.so','librc_native.so'),
                    (OUT.parent/'geo_01a-gunner.rcplay','scene.rcplay'),
                    (OUT.parent/'native/input-replay.bin','input-replay.bin')]:
    adb('push',source,f'{remote}/{name}')
raw=adb('shell',f'cd {remote} && chmod 700 android-mission-replay && LD_LIBRARY_PATH=. ./android-mission-replay scene.rcplay input-replay.bin')
result=json.loads(raw)
assert result['passed'] and result['phase']==1 and result['health']>0
for local,name in [(OUT.parent/'geo_01a-gunner.rcplay','scene.rcplay'),(ROOT/'android/app/src/main/jniLibs/x86_64/librc_native.so','librc_native.so')]:
    digest=hashlib.sha256(local.read_bytes()).hexdigest()
    assert adb('shell','sha256sum',f'{remote}/{name}').split()[0]==digest
    result[name+'_sha256']=digest
(OUT/'native-replay.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
adb('pull',f'{remote}/complete.argb',OUT/'native-complete.argb')
ui.bootstrap(profile=True,scene=OUT.parent/'geo_01a-gunner.rcplay')
tree=ui.ui('paused')
assert ui.find(tree,'Pausiert') is not None
ui.capture('paused')
control=next(n for n in tree.iter('node') if n.attrib.get('content-desc','').startswith('Links Joystick'))
left,top,right,bottom=ui.bounds(control)
r=min(right-left,bottom-top)*.145
ui.tap_node(ui.find(tree,'FORTSETZEN'))
ui.swipe(right-r*1.3,bottom-r*1.7,right-r*1.3,bottom-r*1.7,400)
time.sleep(.2)
ui.capture('gpu-fire')
ui.tap_node(ui.find(ui.ui(),'PAUSE'))
log=adb('logcat','-d','-s','RCPlay:I','RCGpu:I','RCPerf:I','RCAudio:I','AndroidRuntime:E','*:S')
(OUT/'app.log').write_text(log,encoding='utf-8')
assert 'loaded geo_01a' in log and 'upload textures=' in log and '"type":"draw"' in log
assert 'Graphics failure' not in log and 'FATAL EXCEPTION' not in log
assert any(f'shots={n}' in log for n in range(1,20)), 'Touch fire produced no shot'
assert 'bank sounds=20' in log and ' sound=4 ' in log
assert 'sample load failed' not in log and 'play failed' not in log
ui.ui('paused-after-fire')
ui.capture('paused-after-fire')
(OUT/'validation.json').write_text(json.dumps(dict(passed=True,api=adb('shell','getprop','ro.build.version.sdk').strip(),
    abi=adb('shell','getprop','ro.product.cpu.abi').strip(),android_native_contiguous_input_replay=True,
    apk_scene_picker=True,gpu_render=True,touch_fire=True,pause=True,
    limitations=['Complete mission replay uses Android native ABI inputs, not a complete touch-UI playthrough',
                 'Physical ARM64 device acceptance remains with the user']),indent=2),encoding='utf-8')
print('Android native mission victory, original audio events, CPU frames, APK scene load, GPU, touch fire and pause passed')
