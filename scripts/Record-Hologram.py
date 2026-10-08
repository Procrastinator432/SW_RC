"""Independent original shader/color bindings and native wrapper call-state oracle."""
from pathlib import Path
import json,struct,re
root=Path(__file__).resolve().parents[1]
source=root/'scripts/Record-SkeletalMaterials.py';ns={'__file__':str(source)}
exec(compile(source.read_text().split('texture_map={}')[0],str(source),'exec'),ns)
f=ns['f'];sha=ns['sha'];Reader=ns['Reader'];bits=lambda v:struct.unpack('<I',struct.pack('<f',v))[0]
path=root/'analysis/reports/hologram-materials.json';report=json.loads(path.read_text())
assert sha(root/'analysis/reports/skeletal-materials.json')==json.loads((root/'analysis/reports/skeletal-materials-validation.json').read_text())['report_sha256']
engine=ns['game']/'System/engine.dll';pe=engine.read_bytes();h=int.from_bytes(pe[60:64],'little');opt=int.from_bytes(pe[h+20:h+22],'little');base=int.from_bytes(pe[h+52:h+56],'little')
for i in range(int.from_bytes(pe[h+6:h+8],'little')):
    at=h+24+opt+40*i;size,va,rawsize,off=struct.unpack_from('<IIII',pe,at+8);rva=0x10665594-base
    if va<=rva<va+max(size,rawsize):factor=struct.unpack_from('<f',pe,off+rva-va)[0];break
else:raise AssertionError('hologram color conversion constant not mapped')
assert bits(factor)==0x3b808081
def properties(data,meta,e):
    # Independent tagged reader retains every fixed-array index and full string.
    names=meta[2];r=Reader(data[e[5]:e[5]+e[4]]);out=[]
    while True:
        name=names[r.index()]
        if name=='None':return out,r.pos
        info=r.take(1)[0];kind=info&15;struct_name=names[r.index()] if kind==10 else None
        sc=(info>>4)&7;size=[1,2,4,12,16][sc] if sc<5 else int.from_bytes(r.take([1,2,4][sc-5]),'little')
        ai=0
        if info&128 and kind!=3:
            b=r.take(1)[0];ai=b if b<128 else ((b&63)<<8)|r.take(1)[0] if not b&64 else ((b&63)<<24)|int.from_bytes(r.take(3),'big')
        raw=r.take(size) if kind!=3 else b''
        if kind==5:value=Reader(raw).index()
        elif kind==3:value=bool(info&128)
        elif kind==13:
            text=Reader(raw);count=text.index()
            body=text.take(abs(count)*(2 if count<0 else 1));assert text.pos==len(raw)
            if count<0:assert body[-2:]==b'\0\0';value=body[:-2].decode('utf-16-le')
            elif count>0:assert body[-1:]==b'\0';value=body[:-1].decode('latin1')
            else:value=''
        else:value=raw
        out.append({'name':name,'slot':ai,'kind':kind,'struct':struct_name,'raw':raw,'value':value})
def first(props,name,slot=0):return next((p['value'] for p in props if p['name']==name and p['slot']==slot),None)
def ref(package,meta,index):return None if not index else package+'.'+meta[4](index) if index>0 else meta[4](index)
data,meta=ns['package'](ns['files']['engine']);e=next(e for i,e in enumerate(meta[3],1) if meta[4](i)=='HsHologram')
candidates=[]
for off in range(e[5]+e[4]-40,e[5]+e[4]):
    try:
        p,end=properties(data,meta,(0,0,0,0,e[5]+e[4]-off,off))
        if [v['name'] for v in p]==['HologramColor','ShaderImplementation'] and off+end==e[5]+e[4]:candidates.append((off,p))
    except Exception:pass
assert len(candidates)==1
defaults=candidates[0][1];default_color=int.from_bytes(first(defaults,'HologramColor'),'little');default_shader=ref('Engine',meta,first(defaults,'ShaderImplementation'))
assert default_shader=='HardwareShaders.Hologram.DynamicHologram'
old=ns['report'];holo=[b for o in old['objects'] for b in o['bindings'] if b['error']=='Unsupported material class Engine.HsHologram'];colors=[b for o in old['objects'] for b in o['bindings'] if b['error']=='Unsupported material class Engine.ConstantColor']
assert len(holo)==report['hologram_slots']==10 and len(colors)==report['constant_color_slots']==1
assert {b['material'] for b in holo}=={m['material'] for m in report['holograms']}
assert {b['material'] for b in colors}=={m['material'] for m in report['colors']}
def native_wrapper(w,s,host_color=None,host_t0=None):
    visible=dict(s);visible['color_words']=list(s['color_words'])
    if w['diffuse']:visible['texture1']=w['diffuse']
    if not w['use_marker_color']:visible['color_words']=[bits(f(((w['color']>>shift)&255)*factor)) for shift in (16,8,0)]+[bits(1.)]
    after=dict(s);after['color_words']=list(s['color_words']) if not w['use_marker_color'] or host_color is None else host_color
    after['texture1']=s['texture0']
    if host_t0 is not None:after['texture0']=host_t0
    wrapper=dict(w);wrapper['fallback']=visible['texture1']
    return visible,after,wrapper
for m in report['holograms']:
    pn,data,meta,e=ns['asset'](m['material']);assert meta[4](e[0])=='Engine.HsHologram';props,_=properties(data,meta,e)
    assert m['diffuse']==ref(pn,meta,first(props,'DiffuseTexture'))
    assert m['serialized_fallback']==ref(pn,meta,first(props,'FallbackMaterial'))
    color=first(props,'HologramColor');color=int.from_bytes(color,'little') if color is not None else default_color
    assert m['color']==color and m['use_marker_color']==bool(first(props,'UseMarkerColorInstead'))
    shader=ref(pn,meta,first(props,'ShaderImplementation')) or default_shader;assert shader==m['shader_implementation']
    w={'diffuse':1 if m['diffuse'] is not None else 0,'color':color,'use_marker_color':m['use_marker_color'],'fallback':99}
    visible,after,wrapper=native_wrapper(w,m['host_fixture_before'])
    assert m['callback']=={'shader':visible,'arguments':[0,0]};assert m['host_fixture_after']==after and m['wrapper_after']==wrapper and m['result']==1
for m in report['colors']:
    pn,data,meta,e=ns['asset'](m['material']);assert meta[4](e[0])=='Engine.ConstantColor';props,_=properties(data,meta,e)
    color=int.from_bytes(first(props,'Color'),'little');assert color==m['stored_color'] and m['samples']==[color]*3;assert color==0x00282b24
program_count=0;program_hashes={};texture_paths={m['diffuse'] for m in report['holograms'] if m['diffuse']}
for name,shader in report['shaders'].items():
    pn,data,meta,e=ns['asset'](name);assert meta[4](e[0])=='Engine.HardwareShader';props,_=properties(data,meta,e)
    programs={p['name']:p['value'] for p in props if p['name'].endswith('ShaderText')};assert programs==shader['programs'];program_count+=len(programs)
    for field,text in programs.items():
        output=root/'analysis/decompiled'/('hologram-'+field+'.asm');output.write_bytes(text.encode('latin1'));program_hashes[str(output.relative_to(root))]=sha(output)
    textures=[{'slot':p['slot'],'material':ref(pn,meta,p['value'])} for p in props if p['name']=='Textures'];assert textures==shader['textures']
    texture_paths.update(p['material'] for p in textures if p['material'])
assert texture_paths==set(report['textures'])
pixels=0
for name,t in report['textures'].items():
    original=ns['resolve'](name);assert original['uv_scale']==1.
    for key in ['source','chain','format','source_mip','width','height','pixels']:assert original[key]==t[key],key
    pixels+=len(t['pixels'])
for i,p in enumerate(report['probes']):
    visible,after,wrapper=native_wrapper(p['before'],p['shader_before'],[i]*4,0x4000+i)
    assert p['callback']=={'shader':visible,'arguments':[0,0]};assert p['shader_after']==after and p['after']==wrapper;assert p['result']==i%3-1
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/hologram-tests.log').read_text())));assert tests==526,tests
ns['hashes'][str(engine)]=sha(engine)
sources=['crates/rc-package/src/material_hologram.rs','crates/rc-inspect/src/assets.rs','crates/rc-inspect/src/bin/rc-hologram-check.rs','scripts/Record-Hologram.py','analysis/decompiled/material-hologram-color.c','analysis/decompiled/material-hologram-color.asm']
validation={'report_sha256':sha(path),'hologram_slots':10,'hologram_paths':len(report['holograms']),'constant_color_slots':1,'hardware_shader_programs':program_count,'input_texture_paths':len(texture_paths),'decoded_pixels':pixels,'wrapper_probes':len(report['probes']),'hologram_default_offset':candidates[0][0],'default_color':default_color,'default_shader':default_shader,'original_sha256':ns['hashes'],'source_sha256':{s:sha(root/s) for s in sources},'rust_tests':tests,'scope':report['scope'],'diffuse_coverage_unchanged':{'resolved':271,'omitted':18}}
validation['original_shader_program_sha256']=program_hashes
(root/'analysis/reports/hologram-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
e=root/'analysis/evidence.json';evidence=json.loads(e.read_text());evidence['rust_tests']=tests;evidence['hologram_validation']=validation;e.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-08: Drei Hologramm-/Farb-Aufgaben abgeschlossen: nativer HsHologram-Wrapper mit Farb-/Diffusebindung und beobachtetem Rueckschreiben von +8ac nach +8b0, ConstantColor-Wortkopie ohne Alpha-Aenderung und Original-Shader-/Texturbindungskatalog. '
      'Zehn Hologramm-Slots / sieben Materialpfade, ein ConstantColor-Slot, drei Hardware-Shaderprogramme und 1024 Wrapper-Proben unabhaengig geprueft. 526 Tests, Clippy und Format bestanden. '
      'Shaderprogramme noch nicht ausgefuehrt; Diffuse-Abdeckung bleibt 271/289, kein voller Hologrammeffekt behauptet. Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\MATERIAL_HOLOGRAM.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for target in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Drei Hologramm-/Farb-Aufgaben abgeschlossen' not in target.read_text(encoding='utf-8-sig'):
        with target.open('a',encoding='utf-8',newline='\n') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps({k:validation[k] for k in ['hologram_slots','hardware_shader_programs','input_texture_paths','decoded_pixels','wrapper_probes','rust_tests']}))
