"""Independent OT_Jitter tagged properties, scalar state transitions, RNG calls and pixels."""
from pathlib import Path
import json,struct,math,re
from PIL import Image,ImageChops
root=Path(__file__).resolve().parents[1]
source=root/'scripts/Record-SkeletalMaterials.py';ns={'__file__':str(source)}
exec(compile(source.read_text().split('texture_map={}')[0],str(source),'exec'),ns)
f=ns['f'];sha=ns['sha'];bits=lambda v:struct.unpack('<I',struct.pack('<f',v))[0]
report_path=root/'analysis/reports/jitter-materials.json';report=json.loads(report_path.read_text())
regressions={}
for current,original,validation in [('panner-jitter-regression.json','panner-materials.json','panner-validation.json'),('panner2d-jitter-regression.json','panner2d-materials.json','panner2d-validation.json')]:
    digest=sha(root/'analysis/reports'/current)
    assert digest==sha(root/'analysis/reports'/original)==json.loads((root/'analysis/reports'/validation).read_text())['report_sha256']
    regressions[current]=digest
assert sha(root/'analysis/reports/skeletal-materials.json')==json.loads((root/'analysis/reports/skeletal-materials-validation.json').read_text())['report_sha256']
engine=ns['game']/'System/engine.dll';pe=engine.read_bytes();h=int.from_bytes(pe[60:64],'little');opt=int.from_bytes(pe[h+20:h+22],'little');base=int.from_bytes(pe[h+52:h+56],'little')
for i in range(int.from_bytes(pe[h+6:h+8],'little')):
    at=h+24+opt+40*i;size,va,rawsize,off=struct.unpack_from('<IIII',pe,at+8);rva=0x10664f44-base
    if va<=rva<va+max(size,rawsize):factor=struct.unpack_from('<f',pe,off+rva-va)[0];break
else:raise AssertionError('jitter rand scaling constant not mapped')
assert bits(factor)==0x38000100
data,meta=ns['package'](ns['files']['engine']);e=next(e for i,e in enumerate(meta[3],1) if meta[4](i)=='TexOscillator')
required={'UOscillationRate':1.,'VOscillationRate':1.,'UOscillationAmplitude':f(0.1),'VOscillationAmplitude':f(0.1)};candidates=[]
for off in range(max(e[5],e[5]+e[4]-128),e[5]+e[4]):
    fake=(0,0,0,0,e[5]+e[4]-off,off)
    try:
        props,r=ns['properties'](data,meta,fake)
        if props==required and r.pos==fake[4]:candidates.append((off,props))
    except Exception:pass
assert len(candidates)==1,candidates
def parameters(props):
    v=dict(required);v.update(props)
    return {'rate':[v.get(a+'OscillationRate',0.) for a in 'UV'],'phase':[v.get(a+'OscillationPhase',0.) for a in 'UV'],
            'amplitude':[v.get(a+'OscillationAmplitude',0.) for a in 'UV'],'kind':[v.get(a+'OscillationType',0) for a in 'UV'],'pivot':[v.get(a+'Offset',0.) for a in 'UV']}
def same(a,b):
    assert a.keys()==b.keys()
    for key in a:assert list(map(bits,a[key]))==list(map(bits,b[key])),(a,b)
def step(p,state,time,sequence,cursor):
    assert p['kind']==[3,3] and p['pivot']==[0.,0.]
    s=[f(f(rate)*f(time)) for rate in p['rate']]
    last=list(map(f,state['last']));current=list(map(f,state['current']))
    for axis in range(2):
        reset=f(f(math.floor(s[axis]))+f(p['phase'][axis]))
        # Native V uses LastSu, including U writes earlier in this invocation.
        if last[axis]<1. or last[0]>f(s[axis]+1.):last[axis]=reset
        if f(s[axis]-last[axis])>1.:
            value=sequence[cursor];cursor+=1
            current[axis]=f(f(value*f(p['amplitude'][axis]))*factor);last[axis]=reset
    return {'last':last,'current':current},{'scale':[1.,1.],'offset':current},cursor
bindings=[b for o in ns['report']['objects'] for b in o['bindings'] if b['error']=='Unsupported material class Engine.TexOscillator']
assert len(bindings)==report['resolved_slots']==2
assert {b['material'] for b in bindings}=={m['material'] for m in report['materials']}
pixels=0;transitions=0;random_calls=0;snapshots={};images={}
positions=[[[0.,-1.,-1.],[0.,1.,-1.],[0.,-1.,1.]],[[0.,1.,-1.],[0.,1.,1.],[0.,-1.,1.]]]
uvs=[[[0.,0.],[1.,0.],[0.,1.]],[[1.,0.],[1.,1.],[0.,1.]]]
for m in report['materials']:
    path=m['material'];chain=[]
    while True:
        pn,data,meta,e=ns['asset'](path);cls=meta[4](e[0]);chain.append(cls);props,_=ns['properties'](data,meta,e)
        if cls=='Engine.TexOscillator':break
        assert cls in ('Engine.Shader','Engine.FinalBlend')
        ref=props['Diffuse' if cls=='Engine.Shader' else 'Material'];path=meta[4](ref)
        if ref>0:path=pn+'.'+path
    key=path.lower();p=parameters(props);same(p,m['parameters'])
    state={'last':[props.get('LastSu',0.),props.get('LastSv',0.)],'current':[props.get('CurrentUJitter',0.),props.get('CurrentVJitter',0.)]}
    initial=state.copy();ref=props['Material'];path=meta[4](ref)
    if ref>0:path=pn+'.'+path
    diffuse=ns['resolve'](path);assert diffuse['uv_scale']==1. and chain+diffuse['chain']==m['chain']
    for field in ['source','format','source_mip','width','height','pixels']:assert m[field]==diffuse[field],field
    pixels+=len(m['pixels']);cursor=0
    for frame,sample in enumerate(m['samples']):
        assert sample['random_start']==cursor
        if frame==0:assert sample['before']=={}
        else:assert set(sample['before'])=={key};same(state,sample['before'][key])
        state,t,cursor=step(p,state,sample['time'],m['random_fixture'],cursor)
        same(state,sample['after'][key]);same(t,sample['transform']);assert sample['random_end']==cursor
        assert set(sample['after'])=={key}
        snapshot=Path(sample['snapshot']);b=snapshot.read_bytes();snapshots[str(snapshot)]=sha(snapshot)
        assert b[:4]==b'RCSC' and struct.unpack_from('<III',b,4)==(2,2,1)
        for i in range(2):
            words=struct.unpack_from('<17I',b,16+68*i)
            assert list(words[:9])==[bits(v) for point in positions[i] for v in point];assert words[9]==words[16]==0
            expected=[f(f(v)+t['offset'][axis]) for pair in uvs[i] for axis,v in enumerate(pair)]
            assert list(words[10:16])==list(map(bits,expected))
        w,h=struct.unpack_from('<II',b,152);assert [w,h]==[m['width'],m['height']]
        assert list(struct.unpack_from('<'+'I'*(w*h),b,160))==m['pixels'] and len(b)==160+w*h*4
        ppm=Path(sample['ppm']);im=Image.open(ppm);png=ppm.with_suffix('.png');im.save(png)
        bbox=ImageChops.difference(im,Image.new('RGB',im.size,im.getpixel((0,0)))).getbbox()
        assert bbox and bbox[0]>0 and bbox[1]>0 and bbox[2]<256 and bbox[3]<256
        assert Image.open(png).tobytes()==im.tobytes();images[str(png)]=sha(png)
    assert [s['random_end'] for s in m['samples']]==[0,1,1,2]
    assert Path(m['samples'][1]['ppm']).read_bytes()==Path(m['samples'][2]['ppm']).read_bytes()
    random_calls+=cursor
for probe in report['probes']:
    state=probe['initial'];cursor=0
    for sample in probe['steps']:
        same(state,sample['before']);assert sample['random_start']==cursor
        state,t,cursor=step(probe['parameters'],state,sample['time'],probe['random_fixture'],cursor)
        same(state,sample['after']);same(t,sample['transform']);assert sample['random_end']==cursor;transitions+=1
    random_calls+=cursor
assert transitions==3072
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/jitter-tests.log').read_text())));assert tests==517,tests
ns['hashes'][str(engine)]=sha(engine)
sources=['crates/rc-package/src/material_jitter.rs','crates/rc-inspect/src/assets.rs','crates/rc-inspect/src/bin/rc-jitter-check.rs','scripts/Record-Jitter.py','analysis/decompiled/material-oscillator.asm','analysis/decompiled/material-panners.c']
validation={'report_sha256':sha(report_path),'resolved_slots':2,'material_paths':len(report['materials']),'state_transitions':transitions,'random_calls_checked':random_calls,'decoded_pixels':pixels,'original_initial_state':initial,'class_defaults':required,'class_default_file_offset':candidates[0][0],'snapshot_sha256':snapshots,'preview_sha256':images,'distinct_previews':len(set(images.values())),'original_sha256':ns['hashes'],'source_sha256':{s:sha(root/s) for s in sources},'rust_tests':tests,'scope':report['scope'],'combined_used_slots':{'resolved':271,'omitted':18,'total':289}}
validation['panner_regression_sha256']=regressions
(root/'analysis/reports/jitter-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
e=root/'analysis/evidence.json';evidence=json.loads(e.read_text());evidence['rust_tests']=tests;evidence['jitter_validation']=validation;e.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-08: Drei TexOscillator-Jitter-Aufgaben abgeschlossen: native Zustandsuebergaenge einschliesslich Floor-Rundung und V/U-Abhaengigkeit, Host-Schnittstelle fuer geteilte CRT-Zufallswerte und persistente Originalmaterial-/Rendererbindung. '
      'Zwei weitere Slots aufgeloest; 3072 Zustandsuebergaenge, Zufallsverbrauch und vier Material-Snapshots unabhaengig geprueft. 517 Tests, Clippy und Format bestanden. '
      'Zufallswerte sind reproduzierbare Pruefsequenzen, keine rekonstruierte globale Spiel-Zufallsfolge; Diagnose-Diffuse, keine laufende Spielintegration. Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\MATERIAL_JITTER.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for target in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Drei TexOscillator-Jitter-Aufgaben abgeschlossen' not in target.read_text(encoding='utf-8-sig'):
        with target.open('a',encoding='utf-8',newline='\n') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps({k:validation[k] for k in ['resolved_slots','state_transitions','random_calls_checked','decoded_pixels','distinct_previews','rust_tests']}))
