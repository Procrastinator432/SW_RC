"""Compare production scene repair against its explicit source, including CPU ramp rasterization."""
import argparse,ctypes as c,json,struct
from pathlib import Path
from PIL import Image
p=argparse.ArgumentParser();p.add_argument('before',type=Path);p.add_argument('after',type=Path);p.add_argument('--dll',type=Path,default=Path('target/release/rc_native.dll'));p.add_argument('--output',type=Path,default=Path('build/phone-blockers/material-validation'));a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
def scene(path):
 b=path.read_bytes();at=0
 for _ in range(16):
  magic=b[at:at+8];assert magic.startswith(b'RCPLAY');n=struct.unpack_from('<I',b,at+8)[0];at+=12+n
  if magic==b'RCPLAY01':break
 s=b[at:];magic,version,n,nt=struct.unpack_from('<4sIII',s);assert magic==b'RCSC' and version==3
 rows=[s[20+i*68:20+(i+1)*68] for i in range(n)];textures=[];pos=20+n*68
 for _ in range(nt):
  w,h=struct.unpack_from('<II',s,pos);end=pos+8+w*h*4;textures.append(s[pos:end]);pos=end
 return b[:at],rows,textures,s[pos:]
old,faces,tex,tail=scene(a.before);new,repaired,updated,newtail=scene(a.after)
assert old==new and tail==newtail and len(faces)==len(repaired),'Scripts, starts or collision changed'
assert updated[:len(tex)]==tex,'Original textures (including masked uses) changed'
changed=[]
for i,(f,g) in enumerate(zip(faces,repaired)):
 assert f[:64]==g[:64],'Geometry, surface or UV changed'
 if f!=g:
  src=struct.unpack_from('<i',f,64)[0];dst=struct.unpack_from('<i',g,64)[0];assert dst>=len(tex)
  source=tex[src];copy=updated[dst];assert source[:8]==copy[:8]
  assert all(x==y for j,(x,y) in enumerate(zip(source[8:],copy[8:])) if j%4!=3),'RGB changed'
  assert all(v==255 for v in copy[11::4]);changed.append(i)
assert 292 in changed and 293 in changed and len(changed)==21
api=c.CDLL(str(a.dll.resolve()));api.rc_scene_create.argtypes=[c.c_void_p,c.c_size_t,c.c_void_p,c.c_size_t];api.rc_scene_create.restype=c.c_uint64
api.rc_scene_draw.argtypes=[c.c_uint64,c.c_size_t,c.c_size_t,c.c_float,c.c_float,c.c_void_p,c.c_size_t];api.rc_scene_draw.restype=c.c_ssize_t;api.rc_scene_release.argtypes=[c.c_uint64]
coverage={}
for name,rows,textures in [('before',faces,tex),('after',repaired,updated)]:
 probe=bytearray()
 index=struct.unpack_from('<i',rows[292],64)[0]
 for i in [292,293]:
  row=bytearray(rows[i]);assert struct.unpack_from('<I',row,36)[0]==9;struct.pack_into('<i',row,64,0);probe.extend(row)
 data=struct.pack('<4sIII',b'RCSC',2,2,1)+probe+textures[index];buf=c.create_string_buffer(bytes(data));err=c.create_string_buffer(1024)
 handle=api.rc_scene_create(buf,len(data),err,1024);assert handle,err.value
 pixels=(c.c_uint32*(640*400))()
 try:assert api.rc_scene_draw(handle,640,400,.7,1.05,pixels,len(pixels))==len(pixels)
 finally:api.rc_scene_release(handle)
 coverage[name]=sum(v!=0xff101923 for v in pixels)
 Image.frombytes('RGBA',(640,400),bytes(pixels),'raw','BGRA').convert('RGB').save(a.output/f'ramp-{name}.png')
assert coverage['after']>coverage['before']*20
report=dict(passed=True,changed_faces=changed,ramp_pixels=coverage,original_texture_bytes_preserved=True,geometry_uv_collision_scripts_unchanged=True,private_opaque_copies_only=True,limits=['BSP opacity in GEO_01A; mesh/shader effects and physical route acceptance remain separate'])
(a.output/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps(report))
