"""Run isolated original-geometry revival fixtures inside Android Rust runtime.

Fixture setup assigns health/actor positions, explicitly unlike the input-only
mission regressions. The same rc-squad-check executable source runs on Windows.
"""
import hashlib
import importlib.util
import json
import subprocess
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/mission11-revival/android-fixture'
OUT.mkdir(parents=True,exist_ok=True)
spec=importlib.util.spec_from_file_location('playable',ROOT/'scripts/Test-PlayableEmulator.py')
ui=importlib.util.module_from_spec(spec)
spec.loader.exec_module(ui)
remote='/data/local/tmp/rc-revival-fixture'
ui.adb('shell','mkdir','-p',remote)
sources=[(OUT.parent/'android-revival-check','fixture'),
         (OUT.parent/'geo_01a-revival.rcplay','scene.rcplay'),
         (OUT.parent/'playthrough/input-replay.bin','inputs.bin')]
digests={}
for path,name in sources:
    ui.adb('push',path,f'{remote}/{name}')
    digest=hashlib.sha256(path.read_bytes()).hexdigest()
    assert ui.adb('shell','sha256sum',f'{remote}/{name}').split()[0]==digest
    digests[name]=digest
command=f'cd {remote} && chmod 700 fixture && ./fixture scene.rcplay inputs.bin result'
run=subprocess.run([str(ui.ADB),'-s','emulator-5554','shell',command],capture_output=True,text=True,check=True,timeout=180)
(OUT/'run.log').write_text(run.stdout+run.stderr,encoding='utf-8')
for name in ['validation.json','revival-ramp.json','scorch-revived.ppm']:
    ui.adb('pull',f'{remote}/result/{name}',OUT/name)
ramp=json.loads((OUT/'revival-ramp.json').read_text())
assert ramp['passed'] and ramp['idle_after_getup'] and ramp['fractional_accumulation']
assert ramp==json.loads((OUT.parent/'behavior/revival-ramp.json').read_text())
state=json.loads((OUT/'validation.json').read_text())
assert state['passed'] and state['scorch_revive']['health']==200 and state['player_revive']['player_revives']==1
(OUT/'binding.json').write_text(json.dumps(dict(passed=True,android_rust_revive_fixture=True,
    health_timeline_matches_windows=True,digests=digests,
    scope='Isolated original-geometry fixture with actor and health setup; not an input-only or touch mission playthrough'),indent=2),encoding='utf-8')
print('Android original-geometry revival fixtures passed; health timelines match Windows')
