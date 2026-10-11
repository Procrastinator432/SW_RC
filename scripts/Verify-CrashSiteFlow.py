"""Independent projection of original script properties and RIFF timing onto flow traces."""
import io
import json
import wave
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission2'
game = Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData')
objects = json.loads((ROOT / 'analysis/mission1/geo_01a-original.json').read_text())['objects']
by = {o['index']: o for o in objects}

def props(obj):
    return {p['name']: p['value']['value'] for p in obj['properties']['values']}

assets = json.loads((OUT / 'crash-assets.json').read_text())
traces = json.loads((OUT / 'check/timer-traces.json').read_text())
expected = [{'Event': 'BactaUsed'}]
tag_ids = {props(o).get('Tag'): o['index'] for o in objects if 'Tag' in props(o)}
for index, name in by[739]['arrays']['Actions']:
    if index > 2911:
        break
    action = by[index]
    cls = action['class'].split('.')[-1]
    p = props(action)
    if cls == 'ACTION_SetCollision':
        expected.append({'CollisionDisabled': tag_ids[p['CollideActorTag']]})
    elif cls == 'ACTION_ObjectiveUpdate':
        expected.append({'ObjectiveCompleted': p.get('ObjectiveNum', 0)})
    elif cls == 'ACTION_PlaySound':
        expected.append({'PlaySound': p['Sound']['path']})
    elif cls == 'ACTION_TriggerEvent':
        expected.append({'Event': p['Event']})
    elif cls == 'ACTION_MakeInvulnerable':
        expected.append({'Invulnerable': {'tag': p['Tag'], 'enabled': p.get('bMakeGod', True)}})
    elif cls == 'ACTION_HurtActor':
        expected.append({'HurtBarrier': {'original': tag_ids[p['TargetTag']], 'amount': int(p['amount'])}})
    elif cls == 'ACTION_ShowActorObjectiveTracker':
        expected.append({'Tracker': p['ActorTag']})
    else:
        assert cls in ['ACTION_WaitForEvent', 'ACTION_WaitForTimer'], cls
assert all(trace['effects'] == expected for trace in traces)
factory_ids = [o['index'] for o in objects if o['class'] == 'CTGame.PawnFactory'
               and props(o).get('Tag') == 'SpawnGunshipArea'
               and props(o).get('PawnTagFromFactory') == 'GunshipDroids']
assert factory_ids == [f['original'] for f in assets['factories']] == [133, 134, 277, 441, 836]
assert props(by[620])['NumToCount'] == assets['counter'] == len(factory_ids)
assert props(by[620])['Tag'] == 'DeadGunshipDroid'
assert props(by[620])['Event'] == 'CrashSiteSecured'
for index in factory_ids:
    assert props(by[index])['PawnEventFromFactory'] == 'DeadGunshipDroid'
completion = json.loads((OUT / 'check/completion-trace.json').read_text())
assert sorted(completion['order']) == sorted(factory_ids)
expected_completion = [{'Event': props(by[620])['Tag']}] * props(by[620])['NumToCount']
expected_completion += [{'Event': props(by[620])['Event']},
                       {'ObjectiveCompleted': props(by[1544])['ObjectiveNum']},
                       {'Tracker': props(by[1545])['ActorTag']},
                       {'CollisionDisabled': tag_ids[props(by[3293])['CollideActorTag']]}]
assert completion['effects'] == expected_completion
assert completion['state']['counter_remaining'] == 0
sound_report = ROOT / 'analysis/reports/packages/Sounds__GEO_Voice.uax.json'
exports = json.loads(sound_report.read_text())['exports']
e = next(o for o in exports if o['object_path'] == 'GEO_01.CA1G1791')
source = game / 'Sounds/GEO_Voice.uax'
with source.open('rb') as stream:
    stream.seek(e['serial_offset'])
    payload = stream.read(e['serial_size'])
with wave.open(io.BytesIO(payload[payload.index(b'RIFF'):]), 'rb') as wav:
    duration = wav.getnframes() / wav.getframerate()
assert abs(duration - assets['advisor_duration']) < 1e-12
total = props(by[2902])['PauseTime'] + duration + props(by[2908])['PauseTime']
assert all(abs(trace['delay'] - total) < 1e-12 for trace in traces)
receipt = dict(passed=True, original_script_effect_order=True, independent_pcm_duration=duration,
               original_completion_effect_order=True,
               original_factory_ids=factory_ids, original_counter=5,
               inputs=['analysis/mission1/geo_01a-original.json',
                       'analysis/reports/packages/Sounds__GEO_Voice.uax.json'],
               scope='Independent property projection and Python WAV timing; no native AI or physical scene acceptance.')
(OUT / 'original-crosscheck.json').write_bytes((json.dumps(receipt, indent=2) + '\n').encode())
print('Original script effect order, five factories, counter and independent PCM duration verified')
