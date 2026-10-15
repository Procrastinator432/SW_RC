"""Production Windows ABI: real map1 completion, travel, saved restore, map2 controls."""
import ctypes as c
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission18-map2-route/delivery/native'
OUT.mkdir(exist_ok=True)
api = c.CDLL(str(ROOT / 'target/release/rc_native.dll'))
api.rc_game_create.argtypes = [c.c_void_p, c.c_size_t, c.c_void_p, c.c_size_t]
api.rc_game_create.restype = c.c_uint64
api.rc_game_advance.argtypes = [c.c_uint64, c.c_double, c.c_float, c.c_float, c.c_float, c.c_float, c.c_uint32]
api.rc_game_travel.argtypes = api.rc_game_restore_handoff.argtypes = [c.c_uint64, c.c_void_p, c.c_size_t, c.c_void_p, c.c_size_t]
api.rc_game_status.argtypes = api.rc_game_squad_status.argtypes = [c.c_uint64, c.c_void_p, c.c_size_t]
api.rc_game_observe.argtypes = [c.c_uint64, c.c_void_p, c.c_size_t]
for name in ['rc_game_map_handoff', 'rc_game_mission_status']:
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
two = (OUT.parent / 'geo_01b.rcplay').read_bytes()
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
assert api.rc_game_command(h, 1) == 0 and status(h) == entry
api.rc_game_release(h)
(OUT / 'validation.json').write_text(json.dumps(dict(passed=True, real_map1_inputs=completed[5],
    early_wrong_and_corrupt_travel_atomic=True, saved_restore_and_graphics_cache=True,
    entry=entry, explored=explored, audio_events=sound_count, full_map2_inputs=expected["statistics"]["ticks"], all_map2_objectives=True, next_map_request="GEO_01C.ctm#playerstart",
    original_playerstart=True, companion_preserved=True, restart_preserves_carried_state=True), indent=2))
print('Windows ABI full GEO_01A→GEO_01B route, GEO_01C request, audio and restart passed')
