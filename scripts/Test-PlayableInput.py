"""Real emulator multitouch, CANCEL, focus and picker cancellation; read-only telemetry assertions."""
import base64
import argparse
import importlib.util
import json
import math
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT/'analysis/milestone0/input'
OUT.mkdir(parents=True,exist_ok=True)
spec=importlib.util.spec_from_file_location('play_ui',ROOT/'scripts/Test-PlayableEmulator.py')
ui=importlib.util.module_from_spec(spec)
spec.loader.exec_module(ui)
ui.OUT=OUT


def frames():
    raw=ui.adb('logcat','-d','-s','RCPerf:I','*:S')
    result=[]
    for line in raw.splitlines():
        offset=line.find('{')
        if offset>=0:
            value=json.loads(line[offset:])
            if value.get('type')=='frame': result.append(value)
    return result


def wait_frame(predicate,seconds=8,historical=False):
    deadline=time.monotonic()+seconds
    while time.monotonic()<deadline:
        values=frames()
        for value in (values if historical else values[-1:]):
            if predicate(value): return value
        time.sleep(0.1)
    raise AssertionError('Expected frame telemetry did not arrive')


def probe(geometry,mode):
    encoded=base64.b64encode(json.dumps(geometry).encode()).decode()
    result=ui.adb('shell','am','instrument','-w','-r','-e','geometry',encoded,'-e','mode',mode,
                  'org.rcport.inputprobe/.InputProbe')
    (OUT/f'{mode}-instrumentation.txt').write_text(result,encoding='utf-8')
    expected={'focus':'HOME_DURING_HELD_INPUT','release':'FINAL_LOOK_RELEASE_INJECTED','cancel':'MULTITOUCH_AND_CANCEL_INJECTED'}[mode]
    assert expected in result and 'INSTRUMENTATION_CODE: -1' in result,result


def main():
    global OUT
    parser=argparse.ArgumentParser()
    parser.add_argument('--output',type=Path)
    parser.add_argument('--probe',type=Path,default=ROOT/'analysis/milestone0/input-probe/input-probe.apk')
    parser.add_argument('--final-look-release',action='store_true')
    args=parser.parse_args()
    if args.output:
        OUT=args.output.resolve();OUT.mkdir(parents=True,exist_ok=True);ui.OUT=OUT
    (OUT/'validation.json').unlink(missing_ok=True)
    ui.adb('install','-r',args.probe)
    tree=ui.ui()
    control=next(n for n in tree.iter('node') if n.attrib.get('content-desc','').startswith('Links Joystick'))
    left,top,right,bottom=ui.bounds(control)
    geometry=dict(left=left,top=top,width=right-left,height=bottom-top)
    if args.final_look_release:
        ui.tap_node(ui.find(tree,'NEUSTART'))
        before=wait_frame(lambda f:f['phase']==0 and f['ticks']<30)
        probe(geometry,'release');time.sleep(.3)
        after=frames()[-1]
        assert abs((after['pose'][3]-before['pose'][3])+.6)<.002,(before['pose'],after['pose'])
        assert abs((after['pose'][4]-before['pose'][4])+.2)<.002,(before['pose'],after['pose'])
        assert after['buttons']==0 and after['forward']==0 and after['strafe']==0
    ui.adb('logcat','-c')
    ui.tap_node(ui.find(tree,'NEUSTART'))
    before=wait_frame(lambda f:f['phase']==0 and f['ticks']<30)
    probe(geometry,'cancel')
    time.sleep(0.5)
    values=frames()
    active=[f for f in values if f['forward']>0.3 and abs(f['lookX'])>1e-6 and f['buttons']&2]
    assert active,'Move/look/fire did not reach the same Rust frame'
    assert any(f['buttons']&3==3 for f in values),'Jump and fire not combined'
    assert max(f['shots'] for f in values)>before['shots']
    assert max(f['jumps'] for f in values)>before['jumps']
    assert max(math.dist(f['pose'][:3],before['pose'][:3]) for f in values)>10
    assert max(abs(f['pose'][3]-before['pose'][3]) for f in values)>0.02
    released=wait_frame(lambda f:f['buttons']==0 and f['forward']==0 and f['strafe']==0 and f['lookX']==0)
    time.sleep(0.4)
    after=frames()[-1]
    assert after['shots']==released['shots'],'Fire stuck after ACTION_CANCEL'
    ui.capture('after-multitouch-cancel')
    ui.tap_node(ui.find(ui.ui(),'NEUSTART'))
    wait_frame(lambda f:f['phase']==0 and f['ticks']<30)
    probe(geometry,'focus')
    time.sleep(0.5)
    suspended=frames()
    time.sleep(0.6)
    assert len(frames())==len(suspended),'Background simulation continued'
    ui.adb('shell','am','start','-W','-a','android.intent.action.MAIN','-c','android.intent.category.LAUNCHER',
           '-f','0x10200000','-n','org.rcport.diagnostic/.MainActivity')
    resumed=wait_frame(lambda f:f['sequence']>suspended[-1]['sequence'])
    new=[f for f in frames() if f['sequence']>suspended[-1]['sequence']]
    assert new[0]['still'] and new[0]['pose']==suspended[-1]['pose'],'Resume changed frozen state'
    assert all(f['buttons']==0 and f['forward']==0 and f['lookX']==0 for f in new),'Held inputs survived focus loss'
    ui.capture('resumed')
    old_sequence=resumed['sequence']
    ui.tap_node(ui.find(ui.ui(),'SZENE ÖFFNEN'))
    time.sleep(0.4)
    ui.adb('shell','input','keyevent','KEYCODE_BACK')
    wait_frame(lambda f:f['sequence']>old_sequence and f['still'],historical=True)
    ui.capture('picker-cancelled')
    raw=ui.adb('logcat','-d','-s','RCPerf:I','*:S')
    (OUT/'performance.log').write_text(raw,encoding='utf-8')
    crash=ui.adb('logcat','-d','-b','crash')
    assert 'FATAL EXCEPTION' not in crash and 'Fatal signal' not in crash
    result=dict(final_look_release_exact=args.final_look_release,android_api=35,abi='x86_64',four_pointer_ids=[2,7,19,23],
        simultaneous_move_look_fire=True,simultaneous_jump_fire=True,
        observed_position_yaw_shots_and_jumps_change=True,action_cancel_clears_inputs=True,
        focus_loss_stops_simulation=True,resume_preserves_state_and_clears_inputs=True,
        picker_cancel_recovers=True,no_crash=True,
        scope='Separate instrumentation APK injects Android MotionEvents/KEYCODE_HOME through UiAutomation. Game state is observed, never assigned. Physical touch hardware remains unverified.')
    (OUT/'validation.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    print(json.dumps(result))


if __name__=='__main__':main()
