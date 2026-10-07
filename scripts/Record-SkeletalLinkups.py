"""Independent original skeletal prefix parser, name linkage and mapped sampling oracle."""
from pathlib import Path
import json,struct,hashlib,math
root=Path(__file__).resolve().parents[1]
# Reuse only the previously audited independent archive reader definitions, not its run.
source=root/'scripts/Record-OriginalAnimationTracks.py'
namespace={'__file__':str(source)}
exec(compile(source.read_text().split("path=root/'analysis/reports/original-animation-tracks.json'")[0],str(source),'exec'),namespace)
Reader,tables=namespace['Reader'],namespace['tables']
f=lambda x:struct.unpack('<f',struct.pack('<f',x))[0]
bits=lambda x:struct.unpack('<I',struct.pack('<f',x))[0]
value=lambda x:struct.unpack('<f',struct.pack('<I',x))[0]
path=root/'analysis/reports/original-skeletal-linkups.json';report=json.loads(path.read_text())
previous_path=root/'analysis/reports/original-animation-tracks.json';previous=json.loads(previous_path.read_text())
assert (report['meshes'],report['bones'],report['linkups'],report['root_samples'],report['errors'])==(130,3111,225,8172,0)
animations={ (Path(o['file']).stem+'.'+o['object']).lower():o for o in previous['objects'] if 'sequences'in o }
cache={};files={};max_error=0.;sample_count=0;bone_count=0;link_count=0;seen=set()
def package(file):
    if file not in cache:
        data=Path(file).read_bytes();cache[file]=(data,tables(data));files[file]=hashlib.sha256(data).hexdigest()
    return cache[file]
def skip_array(r,stride):r.take(r.count()*stride)
def decode(key):
    a=[]
    for w in key:
        w&=0xfffe
        if w>=32768:w-=65536
        a.append(w*f(1/46339))
    d=math.sqrt(1-sum(x*x for x in a));a.append(-d if key[2]&1 else d)
    shift=(key[0]&1)+2*(key[1]&1);return a[shift:]+a[:shift]
def matrix(rotation,position):
    x,y,z,w=map(value,rotation);x2=f(x*2);y2=f(y*2);z2=f(z*2)
    xx=f(x*x2);yy=f(y2*y);zz=f(z2*z);wx=f(w*x2);wy=f(w*y2);wz=f(w*z2)
    xy=f(x*y2);xz=f(x*z2);yz=f(z2*y)
    return [bits(f(1-f(zz+yy))),bits(f(wz+xy)),bits(f(xz-wy)),0,bits(f(xy-wz)),bits(f(1-f(zz+xx))),bits(f(wx+yz)),0,bits(f(wy+xz)),bits(f(yz-wx)),bits(f(1-f(yy+xx))),0,*position,bits(1)]
track_cache={}
def original_tracks(animation,sequence):
    key=(animation['file'],animation['export_index'],sequence)
    if key in track_cache:return track_cache[key]
    data,(_,_,names,exports,object_path)=package(animation['file']);e=exports[animation['export_index']-1]
    r=Reader(data[e[5]:e[5]+e[4]],animation['sequences'][sequence]['metadata']['payload_offset'])
    r.u32();r.index()
    for _ in range(r.count()):r.index()
    r.take(8)
    for _ in range(r.count()):r.take(4);r.index();r.index()
    r.take(12+1+16);tracks=[]
    for _ in range(r.count()):
        scale=value(r.u32());pn=r.count();pos=[struct.unpack('<hhh',r.take(6)) for _ in range(pn)]
        rn=r.count();rot=[struct.unpack('<HHH',r.take(6)) for _ in range(rn)];dur=r.take(r.count());tracks.append((scale,pos,rot,dur))
    assert r.pos==animation['sequences'][sequence]['metadata']['end_offset'];track_cache[key]=tracks;return tracks
for o in report['objects']:
    data,(version,licensee,names,exports,object_path)=package(o['file']);index=o['export_index'];e=exports[index-1]
    assert o['object']==object_path(index) and object_path(e[0])=='Engine.SkeletalMesh';seen.add((o['file'],index))
    if version<151:assert o['status']=='UnsupportedLegacyPackage';continue
    p=o['prefix'];payload=data[e[5]:e[5]+e[4]];r=Reader(payload)
    assert not e[3]&0x02000000 and names[r.index()]=='None';assert r.pos==p['native_offset']
    r.take(41);lod=r.i32();assert lod==8==p['lod_version'];r.u32();skip_array(r,4)
    for _ in range(r.count()):object_path(r.index())
    r.take(36)
    for stride in [2,8,2,10,8]:skip_array(r,stride)
    r.take(28);object_path(r.index());r.take(52+4+16);skip_array(r,12)
    assert r.pos==p['bones_offset'];bones=[]
    for _ in range(r.count()):
        ni=r.index();b={'name':{'index':ni,'name':names[ni]},'flags':r.u32(),'rotation':[r.u32() for _ in range(4)],'position':[r.u32() for _ in range(3)]}
        for field in ('word_24','word_28_first','word_28','word_30'):b[field]=r.u32()
        b['word_38']=r.i32();b['word_34']=r.i32();bones.append(b)
    assert bones==p['bones'];bone_count+=len(bones);assert r.pos==p['linkups_offset'];links=[]
    for _ in range(r.count()):links.append({'word_0':r.u32() if version>=152 else 1,'animation_index':r.index(),'mapping':[]})
    assert links==p['linkups'];link_count+=len(links);assert r.pos==p['end_offset'] and len(payload)-r.pos==p['remaining_bytes']
    for li,m in zip(links,o['mappings']):
        animation_path=object_path(li['animation_index']);key=((Path(o['file']).stem+'.'+animation_path) if li['animation_index']>0 else animation_path).lower()
        assert key==m['animation'];animation=animations[key]
        mesh_names=[b['name']['name'].lower() for b in bones];anim_names=[b['name']['name'].lower() for b in animation['reference_bones']]
        mapping=[anim_names.index(n) if n in anim_names else -1 for n in mesh_names];matched=sum(i>=0 for i in mapping)
        assert m['mesh_names']==mesh_names and m['animation_names']==anim_names and m['mapping']==mapping
        assert m['refresh']=={'Rebuilt':{'matched':matched,'warning':not matched}}
        if not mapping or mapping[0]<0:
            assert not m['root_samples'];continue
        assert len(m['root_samples'])==3*len(animation['sequences'])
        for j,s in enumerate(m['root_samples']):
            sequence=j//3;metadata=animation['sequences'][sequence]['metadata'];assert s['sequence']==sequence and s['sequence_name']==metadata['name']['name']
            assert s['track']==mapping[0]>=0
            assert s['time_bits']==bits([0.,f(metadata['frames']*.5),f(metadata['frames'])][j%3])
            scale,pos,rot,dur=original_tracks(animation,sequence)[mapping[0]];time=value(s['time_bits']);res=time;current=0;alpha=None
            if len(dur)>1:
                for current,d in enumerate(dur):
                    after=f(res-d)
                    if after<0:break
                    res=after
                else:current=0;d=dur[0]
                if res>0:alpha=f(res/d)
            nxt=(current+1)%len(dur) if alpha is not None else current
            assert s['result']=={'Ok':{'current':current,'next':nxt,'alpha_bits':bits(alpha) if alpha is not None else None}}
            a=decode(rot[0 if len(rot)==1 else current]);b=decode(rot[0 if len(rot)==1 else nxt]);out=a
            if alpha is not None and len(rot)!=1:
                dot=sum(x*y for x,y in zip(a,b));absolute=abs(dot)
                if absolute>=value(0x3f0ccccd):
                    out=[x*(1-alpha)+y*alpha*(-1 if dot<0 else 1) for x,y in zip(a,b)];norm=math.sqrt(sum(x*x for x in out));out=[x/norm for x in out]
                else:
                    angle=math.acos(absolute);left=math.sin((1-alpha)*angle)/math.sin(angle);right=math.sin(alpha*angle)/math.sin(angle)
                    out=[x*left+y*right*(-1 if dot<0 else 1) for x,y in zip(a,b)]
            error=max(abs(x-value(y)) for x,y in zip(out,s['root']['rotation']));max_error=max(max_error,error);assert error<2e-6,(o['object'],sequence,error)
            ps=f(scale*value(0x38000100));a=[f(x*ps) for x in pos[0 if len(pos)==1 else current]];b=[f(x*ps) for x in pos[0 if len(pos)==1 else nxt]]
            out=[f(f(f(y-x)*alpha)+x) for x,y in zip(a,b)] if alpha is not None and len(pos)!=1 else a
            assert s['root']['position']==list(map(bits,out))
            assert s['matrix']==matrix(s['root']['rotation'],s['root']['position']);sample_count+=1
assert (bone_count,link_count,sample_count)==(3111,225,8172)
expected={(file,i) for file,(data,(_,_,_,exports,pathfn)) in cache.items() for i,e in enumerate(exports,1) if pathfn(e[0])=='Engine.SkeletalMesh'}
assert seen==expected
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
sources=['crates/rc-package/src/skeletal_mesh.rs','crates/rc-inspect/src/bin/rc-skeletal-linkup-check.rs','scripts/Record-SkeletalLinkups.py','scripts/Record-OriginalAnimationTracks.py','analysis/decompiled/skeletal-linkup-build.c','analysis/decompiled/skeletal-linkup-build.asm','analysis/decompiled/skeletal-reference-bones.c','analysis/decompiled/skeletal-prefix-serializers.c','analysis/decompiled/lod-prefix-arrays.c','analysis/decompiled/lod-prefix-elements.c','analysis/decompiled/skeletal-mesh-prefix.asm']
asm=(root/'analysis/decompiled/skeletal-linkup-build.asm').read_text()
for marker in ('10509456 JZ 0x10509534','10509492 MOV dword ptr [EAX + ESI*0x4],0xffffffff','105094c0 CMP EBX,dword ptr [EDX]','104ff1b8 PUSH EBX','104ff1bb CALL dword ptr [EAX + 0x4]'):assert marker in asm
result={'date':'2026-10-07','rust_tests':257,'meshes':130,'bones':3111,'resolved_linkups':225,'mapped_root_samples':8172,'legacy_meshes':1,'remaining_mesh_bytes':sum(o.get('prefix',{}).get('remaining_bytes',0) for o in report['objects']),'maximum_portable_rotation_error':max_error,'report_sha256':sha(path),'animation_report_sha256':sha(previous_path),'package_sha256':files,'source_sha256':{s:sha(root/s) for s in sources},'checks':['cargo test --workspace: 257 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','independent prefix byte reader and exact bone/linkup comparison','225 independent ordered name matches','8172 mapped root selections, exact position/matrix words and mathematical quaternion comparison, tolerance 2e-6'],'scope':report['scope']}
(root/'analysis/reports/original-skeletal-linkups-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';e=json.loads(path.read_text(encoding='utf-8-sig'));e['rust_tests']=257;e['original_skeletal_linkups_validation']=result;path.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: Originales Referenzskelett und native Track-Linkups ergänzt. 130 SkeletalMesh-Präfixe, '
      '3111 Knochen und 225 aufgelöste Animationsreferenzen unabhängig aus Paketbytes geprüft. Persistente Archive '
      'lassen transiente Mesh-/Mappingfelder aus; Linkups werden mit erstem Namenstreffer und nativer Längenprüfung '
      'aufgebaut. 8172 zum echten Root-Knochen zugeordnete Track-Samples: Schlüsselwahl, Positionen und Matrizen '
      'unabhängig geprüft, portable Quaternionen gegen mathematische Referenz. 257 Workspace-Tests, Clippy und '
      'Format bestanden. 47362213 Mesh-Tailbytes, Legacy-beast, Vollpose/Skinning offen; Android zum Schluss. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\ORIGINAL_SKELETAL_LINKUPS.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Originales Referenzskelett und native Track-Linkups ergänzt' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps({k:result[k] for k in ('rust_tests','meshes','bones','resolved_linkups','mapped_root_samples','maximum_portable_rotation_error')}))
