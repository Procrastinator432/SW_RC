"""Exercise ordinary mission inputs, immutable loop snapshots and cold restore.
No fabricated gameplay state: Android fixtures are captured from this replay.
"""
import argparse
import copy
import ctypes as c
import hashlib
import json
import struct
from pathlib import Path

p=argparse.ArgumentParser();p.add_argument('--dll',type=Path,required=True);p.add_argument('--scenes',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--only-a',action='store_true',help='Check the A source host while later scene stages are still building');args=p.parse_args()
args.output.mkdir(parents=True,exist_ok=True)
api=c.CDLL(str(args.dll.resolve()));error=c.create_string_buffer(2048)
api.rc_game_create.argtypes=[c.c_void_p,c.c_size_t,c.c_void_p,c.c_size_t];api.rc_game_create.restype=c.c_uint64
api.rc_game_restore_checkpoint.argtypes=[c.c_uint64,c.c_void_p,c.c_size_t,c.c_void_p,c.c_size_t];api.rc_game_restore_checkpoint.restype=c.c_int32
api.rc_game_checkpoint.argtypes=[c.c_uint64,c.c_void_p,c.c_size_t];api.rc_game_checkpoint.restype=c.c_ssize_t
api.rc_game_audio.argtypes=[c.c_uint64,c.c_uint32,c.c_void_p,c.c_size_t];api.rc_game_audio.restype=c.c_ssize_t
api.rc_game_release.argtypes=[c.c_uint64]
api.rc_game_command.argtypes=[c.c_uint64,c.c_uint32];api.rc_game_command.restype=c.c_int32
api.rc_game_advance.argtypes=[c.c_uint64,c.c_double,c.c_float,c.c_float,c.c_float,c.c_float,c.c_uint32];api.rc_game_advance.restype=c.c_int32

def create(raw):
 h=api.rc_game_create(c.create_string_buffer(raw),len(raw),error,len(error));assert h,error.value;return h
def buffer(h,fn,*extra):
 n=fn(h,*extra,None,0);assert n>=12;out=c.create_string_buffer(n);assert fn(h,*extra,out,n)==n;return out.raw
def checkpoint(h):return buffer(h,api.rc_game_checkpoint)
def audio(h,kind=6):return buffer(h,api.rc_game_audio,kind)
def restore(h,saved):assert api.rc_game_restore_checkpoint(h,c.create_string_buffer(saved),len(saved),error,len(error))==0,error.value
def body(saved):return json.loads(saved[20:])
def save(obj):
 b=json.dumps(obj,separators=(',',':')).encode();value=0xcbf29ce484222325
 for v in b:value=((value^v)*0x100000001b3)&0xffffffffffffffff
 return b'RCSAVE01'+struct.pack('<IQ',len(b),value)+b
def packet(b):
 assert b[:8] in [b'RCLOOP02',b'RCLOOP03'];identified=b[:8]==b'RCLOOP03';step=44 if identified else 36;count=struct.unpack_from('<I',b,8)[0];assert count<=16 and len(b)==28+count*step
 listener=struct.unpack_from('<4f',b,12);sources={}
 for at in range(28,len(b),step):
  sound,owner,x,y,z,volume,pitch,radius,flags=struct.unpack_from('<Ii6fI',b,at)
  assert owner>0 and owner not in sources and flags&~7==0
  sources[owner]=dict(sound=sound,location=[x,y,z],volume=volume,pitch=pitch,radius=radius,flags=flags)
  if identified:sources[owner]['activation']=struct.unpack_from('<Q',b,at+36)[0]
 return dict(listener=listener,sources=sources)

manifest=json.loads((args.scenes/'resolved-inputs.json').read_text(encoding='utf-8-sig'))
scene=Path(manifest['inputs']['map_a_scene']['path']).read_bytes();size=struct.unpack_from('<I',scene,8)[0];meta=json.loads(scene[12:12+size]);profiles={v['owner']:v for v in meta['ambient_profiles']};assert len(profiles)==6
lifecycle=meta.get('ambient_policy') is not None
invalid_profiles=[]
for mutation in ['owner','sound','duplicate','category','radius']:
 bad=copy.deepcopy(meta);entry=bad['ambient_profiles'][0]
 if mutation=='owner':entry['owner']=999999
 elif mutation=='sound':entry['sound']=999999
 elif mutation=='duplicate':bad['ambient_profiles'].append(copy.deepcopy(entry))
 elif mutation=='category':entry['params']['category']=1
 elif mutation=='radius':entry['params']['radius']=-1
 encoded=json.dumps(bad,separators=(',',':')).encode();raw=scene[:8]+struct.pack('<I',len(encoded))+encoded+scene[12+size:]
 invalid=api.rc_game_create(c.create_string_buffer(raw),len(raw),error,len(error))
 if invalid:api.rc_game_release(invalid)
 assert not invalid,mutation
 invalid_profiles.append(mutation)
frames=Path(manifest['inputs']['map_a_replay']['path']).read_bytes();h=create(scene);captured={};best_fire_distance=float('inf');previous={};admissions={};fade_owners=set();priority_checks=0
f32=lambda v:struct.unpack('<f',struct.pack('<f',v))[0]
try:
 initial=checkpoint(h)
 for tick,at in enumerate(range(12,len(frames),20),1):
  f,s,x,y,buttons=struct.unpack_from('<ffffI',frames,at)
  assert api.rc_game_advance(h,1/60,f,s,x,y,buttons)==1,error.value
  current=audio(h);loops=packet(current)
  new=[o for o,v in loops['sources'].items() if previous.get(o)!=v['activation']]
  for owner in new:admissions[owner]=admissions.get(owner,0)+1
  previous={o:v['activation'] for o,v in loops['sources'].items()}
  if lifecycle and new:
   voices=body(checkpoint(h))['state']['opening']['ambient_audio']['voices']
   for owner in new:
    source=loops['sources'][owner];voice=next(v for v in voices if v['owner']==owner)
    assert source['activation']==voice['started'] and profiles[owner]['params']['max_volume']==1
    dx,dy,dz=[f32(source['location'][i]-loops['listener'][i]) for i in range(3)]
    squared=f32(f32(f32(dz*dz)+f32(dy*dy))+f32(dx*dx));radius=f32(source['radius']*100)
    priority=max(f32(.01),min(1.,f32(1-f32(squared/f32(radius*radius)))))
    assert struct.pack('<f',priority)==struct.pack('<f',voice['priority']),'Saved priority does not match original admission inputs'
    priority_checks+=1
  if lifecycle:
   for owner,source in loops['sources'].items():
    if source['volume']<.999:
     fade_owners.add(owner)
     if 'fade' not in captured:captured['fade']=(checkpoint(h),current)
  if 736 in loops['sources'] and 'bacta' not in captured:
   captured['bacta']=(checkpoint(h),current)
  if tick>=1930 and tick%60==0 and len(loops['sources'])>=(1 if lifecycle else 5) and any(o!=736 for o in loops['sources']):
   nearest=min(sum((s['location'][i]-loops['listener'][i])**2 for i in range(3))**.5 for o,s in loops['sources'].items() if o!=736)
   if nearest<best_fire_distance*.99:
    saved=checkpoint(h);state=body(saved)['state']
    if not state['opening']['locked'] and not state['mission']['using'] and state['phase']=='Playing':
     captured['fire']=(saved,current);best_fire_distance=nearest
 assert set(captured)==({'bacta','fire','fade'} if lifecycle else {'bacta','fire'}),captured.keys()
 if lifecycle:assert fade_owners and any(n>1 for o,n in admissions.items() if o!=736),'Ordinary replay did not exercise release and re-admission'
 assert body(checkpoint(h))['state']['phase']=='Won'
 assert not packet(audio(h))['sources'],'Won mission kept playing old ambient voices'
 checks=[]
 for name,(saved,expected) in captured.items():
  restore(h,saved);before=checkpoint(h)
  for _ in range(4):assert audio(h)==expected
  short=c.create_string_buffer(b'xxxxxxxx');required=api.rc_game_audio(h,6,None,0)
  assert api.rc_game_audio(h,6,short,8)==required and short.raw==b'xxxxxxxx\0'
  legacy=packet(audio(h,5));modern=packet(expected)
  assert legacy['listener']==modern['listener']
  assert legacy['sources']=={o:{k:v for k,v in s.items() if k!='activation'} for o,s in modern['sources'].items()}
  assert checkpoint(h)==before,'Loop query changed playback or RNG'
  state=body(saved)['state'];voices=state['opening']['ambient_audio']['voices'];parsed=packet(expected)
  assert voices
  for owner,source in parsed['sources'].items():
   profile=profiles[owner];voice=next(v for v in voices if v['owner']==owner);pb=voice['playback'];params=profile['params']
   assert source['sound']==profile['sound']==voice['sound'] and source['activation']==voice['started']
   gain=voice.get('fade',{}).get('value',1.)
   assert struct.pack('<3f',source['volume'],source['pitch'],source['radius'])==struct.pack('<3f',pb['volume']*gain,pb['pitch'],pb['radius'])
   assert params['min_volume']<=pb['volume']<=min(1,params['max_volume']) and params['min_pitch']<=pb['pitch']<=params['max_pitch']
   if owner!=736:
    anchor=next(b for b in meta['world_effects'] if b['original']==owner)['location'];assert struct.pack('<3f',*source['location'])==struct.pack('<3f',*anchor)
  fresh=create(scene)
  try:restore(fresh,saved);assert audio(fresh)==expected;assert body(checkpoint(fresh))==body(saved)
  finally:api.rc_game_release(fresh)
  for kind in [1,2,4]:
   restore(h,saved);audio(h,kind);assert audio(h)==expected,'One-shot consumption altered continuous source'
  restore(h,saved);before=checkpoint(h);bad=[]
  mutations=['missing_module','owner','duplicate','future','pitch','muted']
  if name!='fade':mutations.append('missing_active')
  if lifecycle:mutations.append('priority')
  if name=='fade':mutations+=['fade_value','fade_future']
  for mutation in mutations:
   obj=body(saved);a=obj['state']['opening']['ambient_audio'];v=a['voices'][0]
   if mutation=='missing_module':obj['state']['opening']['ambient_audio']=None
   elif mutation=='missing_active':a['voices']=[]
   elif mutation=='owner':v['owner']=999999
   elif mutation=='duplicate':a['voices'].append(copy.deepcopy(v))
   elif mutation=='future':v['started']=obj['state']['statistics']['ticks']+1
   elif mutation=='pitch':v['playback']['pitch']=1.9
   elif mutation=='muted':v['playback']=None
   elif mutation=='priority':v['priority']=2
   elif mutation=='fade_value':next(v for v in a['voices'] if v.get('fade'))['fade']['value']=.123
   elif mutation=='fade_future':next(v for v in a['voices'] if v.get('fade'))['fade']['started']=obj['state']['statistics']['ticks']+1
   raw=save(obj);assert api.rc_game_restore_checkpoint(h,c.create_string_buffer(raw),len(raw),error,len(error))!=0,mutation
   assert checkpoint(h)==before,'Rejected audio state changed game'
   bad.append(mutation)
  (args.output/('A-ambient-'+name+'.rcsave')).write_bytes(saved)
  checks.append(dict(name=name,tick=state['statistics']['ticks'],snapshot=parsed,query_and_short_copy_read_only=True,cold_restore_exact=True,one_shots_independent=True,invalid_checkpoint_atomic=bad))
 assert api.rc_game_command(h,1)==0 and checkpoint(h)==initial,'Restart did not reset ambient state'
 assert api.rc_game_command(h,0)==0 and not packet(audio(h))['sources']
finally:api.rc_game_release(h)

# Old scene pairs continue to use RCLOOP01 and neutral RCLOOP02 records.
for key in ([] if args.only_a else ['map_b_scene','map_c_scene','factory_scene']):
 raw=Path(manifest['inputs'][key]['path']).read_bytes();h=create(raw)
 try:
  before=checkpoint(h);assert not packet(audio(h))['sources'];assert checkpoint(h)==before
 finally:api.rc_game_release(h)
report=dict(passed=True,native_dll_sha256=hashlib.sha256(args.dll.read_bytes()).hexdigest(),scene_sha256=hashlib.sha256(scene).hexdigest(),ordinary_A_inputs=(len(frames)-12)//20,profiles=6,invalid_profiles_rejected=invalid_profiles,checks=checks,won_stops_loops=True,restart_resets_stream=True,original_admission_and_release=lifecycle,admissions=admissions,fade_owners=sorted(fade_owners),original_admission_priorities_checked=priority_checks,activation_identity_verified=True)
(args.output/'validation.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8');print(json.dumps(dict(passed=True,profiles=6,fixtures=list(captured))))
