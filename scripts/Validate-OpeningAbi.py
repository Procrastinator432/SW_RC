"""Replay normal inputs through the Windows native ABI; no state assignment."""
import ctypes as c
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission12-opening'
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
api.rc_game_squad_status.argtypes=[c.c_uint64,c.POINTER(c.c_uint32),c.c_size_t]
scene=(OUT/'geo_01a-opening.rcplay').read_bytes();payload=c.create_string_buffer(scene);error=c.create_string_buffer(512)
runs=[]
for name,phase in [('playthrough/input-replay.bin',1),('live/squad-assist-input.bin',0)]:
 stream=(OUT/name).read_bytes();assert stream[:8]==b'RCINPUT1';count,=struct.unpack_from('<I',stream,8);assert len(stream)==12+count*20
 handle=api.rc_game_create(payload,len(scene),error,512);assert handle,error.value
 words=(c.c_uint32*10)();sounds=[0]*76
 for i in range(count):
  f,s,x,y,flags=struct.unpack_from('<ffffI',stream,12+i*20);assert api.rc_game_advance(handle,1/60,f,s,x,y,flags)==1,i
  if i in [0,59,1396,1924]:
   assert api.rc_game_status(handle,words,10)==10 and words[0]==(200 if i<1396 else 25)
   if i<60:assert words[1]==20 and words[6]==0
  n=api.rc_game_audio(handle,1,None,0);assert n>=12;data=c.create_string_buffer(n+16);c.memset(data,0xa5,n+16);assert api.rc_game_audio(handle,1,data,n)==n;assert data.raw[n:]==b'\xa5'*16
  for at in range(12,n,24):
   role,=struct.unpack_from('<I',data.raw,at+16);assert role<76;sounds[role]+=1
 assert sounds[65:76]==[1]*11 and sounds[0]==1 and sounds[1]==1
 assert api.rc_game_status(handle,words,10)==10 and words[4]==phase and words[0]>0
 squad=(c.c_uint32*14)(*([0xa5a5a5a5]*14));assert api.rc_game_squad_status(handle,squad,11)==-1 and list(squad)==[0xa5a5a5a5]*14
 assert api.rc_game_squad_status(handle,squad,12)==12 and list(squad)[12:]==[0xa5a5a5a5]*2 and squad[2]==1
 if phase==0:assert squad[7]>0 and squad[8]>0 and squad[9]>0 and sounds[64]>0
 else:assert words[7]==8
 pixels=(c.c_uint32*(320*200+4))();pixels[-4:]=[0xa5a5a5a5]*4;assert api.rc_game_draw(handle,320,200,pixels,320*200)==320*200;assert list(pixels)[-4:]==[0xa5a5a5a5]*4
 (DEST/('complete.argb' if phase else 'assist.argb')).write_bytes(bytes(pixels)[:-16])
 runs.append(dict(name=name,inputs=count,status=list(words),squad=list(squad)[:12],sound_counts=sounds,input_sha256=hashlib.sha256(stream).hexdigest()))
 assert api.rc_game_command(handle,1)==0;assert api.rc_game_squad_status(handle,squad,12)==12 and squad[2]==0 and squad[7]==0
 api.rc_game_release(handle)
(DEST/'validation.json').write_text(json.dumps(dict(passed=True,runs=runs,opening_injury_health_checked=True,guarded_buffers=True,restart=True,scene_sha256=hashlib.sha256(scene).hexdigest()),indent=2))
print('Native mission and Scorch assist replays passed')
