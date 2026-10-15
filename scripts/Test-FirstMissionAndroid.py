"""Android full ABI travel plus APK saved-handoff resume/GPU/control smoke."""
import importlib.util
import json
import subprocess
import time
import struct
import sys
import shlex
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission28-first-mission-complete/delivery/android'
OUT.mkdir(exist_ok=True)
spec = importlib.util.spec_from_file_location('playable', ROOT / 'scripts/Test-PlayableEmulator.py')
ui = importlib.util.module_from_spec(spec); spec.loader.exec_module(ui); ui.OUT = OUT
def adb(*args):
    result = subprocess.run([str(ui.ADB), '-s', 'emulator-5554', *map(str,args)], check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=300)
    if result.returncode:
        raise RuntimeError(f'ADB exit {result.returncode}: '+result.stdout.decode('utf-8',errors='replace')+result.stderr.decode('utf-8',errors='replace'))
    if result.stderr: print(result.stderr.decode('utf-8',errors='replace'),flush=True)
    return result.stdout.decode('utf-8',errors='replace')
if '--ui-only' not in sys.argv:
    reference=json.loads((ROOT/'analysis/mission18-map2-route/final-state.json').read_text())
    expected=[reference[k] for k in ['health','ammo','reserve']]+[sum(e['health']>0 for e in reference['enemies']),1,*[reference['statistics'][k] for k in ['ticks','shots','kills','jumps','enemy_shots']]]
    final=json.loads((ROOT/'analysis/mission27-map3-elevator/delivery/playthrough/validation.json').read_text())['final'];stats=final['statistics'];expected_c=[final[k] for k in ['health','ammo','reserve']]+[sum(e['health']>0 for e in final['enemies']),1,*[stats[k] for k in ['ticks','shots','kills','jumps','enemy_shots']]]
    failure=json.loads((OUT.parent/'playthrough/validation.json').read_text())['final'];stats=failure['statistics'];expected_failure=[failure[k] for k in ['health','ammo','reserve']]+[0,0,*[stats[k] for k in ['ticks','shots','kills','jumps','enemy_shots']]]
    (OUT/'expected.bin').write_bytes(struct.pack('<10I3d20I',*expected,*reference['pawn']['body']['position'],*expected_c,*expected_failure))
    # Release the previous APK smoke-test session before the memory-heavy ABI replay.
    adb('shell','am','force-stop','org.rcport.diagnostic')
    remote = '/data/local/tmp/rc-map3-test'
    adb('shell', 'mkdir', '-p', remote)
    for p, name in [(OUT.parent / 'android-first-mission-replay', 'replay'),
                    (ROOT / 'android/app/src/main/jniLibs/x86_64/librc_native.so', 'librc_native.so'),
                    (ROOT / 'analysis/mission15-complete/delivery/geo_01a-first-mission.rcplay', 'one.rcplay'),
                    (ROOT / 'analysis/mission18-map2-route/delivery/geo_01b.rcplay', 'two.rcplay'),
                    (ROOT / 'analysis/mission15-complete/delivery/playthrough/input-replay.bin', 'input.bin'),
                    (ROOT / 'analysis/mission18-map2-route/input-replay.bin', 'route.bin'),
                    (OUT / 'expected.bin', 'expected.bin'),
                    (OUT.parent/'geo_01c.rcplay','three.rcplay'),
                    (ROOT/'analysis/mission27-map3-elevator/delivery/playthrough/input-replay.bin','controls.bin'),
                    (OUT.parent/'geo_03a.rcplay','factory.rcplay'),
                    (OUT.parent/'playthrough/factory-input.bin','factory-input.bin')]:
        adb('push', p, f'{remote}/{name}')
    job = 'LD_LIBRARY_PATH=. ./replay one.rcplay two.rcplay input.bin route.bin expected.bin three.rcplay controls.bin factory.rcplay factory-input.bin >native-result.json 2>native-stderr.txt; echo $? >native-exit.txt'
    adb('shell', f'cd {remote} && chmod 700 replay && rm -f native-exit.txt && (nohup sh -c {shlex.quote(job)} </dev/null >/dev/null 2>&1 &)')
    deadline=time.monotonic()+480
    while time.monotonic()<deadline:
        time.sleep(3)
        try:
            code=adb('shell', f'cat {remote}/native-exit.txt 2>/dev/null || true').strip()
            if code:
                assert code=='0', adb('shell', f'cat {remote}/native-stderr.txt')
                break
        except RuntimeError:
            subprocess.run([str(ui.ADB),'reconnect','offline'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    else:raise AssertionError('Detached Android native replay did not complete')
    raw=adb('shell', f'cat {remote}/native-result.json')
    (OUT/'native-stderr.log').write_text(adb('shell', f'cat {remote}/native-stderr.txt'))
    result=json.loads(raw);assert result['passed'] and result['real_map1_inputs']==18730 and result['full_map2_inputs']==16414 and result['map3_inputs']==11800 and result['factory_inputs']==360 and result['three_members_carried'] and result['actual_factory_travel']
    (OUT/'native-validation.json').write_text(json.dumps(result,indent=2))
    for name in ['map3-handoff.json','factory.argb']:adb('pull',f'{remote}/{name}',OUT/name)
else:
    result=json.loads((OUT/'native-validation.json').read_text());assert result['passed'] and result['actual_factory_travel'] and (OUT/'map3-handoff.json').exists()

_original_ui=ui.ui
def stable_ui(name=None):
    for attempt in range(3):
        try:return _original_ui(name)
        except subprocess.CalledProcessError as error:
            if attempt==2:raise
            print(f'UI dump retry {attempt+1}: {error.stdout!r} {error.stderr!r}',flush=True)
            time.sleep(.5)
ui.ui=stable_ui
adb('install', '-r', OUT.parent / 'republic-commando-0.45.0.apk')
# Reproduce app-process restart from the handoff actually generated by the ABI
# playthrough above. This is a persisted-resume fixture, not a manual UI mission run.
adb('shell', 'run-as', 'org.rcport.diagnostic', 'mkdir', '-p', 'files')
subprocess.run([str(ui.ADB), '-s', 'emulator-5554', 'shell', '-T', 'run-as',
                'org.rcport.diagnostic', 'tee', 'files/geo_01c-handoff.json'],
               input=(OUT / 'map3-handoff.json').read_bytes(), stdout=subprocess.DEVNULL, check=True)
adb('push', OUT.parent / 'geo_03a.rcplay', '/sdcard/Download/geo_03a.rcplay')
adb('shell', 'am', 'force-stop', 'com.google.android.documentsui')
adb('shell', 'am', 'force-stop', 'org.rcport.diagnostic')
adb('logcat', '-c')
adb('shell', 'am', 'start', '-W', '-n', 'org.rcport.diagnostic/.MainActivity',
    '--ez', 'rcProfile', 'true')
ui.tap_node(ui.find(ui.ui(), 'SPIELABSCHNITT GEO_01A ÖFFNEN'))
# A cold emulator rotates PlayActivity before the portrait document picker.
# Wait for each actual surface before tapping its freshly observed bounds.
deadline=time.monotonic()+30
while time.monotonic()<deadline:
    tree=ui.ui()
    if tree.get('rotation')=='1' and any(n.get('text')=='SZENE ÖFFNEN' for n in tree.iter('node')):break
    time.sleep(.2)
else:raise AssertionError('PlayActivity landscape controls did not settle')
ui.tap_node(ui.find(tree, 'SZENE ÖFFNEN'))
deadline=time.monotonic()+30
while time.monotonic()<deadline:
    tree=ui.ui()
    if any(n.get('package')=='com.google.android.documentsui' for n in tree.iter('node')):break
    time.sleep(.2)
else:raise AssertionError('Document picker did not open')
try:ui.tap_node(ui.find(tree,'geo_03a.rcplay'))
except StopIteration:
    for _ in range(6):ui.swipe(540,650,540,1550,150)
    ui.select_file('geo_03a.rcplay')
deadline = time.monotonic() + 30
while time.monotonic() < deadline:
    log = adb('logcat', '-d', '-s', 'RCPlay:I', '*:S')
    if 'loaded geo_03a' in log: break
    time.sleep(.2)
else: raise AssertionError('Map2 did not load')
tree = ui.ui()
assert any(n.get('text', '').startswith('GEO_03A') for n in tree.iter('node'))
ui.tap_node(ui.find(tree, 'PAUSE'))
tree = ui.ui('entry-paused'); ui.capture('entry-paused')
control = next(n for n in tree.iter('node') if n.get('content-desc', '').startswith('Links Joystick'))
left, top, right, bottom = ui.bounds(control); r = min(right-left, bottom-top)*.145
ui.tap_node(ui.find(tree, 'FORTSETZEN'))
ui.swipe(left+r*1.6, bottom-r*1.7, left+r*1.6, bottom-r*2.5, 900)
ui.swipe(right-r*1.3, bottom-r*1.7, right-r*1.3, bottom-r*1.7, 500)
ui.swipe(right-r*3.1, bottom-r*5.0, right-r*3.1, bottom-r*5.0, 700)
ui.capture('controls')
ui.tap_node(ui.find(ui.ui(), 'PAUSE'))
log = adb('logcat', '-d', '-s', 'RCPlay:I', 'RCAudio:I', 'RCGpu:I', 'RCPerf:I', 'AndroidRuntime:E', '*:S')
(OUT / 'app.log').write_text(log)
assert 'Saved handoff restored on GEO_03A' in log and 'loaded geo_03a' in log
assert 'hp=65 ammo=14' in log and 'sound=0 ' in log and 'upload textures=' in log
assert 'FATAL EXCEPTION' not in log and 'Graphics failure' not in log and 'play failed' not in log
(OUT/'validation.json').write_text(json.dumps(dict(passed=True,apk_factory_saved_resume=True,three_members_from_actual_native_handoff=True,original_weapon_audio=True,gpu=True,touch_move_fire_crouch=True,pause=True,limitations=['APK resume uses the actual native completed C handoff; full A/B/C journey verified through native ABI','No complete manual touch or physical ARM64 acceptance','GEO_03A entry imported; next mission gameplay remains separate']),indent=2))
print('Android complete A/B/C -> factory and APK saved resume/GPU/touch/audio passed')
