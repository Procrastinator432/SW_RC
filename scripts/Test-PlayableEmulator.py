"""Normal Android UI end-to-end test; no gameplay shortcuts or injected game-state answers."""
import argparse
import json
import math
import re
import subprocess
import time
import xml.etree.ElementTree as ET
from pathlib import Path
from PIL import Image, ImageChops

ROOT = Path(__file__).resolve().parents[1]
ADB = ROOT / 'tools/android-sdk/platform-tools/adb.exe'
OUT = ROOT / 'analysis/playable/android'
OUT.mkdir(parents=True, exist_ok=True)

def adb(*args, binary=False):
    result = subprocess.run([str(ADB), '-s', 'emulator-5554', *map(str, args)],
                            capture_output=True, check=True, timeout=45)
    return result.stdout if binary else result.stdout.decode('utf-8', errors='replace')

def ui(name=None):
    adb('shell', 'uiautomator', 'dump', '/data/local/tmp/rc-play-ui.xml')
    raw = adb('shell', 'cat', '/data/local/tmp/rc-play-ui.xml')
    if name:
        (OUT / f'{name}.xml').write_text(raw, encoding='utf-8')
    return ET.fromstring(raw)

def bounds(node):
    return list(map(int, re.findall(r'\d+', node.attrib['bounds'])))

def find(tree, text):
    return next(n for n in tree.iter('node') if n.attrib.get('text', '').casefold() == text.casefold())

def tap(x, y):
    adb('shell', 'input', 'tap', round(x), round(y))

def tap_node(node):
    x1, y1, x2, y2 = bounds(node)
    tap((x1+x2)/2, (y1+y2)/2)

def capture(name):
    (OUT / f'{name}.png').write_bytes(adb('exec-out', 'screencap', '-p', binary=True))

def logs(name):
    value = adb('logcat', '-d', '-s', 'RCPlay:I', '*:S')
    (OUT / f'{name}.log').write_text(value, encoding='utf-8')
    return value

def swipe(x1, y1, x2, y2, duration=250):
    adb('shell', 'input', 'touchscreen', 'swipe', round(x1), round(y1), round(x2), round(y2), duration)

def select_file(name):
    tree = ui()
    for _ in range(10):
        try:
            tap_node(find(tree, name))
            return
        except StopIteration:
            adb('shell', 'input', 'swipe', 540, 1550, 540, 650, 250)
            tree = ui()
    raise AssertionError(f'{name} not found in Android picker')

def pause_loaded_scene():
    # Returning from DocumentsUI rotates portrait -> landscape. Wait for the actual
    # loaded activity and its final bounds, rather than tapping a cached coordinate.
    for _ in range(5):
        tree = ui()
        if tree.attrib.get('rotation') == '1' and any(
                n.attrib.get('text', '').startswith(('Drei Drohnen ausschalten','Drei Gegner ausschalten','Gefallen','Bacta erreichen','Bacta:','Landungsbriefing')) for n in tree.iter('node')):
            tap_node(find(tree, 'PAUSE'))
            # A short passive survival time can expire during the UI dump.
            # Restart and pause through the ordinary controls using the already
            # observed landscape bounds, without another expensive dump between.
            observed = ui()
            if any(n.attrib.get('text', '').startswith('Gefallen') for n in observed.iter('node')):
                tap_node(find(tree, 'NEUSTART'))
                tap_node(find(tree, 'PAUSE'))
            return
    raise AssertionError('Loaded landscape game activity not ready')

def bootstrap(profile=False, software=False, gpu_capture=False, scene=None):
    adb('install', '-r', ROOT / 'android/app/build/outputs/apk/debug/app-debug.apk')
    adb('push', scene or ROOT / 'analysis/playable/geo_01a.rcplay', '/sdcard/Download/geo_01a.rcplay')
    adb('shell', 'am', 'force-stop', 'org.rcport.diagnostic')
    adb('logcat', '-c')
    adb('shell', 'am', 'start', '-W', '-n', 'org.rcport.diagnostic/.MainActivity', '-a', 'android.intent.action.MAIN', '-c', 'android.intent.category.LAUNCHER', '-f', '0x10200000', '--ez', 'rcProfile', str(profile).lower(), '--ez','rcSoftware',str(software).lower(),'--ez','rcGpuCapture',str(gpu_capture).lower())
    tap_node(find(ui(), 'SPIELABSCHNITT GEO_01A ÖFFNEN'))
    tree = ui()
    try:
        tap_node(find(tree, 'Got it'))
        tree = ui()
    except StopIteration:
        pass
    tap_node(find(tree, 'SZENE ÖFFNEN'))
    select_file('geo_01a.rcplay')
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if 'loaded geo_01a' in adb('logcat', '-d', '-s', 'RCPlay:I', '*:S'):
            break
        time.sleep(0.1)
    else:
        raise AssertionError('Scene did not load')
    pause_loaded_scene()

def main():
    global OUT
    parser = argparse.ArgumentParser()
    parser.add_argument('--magazine', type=int, default=20)
    parser.add_argument('--scene', type=Path, help='Use a separate self-contained scene bundle')
    parser.add_argument('--loaded', action='store_true', help='Use the already loaded, paused scene')
    parser.add_argument('--profile', action='store_true')
    parser.add_argument('--software', action='store_true', help='Exercise CPU fallback instead of the default GPU')
    parser.add_argument('--output', type=Path, help='Separate receipt directory')
    args = parser.parse_args()
    if args.output:
        OUT = args.output.resolve()
        OUT.mkdir(parents=True, exist_ok=True)
    (OUT/'validation.json').unlink(missing_ok=True)
    if not args.loaded:
        bootstrap(args.profile,args.software,scene=args.scene)
    tree = ui('paused')
    assert find(tree, 'Pausiert') is not None
    restart = find(tree, 'NEUSTART')
    pause_bounds = bounds(find(tree, 'FORTSETZEN'))
    control = next(n for n in tree.iter('node') if n.attrib.get('content-desc', '').startswith('Links Joystick'))
    left, top, right, bottom = bounds(control)
    w, h = right-left, bottom-top
    r = min(w, h)*0.145
    capture('paused-before')
    time.sleep(0.4)
    capture('paused-after')
    assert ImageChops.difference(Image.open(OUT/'paused-before.png'), Image.open(OUT/'paused-after.png')).getbbox() is None

    report = json.loads((ROOT / 'analysis/playable/geo_01a.json').read_text())
    spawn, yaw = report['spawn'], report['yaw']
    heading, pitch = yaw, 0.0
    tap_node(restart)
    time.sleep(0.15)
    # Touch-down and drag go through the normal right-side look control.
    for target in report['enemies']:
        dx, dy = target[0]-spawn[0], target[1]-spawn[1]
        wanted_heading = math.atan2(dy, dx)
        wanted_pitch = math.atan2(target[2]-(spawn[2]+84*0.65), math.hypot(dx, dy))
        turn = (wanted_heading-heading+math.pi) % math.tau-math.pi
        swipe(left+w*0.66, top+h*0.38,
              left+w*0.66-turn*w/3, top+h*0.38-(wanted_pitch-pitch)*h/2, 180)
        heading, pitch = wanted_heading, wanted_pitch
        # Hold the normal on-screen fire button for two or three shots.
        x, y = right-r*1.3, bottom-r*1.7
        swipe(x, y, x, y, 350 if args.magazine == 60 else 450)
    combat_log = logs('combat')
    capture('cleared')
    assert 'kills=3' in combat_log and 'alive=0 phase=0' in combat_log, combat_log[-3000:]
    tap(right-r*1.3, bottom-r*3.5)
    # Wait for the real simulation completion, not a fixed host sleep. CPU rendering
    # and emulator scheduling can put the original clip duration beyond 1.5 seconds.
    reload_pattern = r'state hp=\d+ ammo=' + str(args.magazine) + r' alive=0 phase=0'
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        reload_log = logs('reload')
        if re.search(reload_pattern, reload_log):
            break
        time.sleep(0.15)
    else:
        raise AssertionError('Touch reload did not complete within 8 seconds')
    capture('reloaded')
    # Align with the exit and hold the normal movement stick forward.
    target = report['exit']
    wanted_heading = math.atan2(target[1]-spawn[1], target[0]-spawn[0])
    turn = (wanted_heading-heading+math.pi) % math.tau-math.pi
    swipe(left+w*0.66, top+h*0.38,
          left+w*0.66-turn*w/3, top+h*0.38+pitch*h/2, 180)
    x, y = left+r*1.6, bottom-r*2.65
    swipe(x, y, x, y, 1400)
    time.sleep(0.2)
    victory_ui = ui('victory')
    assert any(n.attrib.get('text', '').startswith('Abschnitt geschafft') for n in victory_ui.iter('node'))
    capture('victory')
    victory_log = logs('victory')
    assert 'phase=1' in victory_log

    # A second live run: touch jump and enemy damage/defeat.
    tap_node(find(victory_ui, 'NEUSTART'))
    time.sleep(0.15)
    tap(right-r*3.1, bottom-r*1.3)
    time.sleep(1.2)
    capture('jump-run')
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline:
        if 'phase=2' in adb('logcat', '-d', '-s', 'RCPlay:I', '*:S'):
            break
        time.sleep(0.5)
    else:
        raise AssertionError('Enemy fire did not produce defeat')
    defeat_ui = ui('defeat')
    assert any(n.attrib.get('text', '').startswith('Gefallen') for n in defeat_ui.iter('node'))
    capture('defeat')
    defeat_log = logs('defeat')
    assert 'jumps=1' in defeat_log
    tap_node(find(defeat_ui, 'NEUSTART'))
    time.sleep(0.2)
    tap((pause_bounds[0]+pause_bounds[2])/2, (pause_bounds[1]+pause_bounds[3])/2)
    reset_ui = ui('restart-paused')
    capture('restart-paused')
    invalid = OUT / 'invalid.rcplay'
    invalid.write_bytes(b'RCPLAY01\xff\xff\xff\xff')
    adb('push', invalid, '/sdcard/Download/invalid.rcplay')
    tap_node(find(reset_ui, 'SZENE ÖFFNEN'))
    select_file('invalid.rcplay')
    error_ui = ui('invalid-file')
    assert any(n.attrib.get('text', '').startswith('Ladefehler:') for n in error_ui.iter('node'))
    capture('invalid-file')
    tap_node(find(error_ui, 'SZENE ÖFFNEN'))
    select_file('geo_01a.rcplay')
    pause_loaded_scene()
    recovery_ui = ui('recovery')
    assert find(recovery_ui, 'Pausiert') is not None
    capture('recovery')
    crash = adb('logcat', '-d', '-b', 'crash')
    (OUT / 'crash.log').write_text(crash, encoding='utf-8')
    assert 'FATAL EXCEPTION' not in crash and 'Fatal signal' not in crash
    result = dict(scene=str((args.scene or ROOT/'analysis/playable/geo_01a.rcplay').resolve()),android_api=adb('shell','getprop','ro.build.version.sdk').strip(),
        abi=adb('shell','getprop','ro.product.cpu.abi').strip(),
        normal_picker_load=True, touch_aim_and_fire=True, three_kills=True,
        touch_move_and_exit_victory=True, touch_jump=True, enemy_damage_and_defeat=True,
        touch_reload=True, restart=True, pause_frames_exact=True, invalid_file_and_recovery=True, no_crash=True, scope='Normal Android UI with the same Rust scene bundle; no native-state injection. Multitouch and physical ARM64 not asserted.')
    if args.profile:
        performance=adb('logcat','-d','-s','RCPerf:I','*:S')
        (OUT/'performance.log').write_text(performance,encoding='utf-8')
        frames=[json.loads(line[line.index('{'):]) for line in performance.splitlines() if '{' in line and '"type":"frame"' in line]
        expected='cpu' if args.software else 'gpu'
        assert frames and all(f.get('backend')==expected for f in frames),'Unexpected graphics fallback'
        result['backend']=expected
        gpu_log=adb('logcat','-d','-s','RCGpu:I','*:S')
        (OUT/'gpu.log').write_text(gpu_log,encoding='utf-8')
        assert 'Graphics failure' not in gpu_log and 'GPU fallback' not in logs('final')
        (OUT/'memory.txt').write_text(adb('shell','dumpsys','meminfo','org.rcport.diagnostic'),encoding='utf-8')
    (OUT / 'validation.json').write_text(json.dumps(result, indent=2)+'\n')
    print(json.dumps(result))

if __name__ == '__main__':
    main()
