"""Independent original-program formula, register trace and fixture pixel oracle."""
from pathlib import Path
import json,struct,re,math
from PIL import Image
root=Path(__file__).resolve().parents[1]
s=root/'scripts/Record-Hologram.py';ns={'__file__':str(s)}
exec(compile(s.read_text().split("data,meta=ns['package']")[0],str(s),'exec'),ns)
f=ns['f'];sha=ns['sha'];bits=ns['bits'];original=ns['ns'];props=ns['properties']
path=root/'analysis/reports/pixel-shader.json';report=json.loads(path.read_text())
assert sha(root/'analysis/reports/hologram-materials.json')==json.loads((root/'analysis/reports/hologram-validation.json').read_text())['report_sha256']
pn,data,meta,e=original['asset'](report['shader']);p,_=props(data,meta,e)
source=ns['first'](p,'PixelShaderText');assert source==report['source']
lines=[re.sub(r'\s*,\s*',',',line.split(';')[0].strip()) for line in source.splitlines()];lines=[line for line in lines if line]
assert lines==['ps.1.1','tex t0','tex t1','tex t2','dp3_sat r1,t1,c0','mul r0,r1,t0','mul r0.rgb,r0,t2','+mov r0.a,t0.a','mul r0.rgb,r0,v0','mul r0,r0,c1']
assert report['instructions']==9 and report['blocks']==8
constants=[[0.]*4 for _ in range(8)]
for item in p:
    if item['name']!='PSConstants':continue
    nested,_=props(item['raw'],meta,(0,0,0,0,len(item['raw']),0))
    assert ns['first'](nested,'Type')==b'\1'
    raw=ns['first'](nested,'Value');plane,_=props(raw,meta,(0,0,0,0,len(raw),0))
    constants[item['slot']]=[struct.unpack('<f',ns['first'](plane,n) or b'\0'*4)[0] for n in ['X','Y','Z','W']]
assert [[bits(v) for v in c] for c in constants]==[[bits(v) for v in c] for c in report['constants']]
def oracle(textures,c,v,trace=False):
    t=[[f(x) for x in row] for row in textures];c=[[f(x) for x in row] for row in c];v=[min(1.,max(0.,f(x))) for x in v]
    r=[[0.]*4 for _ in range(2)];defined=[[False]*4 for _ in range(2)];states=[]
    def save():states.append({'words':[[bits(x) for x in row] for row in r],'defined':[row[:] for row in defined]})
    for _ in range(3):save()
    luma=min(1.,max(0.,f(f(f(t[1][0]*c[0][0])+f(t[1][1]*c[0][1]))+f(t[1][2]*c[0][2]))))
    r[1]=[luma]*4;defined[1]=[True]*4;save()
    r[0]=[f(luma*x) for x in t[0]];defined[0]=[True]*4;save()
    r[0]=[f(r[0][i]*t[2][i]) for i in range(3)]+[t[0][3]];save()
    r[0]=[f(r[0][i]*v[i]) for i in range(3)]+[r[0][3]];save()
    r[0]=[f(r[0][i]*c[1][i]) for i in range(4)];save()
    return (r[0],states) if trace else r[0]
for i,probe in enumerate(report['probes']):
    inp=probe['input'];t=[[f(((i*37+j*53+k*19)%256)/255.) for k in range(4)] for j in range(3)]+[[0.]*4]
    assert [[bits(x) for x in row] for row in inp['textures']]==[[bits(x) for x in row] for row in t]
    c=[row[:] for row in constants];c[1]=[f(0.2),f(0.4),f(0.8),f(((i*7)%256)/255.)]
    v=[-0.25,f(((i*11)%256)/255.),1.25,1.]
    assert [[bits(x) for x in row] for row in inp['constants']]==[[bits(x) for x in row] for row in c]
    assert [bits(x) for x in inp['colors'][0]]==[bits(x) for x in v]
    color,states=oracle(t,c,v,True)
    assert probe['evaluation']['trace']==states,i
    assert [bits(x) for x in probe['evaluation']['color']]==[bits(x) for x in color],i
assert len(report['probes'])==1024
for name,texture in report['textures'].items():
    decoded=original['resolve'](name)
    for key in ['width','height','pixels']:assert texture[key]==decoded[key]
def sample(t,uv):
    x=min(int(f(f(uv[0]%1.)*t['width'])),t['width']-1);y=min(int(f(f(uv[1]%1.)*t['height'])),t['height']-1)
    pixel=t['pixels'][y*t['width']+x]
    return [f(((pixel>>shift)&255)/255.) for shift in (16,8,0,24)]
preview_hashes={};pixel_count=0
parents={h['material']:h for h in ns['report']['holograms'] if h['diffuse']}
assert len(report['previews'])==len(parents)*2
assert {(p['material'],p['phase']) for p in report['previews']}=={(name,phase) for name in parents for phase in (0.,0.5)}
for index,preview in enumerate(report['previews']):
    h=parents[preview['material']];assert preview['textures']==['HardwareShaders.Gradients.GreyHoloFade',h['diffuse'],'HardwareShaders.Hologram.NoiseDigital']
    c=[row[:] for row in constants];c[1]=[f(((h['color']>>s)&255)*ns['factor']) for s in (16,8,0)]+[1.]
    assert [[bits(x) for x in row] for row in preview['constants']]==[[bits(x) for x in row] for row in c]
    phase=f(preview['phase']);assert phase in (0.,0.5);assert preview['width']==preview['height']==128
    textures=[report['textures'][name] for name in preview['textures']]
    expected=[]
    for y in range(128):
        for x in range(128):
            u=f((x+0.5)/128.);v=f((y+0.5)/128.)
            uv=[[u,0.5],[u,v],[f(f(u*2.)+f(phase*f(.17))),f(f(v*2.)+f(phase*f(.07)))]]
            color=oracle([sample(t,uv[i]) for i,t in enumerate(textures)],c,[f(.8)]*4)
            pixel=0xff000000
            for i,shift in enumerate((16,8,0)):
                dest=f(((0xff101820>>shift)&255)/255.)
                channel=min(1.,max(0.,f(f(color[i]*color[3])+dest)))
                pixel|=math.floor(f(channel*255.)+0.5)<<shift
            expected.append(pixel)
    assert expected==preview['pixels'],index
    image=Image.new('RGB',(128,128));image.putdata([tuple((p>>s)&255 for s in (16,8,0)) for p in expected])
    output=root/f'analysis/reports/pixel-preview-{index:02}.png';image.save(output);preview_hashes[str(output.relative_to(root))]=sha(output);pixel_count+=len(expected)
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/pixel-tests.log').read_text())));assert tests==533,tests
sources=['crates/rc-package/src/pixel_shader.rs','crates/rc-package/src/properties.rs','crates/rc-inspect/src/assets.rs','crates/rc-render/src/fragment.rs','crates/rc-inspect/src/bin/rc-pixel-check.rs','scripts/Record-PixelShader.py']
validation={'report_sha256':sha(path),'input_catalog_sha256':sha(root/'analysis/reports/hologram-materials.json'),'original_sha256':original['hashes'],'source_sha256':{s:sha(root/s) for s in sources},'probe_count':len(report['probes']),'register_snapshots':len(report['probes'])*8,'preview_count':len(report['previews']),'preview_pixels':pixel_count,'preview_sha256':preview_hashes,'rust_tests':tests,'scope':report['scope'],'diffuse_coverage_unchanged':{'resolved':271,'omitted':18}}
(root/'analysis/reports/pixel-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
e=root/'analysis/evidence.json';evidence=json.loads(e.read_text());evidence['rust_tests']=tests;evidence['pixel_shader_validation']=validation;e.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({k:validation[k] for k in ['probe_count','register_snapshots','preview_count','preview_pixels','rust_tests']}))
