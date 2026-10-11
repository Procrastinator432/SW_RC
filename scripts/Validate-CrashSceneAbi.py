"""Native mission load, buffer bounds, input-driven walking and Bacta completion."""
import argparse, ctypes as c, hashlib, json, math
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('--directory',type=Path,default=ROOT/'analysis/mission2-playable');OUT=p.parse_args().directory
api=c.CDLL(str(ROOT/'target/release/rc_native.dll'))
api.rc_game_create.argtypes=[c.c_void_p,c.c_size_t,c.c_void_p,c.c_size_t];api.rc_game_create.restype=c.c_uint64
api.rc_game_advance.argtypes=[c.c_uint64,c.c_double,c.c_float,c.c_float,c.c_float,c.c_float,c.c_uint32];api.rc_game_advance.restype=c.c_int
api.rc_game_observe.argtypes=[c.c_uint64,c.POINTER(c.c_double),c.c_size_t];api.rc_game_observe.restype=c.c_int
api.rc_game_status.argtypes=[c.c_uint64,c.POINTER(c.c_uint32),c.c_size_t];api.rc_game_status.restype=c.c_int
api.rc_game_mission_status.argtypes=[c.c_uint64,c.c_void_p,c.c_size_t];api.rc_game_mission_status.restype=c.c_ssize_t
api.rc_game_audio.argtypes=[c.c_uint64,c.c_uint32,c.c_void_p,c.c_size_t];api.rc_game_audio.restype=c.c_ssize_t
api.rc_game_command.argtypes=[c.c_uint64,c.c_uint32];api.rc_game_command.restype=c.c_int
api.rc_game_release.argtypes=[c.c_uint64]
api.rc_game_actor_status.argtypes=[c.c_uint64,c.c_void_p,c.c_size_t];api.rc_game_actor_status.restype=c.c_ssize_t
data=(OUT/'geo_01a-crash.rcplay').read_bytes();payload=c.create_string_buffer(data);error=c.create_string_buffer(512)
handle=api.rc_game_create(payload,len(data),error,512);assert handle,error.value
assets=json.loads((OUT/'mission-receipt.json').read_text())
def status():
 words=(c.c_uint32*10)();assert api.rc_game_status(handle,words,10)==10;return list(words)
def pose():
 words=(c.c_double*6)();assert api.rc_game_observe(handle,words,6)==6;return list(words)[:5]
def observe():
 n=api.rc_game_actor_status(handle,None,0);assert 0<n<8192
 b=c.create_string_buffer(n+16);c.memset(b,0xa5,n+16)
 assert api.rc_game_actor_status(handle,b,n-1)==-1 and b.raw==b'\xa5'*(n+16)
 assert api.rc_game_actor_status(handle,b,n)==n and b.raw[n:]==b'\xa5'*16
 return json.loads(b.raw[:n])
def advance(flags=0,forward=0,turn=0,pitch=0):
 assert api.rc_game_advance(handle,1/60,forward,0,turn,pitch,flags)==1
 return status()
def aim(target):
 x,y,z,yaw,pitch=pose();eye=z+84*.65
 wanted=math.atan2(target[1]-y,target[0]-x);wanted_pitch=math.atan2(target[2]-eye,math.hypot(target[0]-x,target[1]-y))
 return (yaw-wanted+math.pi)%math.tau-math.pi,pitch-wanted_pitch
def walk(route):
 for target in route:
  for _ in range(1800):
   x,y,z,yaw,pitch=pose();distance=math.dist([x,y,z],target)
   if distance<25:break
   wanted=math.atan2(target[1]-y,target[0]-x);turn=(yaw-wanted+math.pi)%math.tau-math.pi
   advance(forward=min(1,distance/90),turn=turn,pitch=pitch)
   assert status()[4]==0
  else:raise AssertionError(('Native walking stuck',target,pose()))
try:
 initial=pose();assert status()[:5]==[25,20,200,0,0] and observe()==[]
 assert api.rc_game_actor_status(0,None,0)==-1
 assert api.rc_game_actor_status(handle,None,1)==-1
 assert api.rc_game_advance(handle,1/60,0,0,0,0,32)==-1
 walk(assets['route']);turn,pitch=aim(assets['station']);advance(16,turn=turn,pitch=pitch)
 for _ in range(240):advance(16)
 assert status()[0]==100 and status()[4]==0
 for _ in range(400):advance()
 walk(assets['crash']['route']);actors=observe();assert len(actors)==5
 for _ in range(1800):
  if status()[4]!=0:break
  actors=observe();alive=[a for a in actors if a['health']>0 and a['visible']]
  if not alive:advance();continue
  target=min(alive,key=lambda a:math.dist(a['position'],pose()[:3]))['position'];target=target.copy();target[2]+=15
  turn,pitch=aim(target);advance(4 if status()[1]==0 else 2,turn=turn,pitch=pitch)
 assert status()[4]==1 and status()[7]==5,(status(),observe())
 assert all(a['health']==0 for a in observe())
 n=api.rc_game_audio(handle,1,None,0);b=c.create_string_buffer(n+16);c.memset(b,0xa5,n+16)
 assert api.rc_game_audio(handle,1,b,n-1)==n and b.raw==b'\xa5'*(n+16)
 assert api.rc_game_audio(handle,1,b,n)==n and b.raw[n:]==b'\xa5'*16
 assert int.from_bytes(b.raw[8:12],'little')==4+(status()[6] if 'weapon_fire_sound' in assets else 0)
 events=[int.from_bytes(b.raw[i+16:i+20],'little') for i in range(12,n,24)]
 if 'weapon_fire_sound' in assets:assert events.count(assets['weapon_fire_sound'])==status()[6]
 n=api.rc_game_audio(handle,1,None,0);b=c.create_string_buffer(n);api.rc_game_audio(handle,1,b,n)
 assert int.from_bytes(b.raw[8:12],'little')==0
 assert api.rc_game_command(handle,1)==0 and status()[:5]==[25,20,200,0,0] and pose()==initial and observe()==[]
finally:api.rc_game_release(handle)
assert api.rc_game_actor_status(handle,None,0)==-1
result=dict(passed=True,scene_sha256=hashlib.sha256(data).hexdigest(),observer_short_buffers_and_canaries=True,invalid_handles_and_flags=True,physical_walk_both_objectives=True,five_factory_actors_and_kills=True,four_audio_events_consumed_once=True,weapon_fire_events_match_shots=('weapon_fire_sound' in assets),restart_exact=True,scope='Native input-driven selected mission objectives with provisional combat rules; no native AI parity.')
(OUT/'abi-validation.json').write_text(json.dumps(result,indent=2),encoding='utf-8');print(json.dumps(result))
