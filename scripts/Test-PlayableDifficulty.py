"""Load each difficulty through DocumentsUI and compare passive defeat with Rust oracle."""
import argparse
import hashlib
import importlib.util
import json
import re
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--scenes', type=Path, default=ROOT/'analysis/milestone3h')
parser.add_argument('--output', type=Path, default=ROOT/'analysis/milestone3h/android-difficulty')
parser.add_argument('--apk', type=Path, default=ROOT/'analysis/milestone3h/republic-commando-0.16.0.apk')
args = parser.parse_args()
OUT = args.output
spec = importlib.util.spec_from_file_location('play_ui', ROOT / 'scripts/Test-PlayableEmulator.py')
ui = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ui)
OUT.mkdir(parents=True, exist_ok=True)
oracle = json.loads((args.scenes / 'check/game-rules-validation.json').read_text())
ui.adb('install', '-r', args.apk)
rows = []
for expected in oracle['levels']:
    level = expected['level']
    ui.OUT = OUT / level
    ui.OUT.mkdir(parents=True, exist_ok=True)
    scene = args.scenes / f'geo_01a-{level}.rcplay'
    ui.adb('push', scene, '/sdcard/Download/geo_01a.rcplay')
    ui.adb('shell', 'am', 'force-stop', 'org.rcport.diagnostic')
    ui.adb('logcat', '-c')
    ui.adb('shell', 'am', 'start', '-W', '-n', 'org.rcport.diagnostic/.MainActivity',
           '-a', 'android.intent.action.MAIN', '-c', 'android.intent.category.LAUNCHER', '-f', '0x10200000')
    ui.tap_node(ui.find(ui.ui(), 'SPIELABSCHNITT GEO_01A ÖFFNEN'))
    ui.tap_node(ui.find(ui.ui(), 'SZENE ÖFFNEN'))
    ui.select_file('geo_01a.rcplay')
    deadline = time.monotonic() + max(25, expected['defeat_tick']/60*3+20)
    defeat = None
    while time.monotonic() < deadline:
        log = ui.logs('passive-combat')
        defeat = re.search(r'state hp=0 ammo=60 alive=3 phase=2 ticks=(\d+) shots=0 kills=0 jumps=0', log)
        if defeat:
            break
        time.sleep(0.25)
    assert defeat, f'{level}: passive defeat missing'
    assert int(defeat[1]) == expected['defeat_tick'], (level, defeat[1], expected)
    tree = ui.ui('defeat')
    assert any(n.attrib.get('text', '').startswith('Gefallen') for n in tree.iter('node'))
    ui.capture('defeat')
    ui.adb('logcat', '-c')
    ui.tap_node(ui.find(tree, 'NEUSTART'))
    deadline = time.monotonic() + 4
    restarted = False
    while time.monotonic() < deadline:
        log = ui.logs('restart')
        if 'state hp=100 ammo=60 alive=3 phase=0 ticks=0 shots=0 kills=0 jumps=0' in log:
            restarted = True
            break
        time.sleep(0.1)
    assert restarted, f'{level}: restart missing'
    ui.adb('shell', 'am', 'force-stop', 'org.rcport.diagnostic')
    crash = ui.adb('logcat', '-d', '-b', 'crash')
    assert 'FATAL EXCEPTION' not in crash and 'Fatal signal' not in crash
    rows.append(dict(level=level, scene_sha256=hashlib.sha256(scene.read_bytes()).hexdigest(),
                     defeat_tick=int(defeat[1]), original_headless_tick_exact=True,
                     normal_picker=True, restart=True, no_crash=True))
receipt = dict(passed=True, android_api=ui.adb('shell','getprop','ro.build.version.sdk').strip(),
               abi=ui.adb('shell','getprop','ro.product.cpu.abi').strip(), levels=rows,
               scope='Normal picker and passive no-input combat/restart for all levels; interactive victory covered separately in backend receipts, which identify their scene. No gameplay-state injection; physical ARM64 pending.')
(OUT / 'validation.json').write_text(json.dumps(receipt, indent=2), encoding='utf-8')
print(json.dumps(receipt))
