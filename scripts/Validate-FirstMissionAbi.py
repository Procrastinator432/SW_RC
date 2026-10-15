"""Production Windows ABI: real map1 completion, travel, saved restore, map2 controls."""
import ctypes as c
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission28-first-mission-complete/delivery/native'
OUT.mkdir(exist_ok=True)
api = c.CDLL(str(ROOT / 'target/release/rc_native.dll'))
api.rc_game_create.argtypes = [c.c_void_p, c.c_size_t, c.c_void_p, c.c_size_t]
api.rc_game_create.restype = c.c_uint64
api.rc_game_advance.argtypes = [c.c_uint64, c.c_double, c.c_float, c.c_float, c.c_float, c.c_float, c.c_uint32]
api.rc_game_travel.argtypes = api.rc_game_restore_handoff.argtypes = [c.c_uint64, c.c_void_p, c.c_size_t, c.c_void_p, c.c_size_t]
api.rc_game_status.argtypes = api.rc_game_squad_status.argtypes = [c.c_uint64, c.c_void_p, c.c_size_t]
api.rc_game_observe.argtypes = [c.c_uint64, c.c_void_p, c.c_size_t]
for name in ['rc_game_map_handoff', 'rc_game_mission_status','rc_game_actor_status']:
    f = getattr(api, name); f.argtypes = [c.c_uint64, c.c_void_p, c.c_size_t]; f.restype = c.c_ssize_t
api.rc_game_audio.argtypes = api.rc_game_graphics.argtypes = [c.c_uint64, c.c_uint32, c.c_void_p, c.c_size_t]
api.rc_game_audio.restype = api.rc_game_graphics.restype = c.c_ssize_t
api.rc_game_command.argtypes = [c.c_uint64, c.c_uint32]
api.rc_game_release.argtypes = [c.c_uint64]


def status(h):
    words = (c.c_uint32 * 10)(); assert api.rc_game_status(h, words, 10) == 10
    return list(words)


def squad(h):
    words = (c.c_uint32 * 12)(); assert api.rc_game_squad_status(h, words, 12) == 12
    return list(words)


def packet(f, h, *args):
    n = f(h, *args, None, 0); assert n >= 0
    b = c.create_string_buffer(n + 16); c.memset(b, 0xa5, n + 16)
    assert f(h, *args, b, n) == n and b.raw[n:] == b'\xa5' * 16
    return b.raw[:n]


one = (ROOT / 'analysis/mission15-complete/delivery/geo_01a-first-mission.rcplay').read_bytes()
two = (ROOT / 'analysis/mission18-map2-route/delivery/geo_01b.rcplay').read_bytes()
b1, b2, error = c.create_string_buffer(one), c.create_string_buffer(two), c.create_string_buffer(512)
h = api.rc_game_create(b1, len(one), error, 512); assert h, error.value
initial = status(h)
assert api.rc_game_travel(h, b2, len(two), error, 512) == -1 and status(h) == initial
rows = (ROOT / 'analysis/mission15-complete/delivery/playthrough/input-replay.bin').read_bytes()
for row in struct.iter_unpack('<ffffI', rows[12:]):
    assert api.rc_game_advance(h, 1/60, *row) == 1
completed = status(h); assert completed[4] == 1 and completed[0] == 20
handoff = packet(api.rc_game_map_handoff, h)
(OUT / 'map1-handoff.json').write_bytes(handoff)
for payload in [one, b'invalid']:
    b = c.create_string_buffer(payload)
    assert api.rc_game_travel(h, b, len(payload), error, 512) == -1 and status(h) == completed
assert api.rc_game_travel(h, b2, len(two), error, 512) == 0, error.value
entry = status(h); assert entry[:3] == completed[:3] and entry[4:6] == [0, 0]
carry=json.loads(handoff)["squad"]
assert squad(h)[:4] == [carry["health"],__import__("math").ceil(carry["shields"]),int(carry["joined"]),int(carry["down"])]
assert b'GEO_01B' in packet(api.rc_game_mission_status, h)
assert api.rc_game_map_handoff(h, None, 0) == 0
bank = packet(api.rc_game_audio, h, 0); assert bank[:8] == b'RCSND001' and struct.unpack_from('<I', bank, 8)[0] == 32
fresh = api.rc_game_create(b2, len(two), error, 512); assert fresh
old = packet(api.rc_game_graphics, fresh, 1)
invalid = json.loads(handoff); invalid['player']['ammo'] = 999
invalid = json.dumps(invalid).encode(); b = c.create_string_buffer(invalid)
assert api.rc_game_restore_handoff(fresh, b, len(invalid), error, 512) == -1
assert packet(api.rc_game_graphics, fresh, 1) == old
b = c.create_string_buffer(handoff)
assert api.rc_game_restore_handoff(fresh, b, len(handoff), error, 512) == 0, error.value
assert status(fresh) == entry and squad(fresh) == squad(h)
assert packet(api.rc_game_graphics, fresh, 1) != old
api.rc_game_release(fresh)
sound_count = 0
map2_rows=(ROOT/'analysis/mission18-map2-route/input-replay.bin').read_bytes()
assert map2_rows[:8]==b'RCINPUT1'
for index,row in enumerate(struct.iter_unpack('<ffffI',map2_rows[12:])):
    assert api.rc_game_advance(h,1/60,*row)==1,index
    events=packet(api.rc_game_audio,h,1);sound_count+=struct.unpack_from('<I',events,8)[0]
explored=status(h)
expected=json.loads((ROOT/'analysis/mission18-map2-route/final-state.json').read_text())
assert explored==[expected['health'],expected['ammo'],expected['reserve'],sum(e['health']>0 for e in expected['enemies']),1,
 expected['statistics']['ticks'],expected['statistics']['shots'],expected['statistics']['kills'],expected['statistics']['jumps'],expected['statistics']['enemy_shots']]
assert b'GEO_01C' in packet(api.rc_game_mission_status,h)
pose=(c.c_double*6)();assert api.rc_game_observe(h,pose,6)==6
assert max(abs(pose[i]-expected['pawn']['body']['position'][i]) for i in range(3))<.001
end_frame=packet(api.rc_game_graphics,h,1);assert len(end_frame)>12 and sound_count>0
handoff2=packet(api.rc_game_map_handoff,h)
assert handoff2==packet(api.rc_game_map_handoff,h)
receipt=json.loads(handoff2);assert receipt['schema']==2 and receipt['source']=='GEO_01B.ctm' and receipt['map2']['objectives']==[True]*8
(OUT/'map2-handoff.json').write_bytes(handoff2)
three=(OUT.parent/'geo_01c.rcplay').read_bytes();b3=c.create_string_buffer(three)
for payload in [one,two,b'invalid']:
 b=c.create_string_buffer(payload);assert api.rc_game_travel(h,b,len(payload),error,512)==-1 and status(h)==explored
 assert packet(api.rc_game_graphics,h,1)==end_frame
assert api.rc_game_travel(h,b3,len(three),error,512)==0,error.value
entry3=status(h);assert entry3==[10,20,141,0,0,0,0,0,0,0]
assert b'GEO_01C' in packet(api.rc_game_mission_status,h)
assert api.rc_game_map_handoff(h,None,0)==0
api.rc_game_observe(h,pose,6);assert list(pose)[:3]==[-20039.7890625,12575.505859375,1144.]
actors=json.loads(packet(api.rc_game_actor_status,h));assert [a['role'] for a in actors]==['Scorch','Fixer','Sev'] and all(a['health']==100 for a in actors)
assert packet(api.rc_game_graphics,h,1)!=end_frame
entry_frame=packet(api.rc_game_graphics,h,1)
fresh=api.rc_game_create(b3,len(three),error,512);assert fresh
old=packet(api.rc_game_graphics,fresh,1)
bad=json.loads(handoff2);bad['map2']['fixer']['health']=9999
raw=json.dumps(bad).encode();b=c.create_string_buffer(raw);assert api.rc_game_restore_handoff(fresh,b,len(raw),error,512)==-1
assert packet(api.rc_game_graphics,fresh,1)==old
b=c.create_string_buffer(handoff2);assert api.rc_game_restore_handoff(fresh,b,len(handoff2),error,512)==0,error.value
assert packet(api.rc_game_graphics,fresh,1)==entry_frame and status(fresh)==entry3
assert api.rc_game_restore_handoff(fresh,b,len(handoff2),error,512)==-1
api.rc_game_release(fresh)
map3_rows=(ROOT/'analysis/mission27-map3-elevator/delivery/playthrough/input-replay.bin').read_bytes();map3_audio=0
for row in struct.iter_unpack('<ffffI',map3_rows[12:]):
 assert api.rc_game_advance(h,1/60,*row)==1
 map3_audio+=struct.unpack_from('<I',packet(api.rc_game_audio,h,1),8)[0]
result=status(h);reference=json.loads((ROOT/'analysis/mission27-map3-elevator/delivery/playthrough/validation.json').read_text())['final'];stats=reference['statistics']
assert result==[reference['health'],reference['ammo'],reference['reserve'],sum(e['health']>0 for e in reference['enemies']),1,stats['ticks'],stats['shots'],stats['kills'],stats['jumps'],stats['enemy_shots']]
assert result[0]>0 and map3_audio>0
actors=json.loads(packet(api.rc_game_actor_status,h));assert actors[-1]['role']=='Sev' and actors[-1]['joined']
assert b'Mission abgeschlossen' in packet(api.rc_game_mission_status,h)
receipt3=json.loads(packet(api.rc_game_map_handoff,h));assert receipt3['schema']==3 and receipt3['trigger']==39 and receipt3['map3']['objectives']==[True]*7
assert all(receipt3['map3'][name]['joined'] for name in ['fixer','sev']) and receipt3['squad']['joined']
(OUT/'map3-handoff.json').write_text(json.dumps(receipt3,indent=2))
c_result=result
factory=(OUT.parent/'geo_03a.rcplay').read_bytes();bf=c.create_string_buffer(factory)
completed_frame=packet(api.rc_game_graphics,h,1)
for payload in [one,two,three,b'invalid']:
 b=c.create_string_buffer(payload);assert api.rc_game_travel(h,b,len(payload),error,512)==-1
 assert status(h)==c_result and packet(api.rc_game_graphics,h,1)==completed_frame
assert api.rc_game_travel(h,bf,len(factory),error,512)==0,error.value
factory_entry=status(h);assert factory_entry==[65,14,65,0,0,0,0,0,0,0]
assert b'GEO_03A' in packet(api.rc_game_mission_status,h)
api.rc_game_observe(h,pose,6);assert list(pose)[:3]==[-2555.739501953125,2654.17138671875,6920.81640625]
actors=json.loads(packet(api.rc_game_actor_status,h));assert [v['role'] for v in actors]==['Scorch','Fixer','Sev'] and all(v['health']==100 for v in actors)
assert api.rc_game_map_handoff(h,None,0)==0
factory_frame=packet(api.rc_game_graphics,h,1);assert factory_frame!=completed_frame
fresh=api.rc_game_create(bf,len(factory),error,512);assert fresh
assert b'Eigenstaendiger Start' in packet(api.rc_game_mission_status,fresh) and b'Mission 1 abgeschlossen' not in packet(api.rc_game_mission_status,fresh)
before=status(fresh);before_frame=packet(api.rc_game_graphics,fresh,1)
bad=json.loads(json.dumps(receipt3));bad['map3']['sev']['health']=9999
raw=json.dumps(bad).encode();b=c.create_string_buffer(raw)
assert api.rc_game_restore_handoff(fresh,b,len(raw),error,512)==-1 and status(fresh)==before and packet(api.rc_game_graphics,fresh,1)==before_frame
raw=json.dumps(receipt3).encode();b=c.create_string_buffer(raw)
assert api.rc_game_restore_handoff(fresh,b,len(raw),error,512)==0,error.value
assert status(fresh)==factory_entry and packet(api.rc_game_graphics,fresh,1)==factory_frame
assert api.rc_game_restore_handoff(fresh,b,len(raw),error,512)==-1
api.rc_game_release(fresh)
controls=(OUT.parent/'playthrough/factory-input.bin').read_bytes();audio=0
for row in struct.iter_unpack('<ffffI',controls[12:]):
 assert api.rc_game_advance(h,1/60,*row)==1
 audio+=struct.unpack_from('<I',packet(api.rc_game_audio,h,1),8)[0]
final=status(h);reference=json.loads((OUT.parent/'playthrough/validation.json').read_text())['final'];stats=reference['statistics']
assert final==[reference['health'],reference['ammo'],reference['reserve'],0,0,*[stats[k] for k in ['ticks','shots','kills','jumps','enemy_shots']]] and audio==2
assert api.rc_game_command(h,1)==0 and status(h)==factory_entry and packet(api.rc_game_graphics,h,1)==factory_frame
api.rc_game_release(h)
(OUT/'validation.json').write_text(json.dumps(dict(passed=True,real_map1_inputs=18730,full_map2_inputs=16414,map3_inputs=11800,factory_inputs=360,
 actual_factory_travel=True,original_playerstart=True,three_members_carried=True,wrong_travel_atomic=True,invalid_sev_saved_carry_atomic=True,saved_restore_exact=True,restart_exact_status_and_frame=True,c_complete=c_result,entry=factory_entry,final=final,factory_audio=audio),indent=2))
print('Windows ABI complete A/B/C → GEO_03A, three members, atomic failures, save/resume, controls and restart passed')
