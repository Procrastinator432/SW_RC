"""Independent original mesh references, diffuse chains/mips/pixels and preview oracle."""
from pathlib import Path
import collections, hashlib, json, re, struct
from PIL import Image, ImageChops
root=Path(__file__).resolve().parents[1]
source=root/'scripts/Record-OriginalAnimationTracks.py'; ns={'__file__':str(source)}
exec(compile(source.read_text().split("path=root/'analysis/reports/original-animation-tracks.json'")[0],str(source),'exec'),ns)
Reader,tables=ns['Reader'],ns['tables']
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
f=lambda v:struct.unpack('<f',struct.pack('<f',v))[0]
def checked(name,validation):
    p=root/'analysis/reports'/name
    assert sha(p)==json.loads((root/'analysis/reports'/validation).read_text())['report_sha256']
    return json.loads(p.read_text())
links=checked('original-skeletal-linkups.json','original-skeletal-linkups-validation.json')
lods=checked('skeletal-lods.json','skeletal-lods-validation.json')
draw=checked('skeletal-draw.json','skeletal-draw-validation.json')
path=root/'analysis/reports/skeletal-materials.json';report=json.loads(path.read_text())
game=Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData')
files={p.stem.lower():p for d in ['Textures','StaticMeshes','Animations','System'] for p in (game/d).iterdir() if p.suffix.lower() in ['.utx','.usx','.ukx','.u']}
cache={}; hashes={};stats=collections.Counter();errors=collections.Counter()
def package(file):
    file=Path(file)
    if file not in cache:
        b=file.read_bytes();cache[file]=(b,tables(b));hashes[str(file)]=sha(file)
    return cache[file]
def asset(path):
    package_name,object_name=path.split('.',1)
    if package_name.lower() not in files:raise ValueError('Missing material package '+package_name)
    data,meta=package(files[package_name.lower()]); version,licensee,names,exports,object_path=meta
    for i,e in enumerate(exports,1):
        if object_path(i).lower()==object_name.lower():return package_name,data,meta,e
    raise ValueError('Missing material '+path)
def properties(data,meta,e):
    _,_,names,_,_=meta;assert not e[3]&0x02000000
    r=Reader(data[e[5]:e[5]+e[4]]);props={}
    while True:
        name=names[r.index()]
        if name=='None':return props,r
        info=r.take(1)[0];kind=info&15
        if kind==10:r.index()
        sizecode=(info>>4)&7
        size=[1,2,4,12,16][sizecode] if sizecode<5 else int.from_bytes(r.take([1,2,4][sizecode-5]),'little')
        ai=0
        if info&128 and kind!=3:
            b=r.take(1)[0];ai=b if b<128 else ((b&63)<<8)|r.take(1)[0] if not b&64 else ((b&63)<<24)|int.from_bytes(r.take(3),'big')
        raw=r.take(size) if kind!=3 else b''
        if kind in (5,8):v=Reader(raw).index()
        elif kind==4:v=struct.unpack('<f',raw)[0]
        elif kind==2:v=int.from_bytes(raw,'little',signed=True)
        elif kind==1:v=raw[0]
        else:v=raw
        props.setdefault(name,v)
def decode(fmt,w,h,raw):
    if fmt==5:return [int.from_bytes(raw[i:i+4],'little') for i in range(0,len(raw),4)]
    assert fmt in (3,7,8)
    stride=8 if fmt==3 else 16;bw=(w+3)//4;out=[0]*(w*h)
    def rgb(v):
        r=(v>>11)&31;g=(v>>5)&63;b=v&31
        return [(r<<3)|(r>>2),(g<<2)|(g>>4),(b<<3)|(b>>2)]
    for block in range(len(raw)//stride):
        chunk=raw[block*stride:(block+1)*stride];colors=chunk[-8:];a,b,selectors=struct.unpack('<HHI',colors)
        table=[rgb(a),rgb(b)];opaque=fmt!=3 or a>b
        if opaque:table += [[(2*x+y)//3 for x,y in zip(*table)],[(x+2*y)//3 for x,y in zip(*table)]]
        else:table += [[(x+y)//2 for x,y in zip(*table)],[0,0,0]]
        if fmt==8:
            x,y=chunk[:2];alpha=[x,y]
            alpha += [((7-i)*x+i*y)//7 for i in range(1,7)] if x>y else [((5-i)*x+i*y)//5 for i in range(1,5)]+[0,255]
            abits=int.from_bytes(chunk[2:8],'little')
        for i in range(16):
            x=block%bw*4+i%4;y=block//bw*4+i//4
            if x>=w or y>=h:continue
            c=(selectors>>(i*2))&3;r,g,b=table[c]
            a=((int.from_bytes(chunk[:8],'little')>>(i*4))&15)*17 if fmt==7 else alpha[(abits>>(i*3))&7] if fmt==8 else 255
            if not opaque and c==3:a=0
            out[y*w+x]=(a<<24)|(r<<16)|(g<<8)|b
    return out
def resolve(path):
    chain=[];visited=set();scale=1.
    for _ in range(16):
        if path.lower() in visited:raise ValueError('Material cycle')
        visited.add(path.lower());pn,data,meta,e=asset(path);objpath=meta[4];cls=objpath(e[0]);chain.append(cls)
        props,r=properties(data,meta,e)
        if cls=='Engine.Texture':
            fmt=props.get('Format',0);mips=[]
            for _ in range(r.count()):
                end=r.u32();size=r.index();assert 0<=size<=64*1024*1024
                raw=r.take(size);assert end==e[5]+r.pos
                w,h=r.u32(),r.u32();r.take(2)
                if raw:mips.append((w,h,raw))
            assert r.pos==e[4]
            if not mips:raise ValueError('No stored texture mip')
            w,h,raw=min(mips,key=lambda m:abs(max(m[:2])-256));pixels=decode(fmt,w,h,raw)
            ow,oh=min(w,256),min(h,256)
            resized=[pixels[(y*h//oh)*w+x*w//ow] for y in range(oh) for x in range(ow)]
            stats['texture_pixels']+=ow*oh
            return dict(source=path,chain=chain,uv_scale=scale,format=fmt,source_mip=[w,h],width=ow,height=oh,pixels=resized)
        if cls=='Engine.Shader':field='Diffuse'
        elif cls=='Engine.FinalBlend':field='Material'
        elif cls in ['Engine.HsBumpDiff','Engine.HsBumpDiffSpec','Engine.HsBumpDiffSpecMask','Engine.HsBumpIllumSpecMask','Engine.HsBumpDiffBlend','Engine.HsBumpDiffBlendMask','Engine.HsBumpDiffBlendMaskIllum']:
            field='DiffuseTexture';scale=f(scale*props.get('DiffUVScale',1.))
        elif cls=='Engine.Combiner':field={1:'Material2',7:'Mask'}.get(props.get('CombineOperation',0),'Material1')
        elif cls=='Engine.TexCoordSource' and props.get('SourceChannel',0)==0:field='Material'
        else:raise ValueError('Unsupported material class '+cls)
        if field not in props:raise ValueError(f'No {field} in {path}')
        index=props[field]
        if not index:raise ValueError('Empty diffuse reference')
        path=(pn+'.' if index>0 else '')+objpath(index)
    raise ValueError('Material chain exceeds limit')

texture_map={}
for i,t in enumerate(report['textures']):
    expected=resolve(t['material']);assert {k:v for k,v in t.items() if k!='material'}==expected,t['material']
    texture_map[t['material'].lower()]=i;stats['decoded_textures']+=1
lod_map={o['source_index']:o for o in lods['objects']}
assert len(report['objects'])==130
for obj in report['objects']:
    original=links['objects'][obj['source_index']];file=Path(original['file']);data,meta=package(file);e=meta[3][original['export_index']-1]
    r=Reader(data[e[5]:e[5]+e[4]],original['prefix']['native_offset']+41+8)
    r.take(r.count()*4);assert r.pos==obj['materials_offset'];refs=[r.index() for _ in range(r.count())]
    qualify=lambda i:(file.stem+'.' if i>0 else '')+meta[4](i)
    assert obj['stored']==[dict(index=i,path=qualify(i) if i else None) for i in refs]
    slots=sorted({s[0] for l in lod_map[obj['source_index']]['mesh']['lods'] for bank in l['sections'] for s in bank if s[8]})
    assert [b['slot'] for b in obj['bindings']]==slots
    for b in obj['bindings']:
        slot=b['slot'];reference=(refs[slot] if slot<len(refs) else refs[0] if refs else 0) or None
        material=qualify(reference) if reference else None
        assert (b['selected_reference'],b['material'])==(reference,material)
        if material and material.lower() in texture_map:
            assert b['texture']==texture_map[material.lower()] and b['error'] is None;stats['resolved_slots']+=1
        else:
            try:
                if not material:raise ValueError('No selected mesh material')
                resolve(material)
                raise AssertionError('unexpected resolved omission')
            except ValueError as error:assert b['error']==str(error);errors[str(error)]+=1
            assert b['texture'] is None;stats['omitted_slots']+=1
    stats['material_references']+=len(refs);stats['meshes']+=1

preview_path=root/'analysis/reports/skeletal-textured-preview.json';preview=json.loads(preview_path.read_text())
objects={o['source_index']:o for o in report['objects']};preview_hashes={}
def snapshot(p):
    data=p.read_bytes();assert data[:8]==b'RCSC\x02\0\0\0'
    count,nt=struct.unpack_from('<II',data,8);faces=[struct.unpack_from('<16Ii',data,16+i*68) for i in range(count)];offset=16+count*68;textures=[]
    for _ in range(nt):
        w,h=struct.unpack_from('<II',data,offset);offset+=8;pixels=list(struct.unpack_from('<'+'I'*(w*h),data,offset));offset+=w*h*4;textures.append((w,h,pixels))
    assert offset==len(data);return faces,textures
for p in preview['previews']:
    old,_=snapshot(root/p['input_snapshot']);faces,textures=snapshot(root/p['snapshot']);assert len(faces)==len(old)==p['triangles']
    bindings=objects[p['source_index']]['bindings'];mapping={};expected_textures=[]
    for b in bindings:
        if b['texture'] is not None:
            t=report['textures'][b['texture']];mapping[b['slot']]=(len(expected_textures),t['uv_scale']);expected_textures.append((t['width'],t['height'],t['pixels']))
    assert textures==expected_textures
    for a,b in zip(old,faces):
        assert a[:10]==b[:10];slot=a[9];texture,scale=mapping[slot]
        expected=[struct.unpack('<I',struct.pack('<f',f(struct.unpack('<f',struct.pack('<I',v))[0]*scale)))[0] for v in a[10:16]]
        assert b[10:16]==tuple(expected) and b[16]==texture
    assert p['diffuse_triangles']==p['triangles']==3500
    ppm=root/p['ppm'];im=Image.open(ppm);im.save(ppm.with_suffix('.png'))
    bbox=ImageChops.difference(im,Image.new('RGB',im.size,im.getpixel((0,0)))).getbbox();assert bbox and min(bbox[:2])>0 and max(bbox[2:])<512
    preview_hashes[str(ppm)]=sha(ppm);stats['preview_frames']+=1
assert len(set(preview_hashes.values()))==4
assert (stats['meshes'],stats['resolved_slots'],stats['omitted_slots'],stats['decoded_textures'],stats['preview_frames'])==(130,212,77,138,4)
assert (report['resolved_slots'],report['omitted_slots'])==(212,77)
log=(root/'analysis/reports/skeletal-material-tests.log').read_text();tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)));assert tests==496 and 'test result: FAILED' not in log
sources=['crates/rc-package/src/skeletal_mesh.rs','crates/rc-package/src/skeletal_material.rs','crates/rc-inspect/src/assets.rs','crates/rc-inspect/src/lib.rs',
         'crates/rc-inspect/src/bin/rc-skeletal-material-check.rs','crates/rc-inspect/src/bin/rc-skeletal-textured-preview.rs','crates/rc-render/src/skeletal.rs',
         'scripts/Record-SkeletalMaterials.py','docs/SKELETAL_MATERIALS.md','analysis/decompiled/skeletal-material-bindings.c','analysis/decompiled/skeletal-material-array.c']
result=dict(date='2026-10-08',rust_tests=tests,counts=dict(stats),omissions=dict(errors),scope=report['scope'],
    completed_tasks=['stored mesh material references and native nonnegative slot/actor-override selection',
                     'original diffuse chains and mip decode for skeletal material slots',
                     'original diffuse texture/UV-scale renderer binding and four animation previews'],
    report_sha256=sha(path),preview_report_sha256=sha(preview_path),source_sha256={s:sha(root/s) for s in sources},original_package_sha256=hashes,preview_sha256=preview_hashes,
    checks=['cargo test --workspace: 496 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check',
            '130 original material arrays and every used slot independently read from package bytes',
            '138 original diffuse textures independently decoded pixel-for-pixel, with chain references and UV scales checked',
            'all 77 omissions independently reproduced; four preview snapshots preserve original geometry and have correct texture pixels/UVs',
            '12 new tests for material selection callback order, fallbacks, texture binding, missing slots and atomic validation'],
    remaining=['unsupported material wrappers, dynamic UV transforms and shader/effect evaluation','actor instance overrides and live runtime integration',
               'actor/world transforms, instance lifecycle and mesh tails','Actor/Script/game loop','final Android verification'])
(root/'analysis/reports/skeletal-materials-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
e=root/'analysis/evidence.json';state=json.loads(e.read_text(encoding='utf-8-sig'));state['rust_tests']=tests;state['skeletal_materials_validation']=result;e.write_text(json.dumps(state,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-08: Drei Material-Aufgaben abgeschlossen: gespeicherte Mesh-Materialreferenzen und native Slot-/Actor-Override-Auswahl, '
      'Original-Diffuseketten/Mips und Rendererbindung mit Original-UV-Skalierung. 130 Meshes, 212 aufgeloeste und 77 explizit ausgelassene '
      'Slots; 138 Originaltexturen unabhaengig pixelgenau geprueft. Vier Clone-Commando-Frames mit Original-Diffuse statt Schachbrett, '
      'je 3500 texturierte Dreiecke. 496 Tests, Clippy und Format bestanden. Dynamische Materialeffekte, nicht unterstuetzte Wrapper '
      'und laufende Spielintegration offen; Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_MATERIALS.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for target in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Drei Material-Aufgaben abgeschlossen: gespeicherte Mesh-Materialreferenzen' not in target.read_text(encoding='utf-8-sig'):
        with target.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(dict(stats)));print(json.dumps(dict(errors)))
