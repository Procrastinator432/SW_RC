"""Native mission load, buffer bounds, input-driven walking and Bacta completion."""
import ctypes as c, hashlib, json, math
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'analysis/mission1'
api=c.CDLL(str(ROOT/'target/release/rc_native.dll'))
api.rc_game_create.argtypes=[c.c_void_p,c.c_size_t,c.c_void_p,c.c_size_t];api.rc_game_create.restype=c.c_uint64
api.rc_game_advance.argtypes=[c.c_uint64,c.c_double,c.c_float,c.c_float,c.c_float,c.c_float,c.c_uint32];api.rc_game_advance.restype=c.c_int
api.rc_game_observe.argtypes=[c.c_uint64,c.POINTER(c.c_double),c.c_size_t];api.rc_game_observe.restype=c.c_int
api.rc_game_status.argtypes=[c.c_uint64,c.POINTER(c.c_uint32),c.c_size_t];api.rc_game_status.restype=c.c_int
api.rc_game_mission_status.argtypes=[c.c_uint64,c.c_void_p,c.c_size_t];api.rc_game_mission_status.restype=c.c_ssize_t
api.rc_game_audio.argtypes=[c.c_uint64,c.c_uint32,c.c_void_p,c.c_size_t];api.rc_game_audio.restype=c.c_ssize_t
api.rc_game_command.argtypes=[c.c_uint64,c.c_uint32];api.rc_game_command.restype=c.c_int
api.rc_game_release.argtypes=[c.c_uint64]
data=(OUT/'geo_01a-landing.rcplay').read_bytes();payload=c.create_string_buffer(data);error=c.create_string_buffer(512)
handle=api.rc_game_create(payload,len(data),error,512);assert handle,error.value
def status():
 words=(c.c_uint32*10)();assert api.rc_game_status(handle,words,10)==10;return list(words)
def pose():
 words=(c.c_double*6)();assert api.rc_game_observe(handle,words,6)==6;return list(words)[:5]
def guidance():
 n=api.rc_game_mission_status(handle,None,0);assert 0<n<2048
 b=c.create_string_buffer(n+16);c.memset(b,0xa5,n+16)
 assert api.rc_game_mission_status(handle,b,n-1)==-1 and b.raw==b'\xa5'*(n+16)
 assert api.rc_game_mission_status(handle,b,n)==n and b.raw[n:]==b'\xa5'*16
 return b.raw[:n].decode('utf-8')
def packet(kind):
 n=api.rc_game_audio(handle,kind,None,0);b=c.create_string_buffer(n);assert api.rc_game_audio(handle,kind,b,n)==n;return b.raw
try:
 assert status()[:5]==[25,20,200,0,0];initial=pose();guidance()
 assert api.rc_game_advance(handle,1/60,0,0,0,0,32)==-1
 assert api.rc_game_mission_status(0,None,0)==-1
 assert api.rc_game_mission_status(handle,None,1)==-1
 bank=packet(0);assert bank[:8]==b'RCSND001' and int.from_bytes(bank[8:12],'little')==3
 assets=json.loads((OUT/'mission-receipt.json').read_text())
 for target in assets['route']:
  for _ in range(1800):
   x,y,z,yaw,pitch=pose();distance=math.dist([x,y,z],target)
   if distance<25:break
   wanted=math.atan2(target[1]-y,target[0]-x);turn=(yaw-wanted+math.pi)%math.tau-math.pi
   assert api.rc_game_advance(handle,1/60,min(1,distance/90),0,turn,pitch,0)==1
  else:raise AssertionError('Native physical walk failed')
 x,y,z,yaw,pitch=pose();station=assets['station'];wanted=math.atan2(station[1]-y,station[0]-x)
 turn=(yaw-wanted+math.pi)%math.tau-math.pi
 assert api.rc_game_advance(handle,1/60,0,0,turn,pitch,16)==1
 for _ in range(300):
  api.rc_game_advance(handle,1/60,0,0,0,0,16)
  if status()[4]==1:break
 assert status()[0]==100 and status()[4]==1
 assert guidance().startswith('Originalziel abgeschlossen')
 n=api.rc_game_audio(handle,1,None,0);b=c.create_string_buffer(n+16);c.memset(b,0xa5,n+16)
 assert api.rc_game_audio(handle,1,b,n-1)==n and b.raw==b'\xa5'*(n+16)
 events=packet(1);assert int.from_bytes(events[8:12],'little')==3
 assert int.from_bytes(packet(1)[8:12],'little')==0
 assert api.rc_game_command(handle,1)==0 and status()[:5]==[25,20,200,0,0] and pose()==initial
 assert api.rc_game_command(handle,0)==0
finally:api.rc_game_release(handle)
assert api.rc_game_mission_status(handle,None,0)==-1
result=dict(passed=True,scene_sha256=hashlib.sha256(data).hexdigest(),short_buffers_and_canaries=True,invalid_flags_and_handles=True,physical_walk_and_completion=True,three_audio_events_consumed_once=True,restart_exact=True)
(OUT/'abi-validation.json').write_text(json.dumps(result,indent=2),encoding='utf-8');print(json.dumps(result))
