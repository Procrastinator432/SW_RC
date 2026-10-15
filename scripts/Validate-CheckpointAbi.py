"""Production Windows ABI: real map1 completion, travel, saved restore, map2 controls."""
import ctypes as c
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission29-checkpoints/delivery/native'
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


factory=(ROOT/'analysis/mission28-first-mission-complete/delivery/geo_03a.rcplay').read_bytes();scene=c.create_string_buffer(factory);error=c.create_string_buffer(512)
api.rc_game_checkpoint.argtypes=[c.c_uint64,c.c_void_p,c.c_size_t];api.rc_game_checkpoint.restype=c.c_ssize_t
api.rc_game_restore_checkpoint.argtypes=[c.c_uint64,c.c_void_p,c.c_size_t,c.c_void_p,c.c_size_t]
api.rc_game_checkpoint_marker.argtypes=[c.c_uint64];api.rc_game_checkpoint_marker.restype=c.c_uint64
h=api.rc_game_create(scene,len(factory),error,512);assert h,error.value
handoff=(ROOT/'analysis/mission28-first-mission-complete/delivery/native/map3-handoff.json').read_bytes();b=c.create_string_buffer(handoff);assert api.rc_game_restore_handoff(h,b,len(handoff),error,512)==0
entry=status(h);entry_frame=packet(api.rc_game_graphics,h,1)
controls=(ROOT/'analysis/mission28-first-mission-complete/delivery/playthrough/factory-input.bin').read_bytes();rows=list(struct.iter_unpack('<ffffI',controls[12:]))
for row in rows[:140]:assert api.rc_game_advance(h,1/60,*row)==1
saved=status(h);frame=packet(api.rc_game_graphics,h,1);checkpoint=packet(api.rc_game_checkpoint,h)
(OUT/'checkpoint.bin').write_bytes(checkpoint)
for row in rows[140:]:assert api.rc_game_advance(h,1/60,*row)==1
final=status(h);final_frame=packet(api.rc_game_graphics,h,1);api.rc_game_release(h)
h=api.rc_game_create(scene,len(factory),error,512);assert h
fresh=packet(api.rc_game_checkpoint,h);fresh_frame=packet(api.rc_game_graphics,h,1)
import copy
body=json.loads(checkpoint[20:]);bad=[]
for mutation in ['abi','health','clock','module','initial']:
 d=copy.deepcopy(body)
 if mutation=='abi':d['abi']='old-build'
 elif mutation=='health':d['state']['health']=99999
 elif mutation=='clock':d['accumulator']=100
 elif mutation=='module':d['state']['map']=None
 else:d['initial']['statistics']['ticks']=1
 payload=json.dumps(d,separators=(',',':')).encode();hash=0xcbf29ce484222325
 for x in payload:hash=((hash^x)*0x100000001b3)&0xffffffffffffffff
 bad.append(b'RCSAVE01'+struct.pack('<IQ',len(payload),hash)+payload)
bad += [checkpoint[:-1],checkpoint[:30]+bytes([checkpoint[30]^1])+checkpoint[31:],b'invalid']
for raw in bad:
 b=c.create_string_buffer(raw);assert api.rc_game_restore_checkpoint(h,b,len(raw),error,512)==-1
 assert packet(api.rc_game_checkpoint,h)==fresh and packet(api.rc_game_graphics,h,1)==fresh_frame
b=c.create_string_buffer(checkpoint);assert api.rc_game_restore_checkpoint(h,b,len(checkpoint),error,512)==0,error.value
assert status(h)==saved and packet(api.rc_game_graphics,h,1)==frame
marker=api.rc_game_checkpoint_marker(h)
for row in rows[140:]:assert api.rc_game_advance(h,1/60,*row)==1
assert status(h)==final and packet(api.rc_game_graphics,h,1)==final_frame
assert api.rc_game_checkpoint_marker(h)==marker
assert api.rc_game_command(h,1)==0 and status(h)==entry and packet(api.rc_game_graphics,h,1)==entry_frame
other=(ROOT/'analysis/mission18-map2-route/delivery/geo_01b.rcplay').read_bytes();b2=c.create_string_buffer(other);other_id=api.rc_game_create(b2,len(other),error,512);assert other_id
old=status(other_id);b=c.create_string_buffer(checkpoint);assert api.rc_game_restore_checkpoint(other_id,b,len(checkpoint),error,512)==-1 and status(other_id)==old
api.rc_game_release(other_id);api.rc_game_release(h)
(OUT/'validation.json').write_text(json.dumps(dict(passed=True,released_session_before_restore=True,checkpoint_bytes=len(checkpoint),exact_status_and_graphics=True,continuation_exact=True,carried_entry_restart_exact=True,invalid_checkpoints_atomic=len(bad),wrong_scene_rejected=True,marker_ignores_motion_and_resources=True,entry=entry,saved=saved,final=final),indent=2))
print('Windows ABI full checkpoint after handle release, exact continuation, entry restart and corrupt/wrong-scene rejection passed')
