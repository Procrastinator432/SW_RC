"""Replay normal inputs through the Windows native ABI; no state assignment."""
import ctypes as c
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission9-ai'
DEST = OUT / 'native'
DEST.mkdir(exist_ok=True)
api = c.CDLL(str(ROOT / 'target/release/rc_native.dll'))
api.rc_game_create.argtypes = [c.c_void_p,c.c_size_t,c.c_void_p,c.c_size_t]
api.rc_game_create.restype = c.c_uint64
api.rc_game_advance.argtypes = [c.c_uint64,c.c_double,c.c_float,c.c_float,c.c_float,c.c_float,c.c_uint32]
api.rc_game_advance.restype = c.c_int
api.rc_game_status.argtypes = [c.c_uint64,c.POINTER(c.c_uint32),c.c_size_t]
api.rc_game_audio.argtypes = [c.c_uint64,c.c_uint32,c.c_void_p,c.c_size_t]
api.rc_game_audio.restype = c.c_ssize_t
api.rc_game_mission_status.argtypes = [c.c_uint64,c.c_void_p,c.c_size_t]
api.rc_game_mission_status.restype = c.c_ssize_t
api.rc_game_draw.argtypes = [c.c_uint64,c.c_size_t,c.c_size_t,c.c_void_p,c.c_size_t]
api.rc_game_draw.restype = c.c_ssize_t
api.rc_game_command.argtypes = [c.c_uint64,c.c_uint32]
api.rc_game_release.argtypes = [c.c_uint64]
scene = (OUT/'geo_01a-warrior-ai.rcplay').read_bytes()
payload = c.create_string_buffer(scene)
error = c.create_string_buffer(512)
handle = api.rc_game_create(payload,len(scene),error,512)
assert handle,error.value
stream = (OUT/'playthrough/input-replay.bin').read_bytes()
assert stream[:8] == b'RCINPUT1'
count, = struct.unpack_from('<I',stream,8)
assert len(stream) == 12+20*count
words = (c.c_uint32*10)()
sounds = [0]*64
for i in range(count):
    f,s,x,y,flags = struct.unpack_from('<ffffI',stream,12+20*i)
    assert api.rc_game_advance(handle,1/60,f,s,x,y,flags) == 1,i
    assert api.rc_game_status(handle,words,10) == 10
    assert words[4] != 2
    n = api.rc_game_audio(handle,1,None,0)
    assert n >= 12
    data = c.create_string_buffer(n+16)
    c.memset(data,0xa5,n+16)
    assert api.rc_game_audio(handle,1,data,n) == n
    assert data.raw[n:] == b'\xa5'*16
    for at in range(12,n,24):
        role, = struct.unpack_from('<I',data.raw,at+16)
        assert role < len(sounds)
        sounds[role] += 1
assert words[4] == 1 and words[0] > 0 and words[7] == 8
assert sounds[20:33] == [1]*13
assert all(sounds[role]==1 for role in range(33,62) if role!=38)
assert sounds[38]==20
assert sounds[62:64]==[4,4]
n = api.rc_game_mission_status(handle,None,0)
text = c.create_string_buffer(n)
assert api.rc_game_mission_status(handle,text,n) == n
text = text.raw.decode('utf-8')
assert 'Turmeingang' in text
pixels = (c.c_uint32*(320*200+4))()
for i in range(320*200,len(pixels)): pixels[i]=0xa5a5a5a5
assert api.rc_game_draw(handle,320,200,pixels,320*200) == 320*200
assert list(pixels)[-4:] == [0xa5a5a5a5]*4
(DEST/'complete.argb').write_bytes(bytes(pixels)[:-16])
final = list(words)
assert api.rc_game_command(handle,1) == 0
assert api.rc_game_status(handle,words,10) == 10
assert words[0] == 25 and words[4] == 0 and words[7] == 0
api.rc_game_release(handle)
hazard=(OUT/'playthrough/danger-input-replay.bin').read_bytes()
hazard_count,=struct.unpack_from('<I',hazard,8)
handle=api.rc_game_create(payload,len(scene),error,512)
assert handle,error.value
for i in range(hazard_count):
    f,s,x,y,flags=struct.unpack_from('<ffffI',hazard,12+20*i)
    assert api.rc_game_advance(handle,1/60,f,s,x,y,flags)==1
assert api.rc_game_status(handle,words,10)==10 and words[4]==2 and words[0]==0
api.rc_game_release(handle)
behavior={}
for name,phase in [('ai-defeat-input.bin',2),('ai-dodge-input.bin',0),('ai-interrupt-input.bin',0)]:
    data=(OUT/'behavior'/name).read_bytes()
    assert data[:8]==b'RCINPUT1'
    steps,=struct.unpack_from('<I',data,8)
    assert len(data)==12+20*steps
    handle=api.rc_game_create(payload,len(scene),error,512)
    assert handle,error.value
    for i in range(steps):
        f,s,x,y,flags=struct.unpack_from('<ffffI',data,12+20*i)
        assert api.rc_game_advance(handle,1/60,f,s,x,y,flags)==1,i
    assert api.rc_game_status(handle,words,10)==10 and words[4]==phase
    assert words[0]==(0 if phase==2 else 20)
    behavior[name]=dict(passed=True,inputs=steps,status=list(words),sha256=hashlib.sha256(data).hexdigest())
    api.rc_game_release(handle)
result = dict(ai_behavior=behavior,bombardment_defeat_replay=True,passed=True,inputs=count,status=final,sound_counts=sounds,mission_text=text,restart=True,
              scene_sha256=hashlib.sha256(scene).hexdigest(),input_sha256=hashlib.sha256(stream).hexdigest(),
              scope='Windows native ABI replay, normal inputs, guarded buffers and CPU frame; Android checked separately')
(DEST/'validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
print(f'Native ABI: objectives0–6 completed with {count} inputs, original encounter and artillery audio, restart passed')
