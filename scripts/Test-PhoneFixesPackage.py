"""Verify the shipped ZIP, not an intermediate builder output: native sound banks and ramp opacity."""
import argparse,base64,ctypes as c,hashlib,json,struct,zipfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('package',type=Path);p.add_argument('--dll',type=Path,default=Path('target/release/rc_native.dll'));p.add_argument('--output',type=Path,default=Path('build/phone-blockers/package-validation.json'));p.add_argument('--legacy-inputs',type=Path);p.add_argument('--legacy-checkpoints',type=Path);a=p.parse_args()
api=c.CDLL(str(a.dll.resolve()));api.rc_game_create.argtypes=[c.c_void_p,c.c_size_t,c.c_void_p,c.c_size_t];api.rc_game_create.restype=c.c_uint64
api.rc_game_audio.argtypes=[c.c_uint64,c.c_uint32,c.c_void_p,c.c_size_t];api.rc_game_audio.restype=c.c_ssize_t;api.rc_game_release.argtypes=[c.c_uint64]
checks=[]
with zipfile.ZipFile(a.package) as z:
 props=dict(line.split('=',1) for line in z.read('campaign.properties').decode().splitlines() if '=' in line)
 for name,target in [('GEO_01A',b'RCPLAY04'),('GEO_01B',b'RCPLAY14')]:
  raw=z.read(props[name+'.file']);assert hashlib.sha256(raw).hexdigest()==props[name+'.sha256'];at=0;metadata=None;opening=None
  while raw[at:at+6]==b'RCPLAY':
   magic=raw[at:at+8];n=struct.unpack_from('<I',raw,at+8)[0]
   if magic==target:metadata=json.loads(raw[at+12:at+12+n])
   if magic==b'RCPLAY13':opening=json.loads(raw[at+12:at+12+n])
   at+=12+n
   if magic==b'RCPLAY01':break
  outer=opening is not None and opening.get('enemy_fire_source') is not None
  assert metadata is not None and (outer or 'enemy_fire_sound' in metadata),'Missing production enemy fire binding'
  index=metadata.get('enemy_fire_sound');sound=next(s for s in opening['sounds'] if s['source']==opening['enemy_fire_source']) if outer else metadata['sounds'][index];assert sound['source']=='GEN_Sound.Weapons.wep_GEN_sbdGun_01'
  wave=sound['wave'];expected=base64.b64decode(wave[7:]) if isinstance(wave,str) else bytes(wave)
  buf=c.create_string_buffer(raw);err=c.create_string_buffer(1024);handle=api.rc_game_create(buf,len(raw),err,1024);assert handle,err.value
  try:
   n=api.rc_game_audio(handle,0,None,0);assert n>12;bank=c.create_string_buffer(n);assert api.rc_game_audio(handle,0,bank,n)==n
   b=bank.raw;assert b[:8]==b'RCSND001';samples=[];pos=12
   for _ in range(struct.unpack_from('<I',b,8)[0]):
    size=struct.unpack_from('<I',b,pos)[0];pos+=4;samples.append(b[pos:pos+size]);pos+=size
   if outer:index=len(samples)-1
   assert pos==len(b) and samples[index]==expected,'Native sound-bank index does not select the original cue'
  finally:api.rc_game_release(handle)
  if name=='GEO_01A':
   scene=raw[at:];version,count,nt=struct.unpack_from('<III',scene,4);assert version==3
   indices=[struct.unpack_from('<i',scene,20+i*68+64)[0] for i in [292,293]];assert indices[0]==indices[1]
   textures=[];pos=20+68*count
   for _ in range(nt):
    w,h=struct.unpack_from('<II',scene,pos);pos+=8;textures.append(scene[pos:pos+w*h*4]);pos+=w*h*4
   assert all(v==255 for v in textures[indices[0]][3::4]);assert sum(v<128 for v in textures[62][3::4])==16278
  checks.append(dict(map=name,native_sound_index=index,original_pcm_sha256=hashlib.sha256(expected).hexdigest(),scene_sha256=props[name+'.sha256']))
report=dict(passed=True,package_sha256=hashlib.sha256(a.package.read_bytes()).hexdigest(),production_ramp_opaque_original_alpha_preserved=True,original_enemy_cue_in_both_native_banks=True,checks=checks)
if a.legacy_inputs or a.legacy_checkpoints:
 assert a.legacy_inputs and a.legacy_checkpoints,'Provide both legacy input manifest and checkpoint directory'
 manifest=json.loads(a.legacy_inputs.read_text(encoding='utf-8-sig'));legacy=[]
 api.rc_game_restore_checkpoint.argtypes=[c.c_uint64,c.c_void_p,c.c_size_t,c.c_void_p,c.c_size_t];api.rc_game_restore_checkpoint.restype=c.c_int32
 for key,name in [('map_a_scene','A-4000.rcsave'),('map_b_scene','B-6000.rcsave'),('map_c_scene','C-3400.rcsave'),('factory_scene','factory-substep.rcsave')]:
  entry=manifest['inputs'][key];path=Path(entry['path']);path=path if path.is_absolute() else a.legacy_inputs.parent/path
  raw=path.read_bytes();assert hashlib.sha256(raw).hexdigest()==entry['sha256'];save=(a.legacy_checkpoints/name).read_bytes()
  assert 'enemy_traces' not in json.loads(save[20:])['state'],'Expected a checkpoint from before this change'
  buf=c.create_string_buffer(raw);err=c.create_string_buffer(1024);handle=api.rc_game_create(buf,len(raw),err,1024);assert handle,err.value
  try:
   state=c.create_string_buffer(save);assert api.rc_game_restore_checkpoint(handle,state,len(save),err,1024)==0,err.value
  finally:api.rc_game_release(handle)
  legacy.append(dict(scene=key,checkpoint=name,restored=True))
 report['legacy_checkpoints']=legacy
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(report,indent=2));print(json.dumps(report))
