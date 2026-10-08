"""Independent original TexPanner properties, sine table, SSE phases and snapshots."""
from pathlib import Path
import json,struct,math,re
from PIL import Image,ImageChops
root=Path(__file__).resolve().parents[1]
source=root/'scripts/Record-SkeletalMaterials.py';ns={'__file__':str(source)}
exec(compile(source.read_text().split('texture_map={}')[0],str(source),'exec'),ns)
f=ns['f'];sha=ns['sha'];bits=lambda v:struct.unpack('<I',struct.pack('<f',v))[0]
report_path=root/'analysis/reports/panner-materials.json';report=json.loads(report_path.read_text())
baseline=root/'analysis/reports/skeletal-materials.json'
assert sha(baseline)==json.loads((root/'analysis/reports/skeletal-materials-validation.json').read_text())['report_sha256']
regression=root/'analysis/reports/panner2d-regression.json'
assert sha(regression)==sha(root/'analysis/reports/panner2d-materials.json')==json.loads((root/'analysis/reports/panner2d-validation.json').read_text())['report_sha256']
def pe_constant(file,address,fmt):
    data=file.read_bytes();h=int.from_bytes(data[60:64],'little');opt=int.from_bytes(data[h+20:h+22],'little')
    base=struct.unpack_from('<I',data,h+24+28)[0];rva=address-base
    for i in range(int.from_bytes(data[h+6:h+8],'little')):
        at=h+24+opt+40*i;size,va,rawsize,off=struct.unpack_from('<IIII',data,at+8)
        if va<=rva<va+max(size,rawsize):return struct.unpack_from(fmt,data,off+rva-va)[0]
    raise AssertionError('PE address not mapped')
core=ns['game']/'System/core.dll';engine=ns['game']/'System/engine.dll'
step=pe_constant(core,0x101885d4,'<f');assert bits(step)==0x39c90fdb
magic=pe_constant(engine,0x10673b00,'<d');assert magic==103079215104.
factor=pe_constant(engine,0x10673bd8,'<f');size=pe_constant(engine,0x10673bd4,'<f')
assert factor==1/1024 and size==1024.
# Integer times the original float constant is exact in binary64 (<=38 bits).
# Independent libm evaluation checks every x87-generated stored f32 entry.
table=[f(math.sin(i*step)) for i in range(16384)]
table_path=root/'crates/rc-package/src/material_sine_table.bin'
assert list(struct.unpack('<16384I',table_path.read_bytes()))==list(map(bits,table))
def signed(v):return (v+2**31)%2**32-2**31
def direction(rotation):
    pitch,yaw,_=rotation
    cosine=table[(signed(pitch+16384)>>2)&16383]
    return [f(table[(signed(yaw+16384)>>2)&16383]*cosine),f(table[(yaw>>2)&16383]*cosine),table[(pitch>>2)&16383]]
def matrix(p,time):
    d=direction(p['direction']);rate=f(p['rate']);time=f(time);out=[]
    for v in d[:2]:
        phase=f(f(f(rate*v)*time)*factor)
        raw=struct.unpack('<i',struct.pack('<d',phase+magic)[:4])[0]
        out.append(f(f(phase-f(raw>>16))*size))
    return {'scale':[1.,1.],'offset':out}
def same(a,b):
    assert a.keys()==b.keys()
    for key in a:assert list(map(bits,a[key]))==list(map(bits,b[key])),(a,b)
data,meta=ns['package'](ns['files']['engine'])
e=next(e for i,e in enumerate(meta[3],1) if meta[4](i)=='TexPanner')
required={'PanRate':f(0.1)};candidates=[]
for off in range(max(e[5],e[5]+e[4]-128),e[5]+e[4]):
    fake=(0,0,0,0,e[5]+e[4]-off,off)
    try:
        props,r=ns['properties'](data,meta,fake)
        if props==required and r.pos==fake[4]:candidates.append((off,props))
    except Exception:pass
assert len(candidates)==1,candidates
bindings=[b for o in ns['report']['objects'] for b in o['bindings'] if b['error']=='Unsupported material class Engine.TexPanner']
assert len(bindings)==report['resolved_slots']==5
assert {b['material'] for b in bindings}=={m['material'] for m in report['materials']}
pixels=0;samples=0;snapshot_hashes={};image_hashes={}
positions=[[[0.,-1.,-1.],[0.,1.,-1.],[0.,-1.,1.]],[[0.,1.,-1.],[0.,1.,1.],[0.,-1.,1.]]]
uvs=[[[0.,0.],[1.,0.],[0.,1.]],[[1.,0.],[1.,1.],[0.,1.]]]
for m in report['materials']:
    path=m['material'];chain=[]
    while True:
        pn,data,meta,e=ns['asset'](path);cls=meta[4](e[0]);chain.append(cls)
        props,_=ns['properties'](data,meta,e)
        if cls=='Engine.TexPanner':break
        assert cls in ('Engine.Shader','Engine.FinalBlend')
        ref=props['Diffuse' if cls=='Engine.Shader' else 'Material'];path=meta[4](ref)
        if ref>0:path=pn+'.'+path
    p={'direction':list(struct.unpack('<iii',props['PanDirection'])) if 'PanDirection' in props else [0,0,0],'rate':props.get('PanRate',required['PanRate'])}
    assert p['direction']==m['parameters']['direction'];assert bits(p['rate'])==bits(m['parameters']['rate'])
    assert list(map(bits,direction(p['direction'])))==list(map(bits,m['direction']))
    ref=props['Material'];path=meta[4](ref)
    if ref>0:path=pn+'.'+path
    diffuse=ns['resolve'](path);assert diffuse['uv_scale']==1.
    assert chain+diffuse['chain']==m['chain']
    for key in ['source','format','source_mip','width','height','pixels']:assert m[key]==diffuse[key],key
    pixels+=len(m['pixels'])
    for sample in m['samples']:
        t=matrix(p,sample['time']);same(t,sample['transform']);samples+=1
        snapshot=Path(sample['snapshot']);b=snapshot.read_bytes();snapshot_hashes[str(snapshot)]=sha(snapshot)
        assert b[:4]==b'RCSC' and struct.unpack_from('<III',b,4)==(2,2,1)
        for i in range(2):
            words=struct.unpack_from('<17I',b,16+68*i)
            assert list(words[:9])==[bits(v) for point in positions[i] for v in point]
            assert words[9]==words[16]==0
            expected=[f(f(v*t['scale'][axis])+t['offset'][axis]) for pair in uvs[i] for axis,v in enumerate(pair)]
            assert list(words[10:16])==list(map(bits,expected))
        w,h=struct.unpack_from('<II',b,152);assert [w,h]==[m['width'],m['height']]
        assert list(struct.unpack_from('<'+'I'*(w*h),b,160))==m['pixels'];assert len(b)==160+w*h*4
        ppm=Path(sample['ppm']);im=Image.open(ppm);png=ppm.with_suffix('.png');im.save(png)
        bbox=ImageChops.difference(im,Image.new('RGB',im.size,im.getpixel((0,0)))).getbbox()
        assert bbox and bbox[0]>0 and bbox[1]>0 and bbox[2]<256 and bbox[3]<256,bbox
        assert Image.open(png).tobytes()==im.tobytes();image_hashes[str(png)]=sha(png)
for probe in report['probes']:
    assert list(map(bits,direction(probe['parameters']['direction'])))==list(map(bits,probe['direction']))
    same(matrix(probe['parameters'],probe['time']),probe['transform'])
log=(root/'analysis/reports/panner-tests.log').read_text();tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)))
assert tests==510,tests
ns['hashes'].update({str(core):sha(core),str(engine):sha(engine)})
sources=['crates/rc-package/src/material_panner.rs','crates/rc-package/src/material_sine_table.bin','crates/rc-inspect/src/assets.rs','crates/rc-inspect/src/bin/rc-panner-check.rs','scripts/Generate-MaterialSineTable.rs','scripts/Record-Panner.py','analysis/decompiled/material-panners.c','analysis/decompiled/material-panner.asm','analysis/decompiled/material-rotator-vector.c','analysis/decompiled/material-sine-table.asm']
validation={'report_sha256':sha(report_path),'resolved_slots':5,'material_paths':len(report['materials']),'original_samples':samples,'numerical_probes':len(report['probes']),'sine_table_entries':len(table),'sine_step_bits':hex(bits(step)),'decoded_pixels':pixels,'class_defaults':required,'class_default_file_offset':candidates[0][0],'snapshot_sha256':snapshot_hashes,'preview_sha256':image_hashes,'distinct_preview_images':len(set(image_hashes.values())),'original_sha256':ns['hashes'],'source_sha256':{s:sha(root/s) for s in sources},'rust_tests':tests,'scope':report['scope']}
validation['panner2d_regression_sha256']=sha(regression)
validation['combined_used_slots']={'total':289,'resolved':269,'omitted':20}
assert ns['report']['resolved_slots']==212 and ns['report']['omitted_slots']==77
assert sum(len(o['bindings']) for o in ns['report']['objects'])==289
(root/'analysis/reports/panner-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
e=root/'analysis/evidence.json';evidence=json.loads(e.read_text());evidence['rust_tests']=tests;evidence['panner_validation']=validation;e.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-08: Drei TexPanner-Aufgaben abgeschlossen: native quantisierte Richtungsberechnung mit 16384 reproduzierten Sinustabellenwerten, periodische UV-Verschiebung und Originalmaterial-/Rendereranbindung. '
      'Fuenf weitere bisher ausgelassene Slots aufgeloest; 520 Richtungs-/Zeitproben und vier Originalmaterial-Snapshots unabhaengig geprueft. '
      '510 Tests, Clippy und Format bestanden. Diagnose-Materialflaechen, keine laufende Spielintegration; Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\MATERIAL_PANNER.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for target in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Drei TexPanner-Aufgaben abgeschlossen' not in target.read_text(encoding='utf-8-sig'):
        with target.open('a',encoding='utf-8',newline='\n') as stream:stream.write('\n\n'+note+'\n')
    # Preserve existing bytes; normalize only this note if an earlier run appended CRLF.
    data=target.read_bytes();suffix=note.encode('utf-8')+b'\r\n'
    if data.endswith(suffix):target.write_bytes(data[:-len(suffix)].rstrip(b'\r\n')+b'\n\n'+note.encode('utf-8')+b'\n')
print(json.dumps({k:validation[k] for k in ['resolved_slots','sine_table_entries','numerical_probes','decoded_pixels','distinct_preview_images','rust_tests']}))
