"""Independent original package/vertex/raster oracle for snapshot-driven mesh draws."""
from pathlib import Path
import json,struct,re,hashlib
from PIL import Image
root=Path(__file__).resolve().parents[1]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=root/'scripts/Record-HologramDraw.py';n={'__file__':str(source)}
exec(compile(source.read_text().split('frame_hashes={}')[0],str(source),'exec'),n)
f=n['f'];bits=n['bits'];original=n['original'];algorithm=n['algorithm'];first=n['n']['ns']['first'];props=n['n']['props'];meta=n['n']['meta']
path=root/'analysis/reports/snapshot-draw.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/snapshot-draw-release.json').read_bytes()
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
total=0;previews={};writes=[]
for index,frame in enumerate(report['frames']):
    reference=snapshots['probes'][frame['snapshot']]['pipeline']
    assert frame['vertex_bank']==reference['vertex'] and frame['pixel_bank']==reference['pixel'] and frame['flicker']==reference['flicker']
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
    writes.append(stats[3]);output=root/f'analysis/reports/snapshot-mesh-{index}.png'
    im=Image.new('RGB',(128,128));im.putdata([tuple((p>>shift)&255 for shift in (16,8,0)) for p in pixels]);im.save(output);previews[str(output.relative_to(root))]=sha(output)
assert len(set(previews.values()))==2
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/snapshot-draw-tests.log').read_text())));assert tests==589,tests
sources=['crates/rc-inspect/src/assets.rs','crates/rc-inspect/src/bin/rc-snapshot-draw-check.rs','crates/rc-render/src/hologram_pass.rs','crates/rc-render/src/shader_raster.rs','crates/rc-render/src/fragment.rs','crates/rc-package/src/shader_snapshot.rs','scripts/Record-SnapshotDraw.py','scripts/Record-HologramDraw.py','scripts/Record-ShaderSnapshots.py','scripts/Record-VertexShader.py']
result={'date':'2026-10-09','frames':2,'triangles_per_frame':3500,'verified_vertex_outputs':total,'verified_image_pixels':32768,'written_fragments':writes,'texture_paths':paths,'debug_release_identical':True,'rust_tests':tests,'scope':report['scope'],'report_sha256':sha(path),'snapshot_report_sha256':sha(snapshot_path),'preview_sha256':previews,'source_sha256':{s:sha(root/s) for s in sources},'original_sha256':original['hashes'],'android':'deferred until end per user'}
(root/'analysis/reports/snapshot-draw-validation.json').write_text(json.dumps(result,indent=2)+'\n')
ledger=root/'analysis/evidence.json';evidence=json.loads(ledger.read_text(encoding='utf-8'));evidence['rust_tests']=tests;evidence['snapshot_draw_validation']=result;ledger.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({k:result[k] for k in ['frames','verified_vertex_outputs','verified_image_pixels','written_fragments','rust_tests']}))
