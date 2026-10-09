"""Independent original package/vertex/raster oracle for snapshot-driven mesh draws."""
from pathlib import Path
import json,struct,re,hashlib
from PIL import Image
root=Path(__file__).resolve().parents[1]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=root/'scripts/Record-HologramDraw.py';n={'__file__':str(source)}
exec(compile(source.read_text().split('frame_hashes={}')[0],str(source),'exec'),n)
f=n['f'];bits=n['bits'];original=n['original'];algorithm=n['algorithm'];first=n['n']['ns']['first'];props=n['n']['props'];meta=n['n']['meta']
path=root/'analysis/reports/hardware-draw.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/hardware-draw-release.json').read_bytes()
assert report['shader']=='HardwareShaders.Hologram.DynamicHologram'
assert report['source_words']==n['source'] and report['center']==n['center'] and report['radius']==n['radius'] and report['fit_scale']==1.5
inputs=report['inputs'];assert inputs['vertex_source']==first(props,'VertexShaderText') and inputs['pixel_source']==first(props,'PixelShaderText')
# Independently read serialized constant arrays and texture references.
s=root/'scripts/Record-ShaderSnapshots.py';context={'__file__':str(s)}
exec(compile(s.read_text().split('\npath=root/')[0],str(s),'exec'),context)
assert inputs['vertex_constants']==context['vs'] and inputs['pixel_constants']==context['ps']
ref=n['n']['ns']['ref']
textures=[{'slot':p['slot'],'material':ref('HardwareShaders',meta,p['value'])} for p in props if p['name']=='Textures']
assert inputs['textures']==textures
paths=[next(t['material'] for t in textures if t['slot']==i) for i in range(3)]
assert report['texture_paths']==paths==['HardwareShaders.Gradients.GreyHoloFade','HardwareShaders.Gradients.GreyHoloFade','HardwareShaders.Hologram.NoiseDigital']
n['textures']=[original['resolve'](p) for p in paths]
snapshot_path=root/'analysis/reports/shader-snapshots.json';snapshots=json.loads(snapshot_path.read_text())
validation=json.loads((root/'analysis/reports/shader-snapshots-validation.json').read_text())
assert sha(snapshot_path)==validation['report_sha256'] and sha(root/'analysis/reports/shader-snapshots.input.json')==validation['input_sha256']
assert (report['successful_updates'],report['rejected_updates'])==(128,128)
assert [frame['snapshot'] for frame in report['frames']]==[251,255]
# The new native call-site evidence establishes PS-first, shared scratch and upload ranges.
asm=(root/'analysis/decompiled/hardware-shader-setup.asm').read_text()
for marker in ['1000dc77 PUSH 0x60','1000dc7b PUSH 0x10083e48','1000dcad LEA EAX,[ESI + 0x80c]',
               '1000dcbb CALL 0x1001c130','1000dcda CALL dword ptr [EDX + 0x16c]',
               '1000dcf1 LEA EAX,[ESI + 0x8c]','1000dcf8 PUSH 0x10083e48',
               '1000dcff CALL 0x1001c130','1000dd1e CALL dword ptr [EDX + 0x13c]',
               '1000dd86 JZ 0x1000de9e','1000dd9e JZ 0x1000de9e',
               '1000dddb AND EDX,0x1ffff80','1000ddf1 JNP 0x1000de5d',
               '1000ddf6 AND EAX,0xffff7fff','1000ddfb OR EAX,0x17000',
               '1000de46 MOV dword ptr [ESI + 0x6c],EDX',
               '1000de58 MOV dword ptr [ESI + 0x68],EAX',
               '1000de60 AND EDX,0xfffe2fff','1000de66 OR EDX,0x2000']:
    assert marker in asm,marker
# Direct3D 8, not the earlier imprecise D3D9 label: imports are read from original PE.
dll=context['scene']['dll'];raw=dll.read_bytes();assert b'd3d8.dll' in raw.lower() and b'Direct3DCreate8' in raw
import copy
scratch={'count':0,'words':[[0]*4 for _ in range(96)]};scratch['words'][17]=[bits(.25)]*4
pixel={'count':0,'words':[[0]*4 for _ in range(8)]}
vertex={'count':0,'words':copy.deepcopy(scratch['words'])}
counts={'pixel':-3,'vertex':-2};flicker={'last_time':None,'samples':[.5]*3}
assert len(report['probes'])==256
verified_words=0;errors=0
for i,p in enumerate(report['probes']):
    assert p['id']==i
    before={'device':{'scratch':scratch,'pixel':pixel,'vertex':vertex},'counts':counts,'flicker':flicker}
    assert p['before']==before,(i,'before')
    q=snapshots['probes'][i]['pipeline']
    # Native PS dispatch writes c0/c1 before every VS dispatch, then uploads two planes.
    for b in context['ps']:
        assert b['kind']==1;scratch['words'][b['slot']]=list(map(bits,b['value']))
    pixel={'count':2,'words':copy.deepcopy(pixel['words'])};pixel['words'][:2]=copy.deepcopy(scratch['words'][:2])
    # Original VS starts by writing all c0..3, so the previously validated instruction
    # replay equals scratch after this particular material (including partial errors).
    scratch=copy.deepcopy(q['vertex']);counts={'pixel':2,'vertex':32}
    if 'Ok' in q['status']:
        vertex={'count':32,'words':copy.deepcopy(vertex['words'])}
        vertex['words'][:32]=copy.deepcopy(scratch['words'][:32])
    else:errors+=1
    flicker=q['flicker']
    assert p['device']=={'scratch':scratch,'pixel':pixel,'vertex':vertex},(i,'device')
    assert p['counts']==counts and p['flicker']==flicker and p['status']==q['status']
    assert p['rng']==q['rng'] and p['inverse_calls']==q['inverse_calls']
    verified_words+=(96+96+8)*4*2
assert errors==128
# Independent raw-word post-resolver stage oracle: all 256 masks, capacities, failure stops.
assert len(report['stage_probes'])==768
bound_counts=set();transform_branches=set();stage_words=0
for q in report['stage_probes']:
    mask=q['mask'];variant=q['variant'];limit=mask%9 if variant==1 else 8;fail=mask%8 if variant==2 else 8
    assert (q['limit'],q['fail'])==(limit,fail)
    stages=copy.deepcopy(q['initial']);calls=[];bound=0
    for j in range(limit):
        if mask&(1<<j)==0:break
        calls.append(j)
        if j==fail:break
        stage=stages[j];transform=q['transforms'][j]
        stage[4]=(stage[4]&~0x1ffff80)|((j<<7)&0x1ffff80)
        zero=(transform[0]&0x7fffffff)==0
        transform_branches.add('zero' if zero else ('nan' if transform[0]==0x7fc12345 else 'nonzero'))
        if zero:stage[2]=(stage[2]&0xfffe2fff)|0x2000
        else:
            stage[2]=(stage[2]&0xffff7fff)|0x17000
            stage[22:28]=[transform[0],0,0,transform[0],transform[2],transform[1]]
        bound+=1
    assert q['stages']==stages and q['calls']==calls and q['bound']==bound
    bound_counts.add(bound);stage_words+=8*28
assert bound_counts==set(range(9)) and transform_branches=={'zero','nan','nonzero'}
assert report['bound_stages']==3 and report['resolved']==[[i,p] for i,p in enumerate(paths)]
assert report['stages']==[[0,0,0x2000,0,i<<7]+[0]*23 if i<3 else [0]*28 for i in range(8)]
total=0;previews={};writes=[]
for index,frame in enumerate(report['frames']):
    reference=snapshots['probes'][frame['snapshot']]['pipeline']
    device=report['probes'][frame['snapshot']]['device']
    assert frame['vertex_bank']==device['vertex'] and frame['pixel_bank']==device['pixel'] and frame['flicker']==reference['flicker']
    c=[[context['value'](x) for x in row] for row in frame['vertex_bank']['words']]
    n['pc']=[[context['value'](x) for x in row] for row in frame['pixel_bank']['words']]
    expected=[]
    for t in range(3500):
        tri=[]
        for vertex in n['vertices'][t*3:t*3+3]:
            values=[[0.]*4 for _ in range(16)];values[0]=[f(f(f(vertex[i]-n['center'][i])/n['radius'])*1.5) for i in range(3)]+[1.];values[1]=vertex[3:6]+[0.];values[2]=vertex[6:8]+[0.,1.]
            output,defined,_=algorithm(values,c,False)
            tri.append({'clip':output[0],'varying':[output[3][0],*output[4][:2],*output[5][:2],*output[1],output[11][0]]})
            total+=1
        expected.append(tri)
    assert frame['projected']==expected,(index,'vertices')
    assert frame['width']==frame['height']==128
    pixels,stats=n['image'](expected,128,128)
    assert frame['stats']==stats and frame['pixels']==pixels,(index,'raster')
    assert frame['depth_words']==[bits(1.)]*16384 and stats[3]>0
    writes.append(stats[3]);output=root/f'analysis/reports/hardware-mesh-{index}.png'
    im=Image.new('RGB',(128,128));im.putdata([tuple((p>>shift)&255 for shift in (16,8,0)) for p in pixels]);im.save(output);previews[str(output.relative_to(root))]=sha(output)
assert len(set(previews.values()))==2
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/hardware-draw-tests.log').read_text())));assert tests==596,tests
sources=['crates/rc-package/src/hardware_constants.rs','crates/rc-package/src/hardware_stages.rs','analysis/decompiled/hardware-shader-setup.asm','crates/rc-inspect/src/assets.rs','crates/rc-inspect/src/bin/rc-hardware-draw-check.rs','crates/rc-render/src/hologram_pass.rs','crates/rc-render/src/shader_raster.rs','crates/rc-render/src/fragment.rs','crates/rc-package/src/shader_snapshot.rs','scripts/Record-HardwareDraw.py','scripts/Record-HologramDraw.py','scripts/Record-ShaderSnapshots.py','scripts/Record-VertexShader.py']
result={'date':'2026-10-09','snapshots':256,'expected_dispatch_errors':errors,'verified_device_words':verified_words,'stage_probes':768,'verified_stage_words':stage_words,'bound_stage_counts':sorted(bound_counts),'frames':2,'triangles_per_frame':3500,'verified_vertex_outputs':total,'verified_image_pixels':32768,'written_fragments':writes,'texture_paths':paths,'debug_release_identical':True,'rust_tests':tests,'scope':report['scope'],'report_sha256':sha(path),'snapshot_report_sha256':sha(snapshot_path),'preview_sha256':previews,'source_sha256':{s:sha(root/s) for s in sources},'original_d3ddrv_sha256':sha(dll),'original_sha256':original['hashes'],'android':'deferred until end per user'}
(root/'analysis/reports/hardware-draw-validation.json').write_text(json.dumps(result,indent=2)+'\n')
ledger=root/'analysis/evidence.json';evidence=json.loads(ledger.read_text(encoding='utf-8'));evidence['rust_tests']=tests;evidence['hardware_draw_validation']=result;ledger.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({k:result[k] for k in ['snapshots','stage_probes','verified_device_words','frames','verified_vertex_outputs','verified_image_pixels','written_fragments','rust_tests']}))
