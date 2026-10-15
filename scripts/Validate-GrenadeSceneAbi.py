"""Native mission load, buffer bounds, input-driven walking and Bacta completion."""
import argparse, ctypes as c, hashlib, json, math, struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('--directory',type=Path,default=ROOT/'analysis/mission3-playable')
p.add_argument('--scene',type=Path);p.add_argument('--assets',type=Path)
args=p.parse_args();OUT=args.directory;OUT.mkdir(parents=True,exist_ok=True)
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
data=(args.scene or OUT/'geo_01a-grenades.rcplay').read_bytes();payload=c.create_string_buffer(data);error=c.create_string_buffer(512)
gunner=data.startswith(b'RCPLAY06')
base=data[12+struct.unpack_from('<I',data,8)[0]:] if gunner else data
thermal=json.loads(base[12:12+struct.unpack_from('<I',base,8)[0]]) if base.startswith(b'RCPLAY05') else None
handle=api.rc_game_create(payload,len(data),error,512);assert handle,error.value
assets=json.loads((args.assets or OUT/'mission-receipt.json').read_text())
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
replay=[]
def advance(flags=0,forward=0,turn=0,pitch=0):
 replay.append((forward,0,turn,pitch,flags))
 assert api.rc_game_advance(handle,1/60,forward,0,turn,pitch,flags)==1
 return status()

def mission_text():
 n=api.rc_game_mission_status(handle,None,0);text=c.create_string_buffer(n);assert api.rc_game_mission_status(handle,text,n)==n
 return text.raw.decode('utf-8')
def aim(target):
 x,y,z,yaw,pitch=pose();eye=z+84*.65
 wanted=math.atan2(target[1]-y,target[0]-x);wanted_pitch=math.atan2(target[2]-eye,math.hypot(target[0]-x,target[1]-y))
 return (yaw-wanted+math.pi)%math.tau-math.pi,pitch-wanted_pitch
def walk(route,gate=None):
 for index,target in enumerate(route):
  if gate is not None and index==gate+1:
   for _ in range(900):advance()
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
 assert api.rc_game_advance(handle,1/60,0,0,0,0,64)==-1
 walk(assets['route']);turn,pitch=aim(assets['station']);advance(16,turn=turn,pitch=pitch)
 for _ in range(240):advance(16)
 assert status()[0]==100 and status()[4]==0
 for _ in range(400):advance()
 walk(assets['crash']['route']);actors=observe();assert len(actors)==5
 for _ in range(1800):
  if status()[4]!=0 or status()[7]==5:break
  actors=observe();alive=[a for a in actors if a['health']>0 and a['visible']]
  if not alive:advance();continue
  target=min(alive,key=lambda a:math.dist(a['position'],pose()[:3]))['position'];target=target.copy();target[2]+=15
  turn,pitch=aim(target);advance(4 if status()[1]==0 else 2,turn=turn,pitch=pitch)
 assert status()[4]==0 and status()[7]==5,(status(),observe())
 walk(assets['grenades']['route'],assets['grenades']['barrier_waypoint'])
 for _ in range(1200):
  if status()[4]==1:break
  advance()
 assert status()[4]==(0 if thermal else 1) and status()[7]==5,(status(),observe())
 n=api.rc_game_mission_status(handle,None,0);text=c.create_string_buffer(n);assert api.rc_game_mission_status(handle,text,n)==n
 assert ('Granaten 5' if thermal else '5 Thermaldetonatoren') in text.raw.decode('utf-8')
 assert all(a['health']==0 for a in observe())
 n=api.rc_game_audio(handle,1,None,0);b=c.create_string_buffer(n+16);c.memset(b,0xa5,n+16)
 assert api.rc_game_audio(handle,1,b,n-1)==n and b.raw==b'\xa5'*(n+16)
 assert api.rc_game_audio(handle,1,b,n)==n and b.raw[n:]==b'\xa5'*16
 assert int.from_bytes(b.raw[8:12],'little')==12+status()[6]
 events=[int.from_bytes(b.raw[i+16:i+20],'little') for i in range(12,n,24)]
 assert events.count(assets['weapon_fire_sound'])==status()[6]
 for i in range(13):assert events.count(i)==(status()[6] if i==4 else 1),(i,events)
 n=api.rc_game_audio(handle,1,None,0);b=c.create_string_buffer(n);api.rc_game_audio(handle,1,b,n)
 assert int.from_bytes(b.raw[8:12],'little')==0
 if thermal:
  walk(thermal['route'])
  for _ in range(60):advance()
  target=thermal['route'][-1]
  for _ in range(900):
   x,y,z,yaw,pitch=pose()
   if math.dist([x,y,z],target)<3:break
   wanted=math.atan2(target[1]-y,target[0]-x)
   advance(forward=.03,turn=(yaw-wanted+math.pi)%math.tau-math.pi,pitch=pitch)
  for _ in range(60):advance()
  target=thermal['gun']['contact']['position'];turn,pitch=aim(target)
  shots=status()[6]
  if gunner:
   advance(32|2|4,forward=-1,turn=turn,pitch=pitch-.2)
   for tick in range(2,201):
    advance(32|(6 if tick<=57 else 0),forward=(-1 if tick<=35 else 1 if tick<155 else 0),turn=(math.pi if tick==36 else 0))
   assert status()[6]==shots and status()[4]==0,(status(),mission_text())
   for _ in range(400):
    if status()[4]!=0:break
    advance()
   assert status()[4]==1 and 'Geschützstellung' in mission_text(),(status(),mission_text())
   n=api.rc_game_audio(handle,1,None,0);b=c.create_string_buffer(n);assert api.rc_game_audio(handle,1,b,n)==n
   thrown=[int.from_bytes(b.raw[i+16:i+20],'little') for i in range(12,n,24)]
   for role in [13,14,17,18,19]:assert thrown.count(role)==1,(role,thrown)
   (OUT/'input-replay.bin').write_bytes(b'RCINPUT1'+struct.pack('<I',len(replay))+b''.join(struct.pack('<ffffI',*i) for i in replay))
   (OUT/'gunner-abi-validation.json').write_text(json.dumps({'passed':True,'scene_sha256':hashlib.sha256(data).hexdigest(),'input_driven_objectives':[0,1,2,3],'victory':True,'alive':status()[0]>0,'explosion_confirmation_audio_once':True,'sound_roles':thrown},indent=2))
  else:
   advance(32|2|4,turn=turn,pitch=pitch-.4)
   for _ in range(56):advance(32|2|4)
   assert status()[6]==shots,'Weapon gate leaked shot'
   for _ in range(143):advance(32)
   assert 'Geschütz getroffen' in mission_text(),mission_text()
   # Holding beyond animation end must not produce another throw.
   def drain():
    n=api.rc_game_audio(handle,1,None,0);b=c.create_string_buffer(n);assert api.rc_game_audio(handle,1,b,n)==n
    return [int.from_bytes(b.raw[i+16:i+20],'little') for i in range(12,n,24)]
   thrown=drain();assert thrown.count(13)==thrown.count(14)==1
   advance()
   for _ in range(4):
    advance(32)
    for _ in range(200):advance()
   advance(32)
   more=drain();assert more.count(13)==more.count(14)==4 and more.count(16)==1
   assert 15 in thrown+more and status()[4]==0
   (OUT/'live-abi-validation.json').write_text(json.dumps({'passed':True,'scene_sha256':hashlib.sha256(data).hexdigest(),
    'contiguous_mission_route':True,'bit5_live_throw':True,'world_contact621':True,'held_input_no_repeat':True,
    'weapon_gate':True,'five_foley_pairs_then_empty':True,'bounce_sound':True,'no_premature_victory':True},indent=2))
 assert api.rc_game_command(handle,1)==0 and status()[:5]==[25,20,200,0,0] and pose()==initial and observe()==[]
finally:api.rc_game_release(handle)
assert api.rc_game_actor_status(handle,None,0)==-1
result=dict(passed=True,scene_sha256=hashlib.sha256(data).hexdigest(),observer_short_buffers_and_canaries=True,invalid_handles_and_flags=True,physical_walk_three_objectives=True,grenades=5,five_factory_actors_and_kills=True,all_original_audio_roles_consumed_once=True,weapon_fire_events_match_shots=('weapon_fire_sound' in assets),restart_exact=True,scope='Native input-driven selected mission objectives with provisional combat rules; no native AI/Karma parity.')
if thermal and not gunner:result.update(grenades_collected=5,grenades_spent=5,live_throw=True,scope='Native objectives0/1/2, live thermal throw and trigger621 contact. Bunker destruction, radius damage and Android acceptance remain open.')
if gunner:result.update(gun_position_completed=True,scope='Native contiguous objectives0-3; bounded combat and particle adapters; Android checked separately.')
(OUT/'abi-validation.json').write_text(json.dumps(result,indent=2),encoding='utf-8');print(json.dumps(result))
