"""Verify immutable queued playback, ABI query/copy and both legacy event formats.
Uses controlled original-animation snapshots produced by rc-weapon-foley-check.
"""
import argparse,ctypes as c,hashlib,json,struct
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--dll',type=Path,required=True);p.add_argument('--scenes',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--scene-keys',nargs='+',default=['map_a_scene','map_b_scene','map_c_scene','factory_scene']);args=p.parse_args()
api=c.CDLL(str(args.dll.resolve()));error=c.create_string_buffer(1024)
api.rc_game_create.argtypes=[c.c_void_p,c.c_size_t,c.c_void_p,c.c_size_t];api.rc_game_create.restype=c.c_uint64
api.rc_game_restore_checkpoint.argtypes=[c.c_uint64,c.c_void_p,c.c_size_t,c.c_void_p,c.c_size_t];api.rc_game_restore_checkpoint.restype=c.c_int32
api.rc_game_checkpoint.argtypes=[c.c_uint64,c.c_void_p,c.c_size_t];api.rc_game_checkpoint.restype=c.c_ssize_t
api.rc_game_audio.argtypes=[c.c_uint64,c.c_uint32,c.c_void_p,c.c_size_t];api.rc_game_audio.restype=c.c_ssize_t
api.rc_game_release.argtypes=[c.c_uint64]
api.rc_game_advance.argtypes=[c.c_uint64,c.c_double,c.c_float,c.c_float,c.c_float,c.c_float,c.c_uint32];api.rc_game_advance.restype=c.c_int32
def checkpoint(h):
 n=api.rc_game_checkpoint(h,None,0);assert n>20;out=c.create_string_buffer(n);assert api.rc_game_checkpoint(h,out,n)==n;return out.raw
def restore(h,b):assert api.rc_game_restore_checkpoint(h,c.create_string_buffer(b),len(b),error,1024)==0,error.value
def audio(h,kind):
 n=api.rc_game_audio(h,kind,None,0);assert n>=12;out=c.create_string_buffer(n);assert api.rc_game_audio(h,kind,out,n)==n;return out.raw
manifest=json.loads((args.scenes/'resolved-inputs.json').read_text(encoding='utf-8-sig'));checks=[]
for key in args.scene_keys:
 raw=Path(manifest['inputs'][key]['path']).read_bytes();saved=(args.scenes/'weapon-foley'/key/'queued-audio.rcsave').read_bytes();expected=json.loads(saved[20:]);records=expected['state']['visuals']['weapon_animation']['audio'];assert len(records)==1
 h=api.rc_game_create(c.create_string_buffer(raw),len(raw),error,1024);assert h,error.value
 try:
  if key=='map_a_scene':
   replay=Path(manifest['inputs']['map_a_replay']['path']).read_bytes()
   for at in range(12,12+1930*20,20):
    forward,strafe,look_x,look_y,buttons=struct.unpack_from('<ffffI',replay,at)
    assert api.rc_game_advance(h,1/60,forward,strafe,look_x,look_y,buttons)==1
   normal=checkpoint(h);state=json.loads(normal[20:]);records_normal=state['state']['visuals']['weapon_animation']['audio']
   assert records_normal and all(r['sequence']>0 for r in records_normal),'Normal tick path lost playback records'
   restore(h,normal);assert json.loads(checkpoint(h)[20:])==state,'Normal input checkpoint changed'
  restore(h,saved);before=checkpoint(h);n=api.rc_game_audio(h,4,None,0);assert n==60
  short=c.create_string_buffer(b'xxxxxxxx');assert api.rc_game_audio(h,4,short,8)==n and short.raw==b'xxxxxxxx\0'
  assert checkpoint(h)==before,'Query or short buffer consumed playback/RNG'
  packet=audio(h,4);assert packet[:8]==b'RCAUD003' and struct.unpack_from('<I',packet,8)[0]==1
  sequence,tick,sound,owner,category,flags,volume,pitch,radius,parameters=struct.unpack_from('<QQIiIIfffI',packet,12)
  record=records[0];playback=record['playback'];assert sequence==record['sequence'] and tick==record['tick'] and owner==-1 and category==playback['category'] and parameters==1
  assert struct.pack('<fff',volume,pitch,radius)==struct.pack('<fff',playback['volume'],playback['pitch'],playback['radius'])
  assert volume==0.25 and 0.95<=pitch<=1.05 and radius==100
  after=json.loads(checkpoint(h)[20:])['state'];assert after['visuals']['weapon_animation']['audio']==[]
  assert after['visuals']['weapon_animation']['selection']==expected['state']['visuals']['weapon_animation']['selection']
  assert len(audio(h,4))==12,'Copy did not consume exactly once'
  for kind,magic,size in [(1,b'RCAUD001',36),(2,b'RCAUD002',44)]:
   restore(h,saved);packet=audio(h,kind);assert packet[:8]==magic and len(packet)==size
   assert len(audio(h,4))==12
   assert json.loads(checkpoint(h)[20:])['state']['visuals']['weapon_animation']['audio']==[]
  checks.append(dict(scene=key,queued_parameters_exact=True,query_and_short_copy_read_only=True,all_three_formats_consume_once=True,consumption_preserves_rng=True,normal_input_checkpoint_1930=(key=='map_a_scene')))
 finally:api.rc_game_release(h)
args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(dict(passed=True,native_dll_sha256=hashlib.sha256(args.dll.read_bytes()).hexdigest(),checks=checks),indent=2)+'\n',encoding='utf-8');print(json.dumps(dict(passed=True,scenes=len(checks))))
