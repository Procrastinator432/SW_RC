"""Separate package/property/float/pixel/snapshot oracle for TexPanner2D."""
from pathlib import Path
import hashlib,json,struct,collections
root=Path(__file__).resolve().parents[1]
source=root/'scripts/Record-SkeletalMaterials.py';ns={'__file__':str(source)}
exec(compile(source.read_text().split('texture_map={}')[0],str(source),'exec'),ns)
f=ns['f'];bits=lambda x:struct.unpack('<I',struct.pack('<f',x))[0]
sha=ns['sha'];report_path=root/'analysis/reports/panner2d-materials.json'
report=json.loads(report_path.read_text())
# Locate and independently parse the complete class default tag suffix.
data,meta=ns['package'](ns['files']['engine'])
e=next(e for i,e in enumerate(meta[3],1) if meta[4](i)=='TexPanner2D')
required={'SpeedU':0.5,'SpeedV':0.5,'ScaleU':1.,'ScaleV':1.,'ClampedSizeU':1.,'ClampedSizeV':1.}
candidates=[]
for off in range(max(e[5],e[5]+e[4]-128),e[5]+e[4]):
    fake=(0,0,0,0,e[5]+e[4]-off,off)
    try:
        props,r=ns['properties'](data,meta,fake)
        if props==required and r.pos==fake[4]:candidates.append((off,props))
    except (Exception,AssertionError):pass
assert len(candidates)==1,candidates
defaults=candidates[0][1]
pe_path=ns['game']/'System/engine.dll';pe=pe_path.read_bytes();header=int.from_bytes(pe[60:64],'little')
sections=int.from_bytes(pe[header+6:header+8],'little');opt=int.from_bytes(pe[header+20:header+22],'little')
for i in range(sections):
    at=header+24+opt+40*i;size,va,rawsize,off=struct.unpack_from('<IIII',pe,at+8)
    rva=0x10673b00-0x10300000
    if va<=rva<va+max(size,rawsize):assert struct.unpack_from('<d',pe,off+rva-va)[0]==103079215104.;break
else:raise AssertionError('native rounding constant not mapped')
ns['hashes'][str(pe_path)]=sha(pe_path)
def parameters(props):
    values=dict(defaults);values.update(props)
    return {group:[values.get(prefix+axis,0.) for axis in 'UV'] for group,prefix in [('speed','Speed'),('offset','Offset'),('scale','Scale'),('clamped_size','ClampedSize')]}
def matrix(p,time):
    # Explicit SSE float stores; x87 FADD/FSTP to binary64, signed SAR extraction.
    time=f(time);out=[]
    for i in range(2):
        speed,size,offset=f(p['speed'][i]),f(p['clamped_size'][i]),f(p['offset'][i])
        phase=f(f(speed/size)*time)
        raw=struct.unpack('<i',struct.pack('<d',phase+103079215104.)[:4])[0]
        integral=raw>>16
        value=f(f(f(phase-f(integral))*size)+offset)
        if size>1. and value>f(size-1.):value=f(value-size)
        out.append(value)
    return {'scale':p['scale'],'offset':out}
def same(a,b):
    assert a.keys()==b.keys()
    for key in a:assert [bits(v) for v in a[key]]==[bits(v) for v in b[key]],(a,b)
old=ns['report'];bindings=[b for o in old['objects'] for b in o['bindings'] if b['error']=='Unsupported material class Engine.TexPanner2D']
assert len(bindings)==report['resolved_slots']==52
assert {b['material'] for b in bindings}=={m['material'] for m in report['materials']}
pixels=0;sample_count=0;snapshots={}
for m in report['materials']:
    path=m['material'];chain=[]
    while True:
        pn,data,meta,e=ns['asset'](path);cls=meta[4](e[0]);chain.append(cls)
        props,_=ns['properties'](data,meta,e)
        if cls=='Engine.TexPanner2D':break
        assert cls in ('Engine.Shader','Engine.FinalBlend')
        ref=props['Diffuse' if cls=='Engine.Shader' else 'Material'];path=meta[4](ref)
        if ref>0:path=pn+'.'+path
    p=parameters(props)
    for key in p:assert [bits(v) for v in p[key]]==[bits(v) for v in m['parameters'][key]]
    ref=props['Material'];path=meta[4](ref)
    if ref>0:path=pn+'.'+path
    diffuse=ns['resolve'](path)
    assert chain+diffuse['chain']==m['chain'];assert diffuse['uv_scale']==1.
    for key in ['source','format','source_mip','width','height','pixels']:assert m[key]==diffuse[key],key
    pixels+=len(m['pixels'])
    for sample in m['samples']:same(matrix(p,sample['time']),sample['transform']);sample_count+=1
    snapshot=Path(m['snapshot']);b=snapshot.read_bytes();snapshots[str(snapshot)]=sha(snapshot)
    assert b[:4]==b'RCSC';assert struct.unpack_from('<III',b,4)==(2,1,1)
    words=struct.unpack_from('<17I',b,16);assert words[9]==0 and words[16]==0
    t=matrix(p,0.)
    uv=[f(f(v*t['scale'][axis])+t['offset'][axis]) for pair in [[0.,0.],[1.,0.],[-0.25,1.25]] for axis,v in enumerate(pair)]
    assert list(words[10:16])==list(map(bits,uv))
    w,h=struct.unpack_from('<II',b,84);assert [w,h]==[m['width'],m['height']]
    assert list(struct.unpack_from('<'+'I'*(w*h),b,92))==m['pixels'];assert len(b)==92+w*h*4
for probe in report['probes']:same(matrix(probe['parameters'],probe['time']),probe['transform'])
log=(root/'analysis/reports/panner2d-tests.log').read_text()
tests=sum(map(int,__import__('re').findall(r'test result: ok\. (\d+) passed',log)))
assert tests==504,tests
validation={'report_sha256':sha(report_path),'class_default_file_offset':candidates[0][0],'class_defaults':defaults,'resolved_slots':52,'material_paths':len(report['materials']),'sampled_matrices':sample_count,'numerical_probes':len(report['probes']),'decoded_pixels':pixels,'snapshot_sha256':snapshots,'original_sha256':ns['hashes'],'source_sha256':{str(p.relative_to(root)):sha(p) for p in [root/'crates/rc-package/src/material_uv.rs',root/'crates/rc-inspect/src/assets.rs',root/'crates/rc-render/src/skeletal.rs',root/'crates/rc-inspect/src/bin/rc-panner2d-check.rs',Path(__file__),root/'analysis/decompiled/material-panners.c',root/'analysis/decompiled/material-panner2d.asm']},'rust_tests':tests,'scope':report['scope']}
output=root/'analysis/reports/panner2d-validation.json';output.write_text(json.dumps(validation,indent=2)+'\n')
evidence_path=root/'analysis/evidence.json';evidence=json.loads(evidence_path.read_text());evidence['rust_tests']=tests;evidence['panner2d_validation']=validation;evidence_path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-08: Drei TexPanner2D-Aufgaben abgeschlossen: native UV-Matrixberechnung, Engine.u-Klassenvorgaben/Instanzparameter und zeitabhaengiger Diffuse-Resolver mit Rendererbindung. '
      '52 zuvor ausgelassene Slots / 15 Materialpfade aufgeloest; 417792 Originalpixel, 90 Zeitproben, 265 numerische Grenz-/Bewegungsproben und 15 Material-Snapshots unabhaengig geprueft. '
      'Die gespeicherten HUD-Panner haben Geschwindigkeit null; Script-Aenderungen sind noch nicht angebunden. 504 Tests, Clippy und Format bestanden. '
      'Weitere Wrapper/Effekte, Modifier-Komposition und laufende Spielintegration offen; Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\MATERIAL_PANNER2D.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for target in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Drei TexPanner2D-Aufgaben abgeschlossen' not in target.read_text(encoding='utf-8-sig'):
        with target.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps({k:validation[k] for k in ['resolved_slots','material_paths','sampled_matrices','numerical_probes','decoded_pixels','rust_tests']}))
