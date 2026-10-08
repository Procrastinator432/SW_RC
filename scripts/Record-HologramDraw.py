"""Independent original mesh/shader outputs and diagnostic triangle image oracle."""
from pathlib import Path
import json,struct,math,re
from PIL import Image
root=Path(__file__).resolve().parents[1]
s=root/'scripts/Record-VertexShader.py';n={'__file__':str(s)}
exec(compile(s.read_text().split('textures=[]')[0],str(s),'exec'),n)
f=n['f'];bits=n['bits'];sha=n['sha'];original=n['original'];algorithm=n['algorithm']
path=root/'analysis/reports/hologram-draw.json';report=json.loads(path.read_text())
binary=root/'analysis/reports/skeletal-draw.bin';v=json.loads((root/'analysis/reports/skeletal-draw-validation.json').read_text());assert sha(binary)==v['binary_sha256']
source=[]
with binary.open('rb') as stream:
    assert stream.read(12)==b'RCSKDRAW\1\0\0\0'
    while record:=stream.read(132):
        assert len(record)==132
        words=struct.unpack('<33I',record)
        if words[:2]==(25,0):source.append(list(words[9:]))
assert source==report['source_words'] and len(source)==3500
as_float=lambda w:struct.unpack('<f',struct.pack('<I',w))[0]
vertices=[list(map(as_float,t[i:i+8])) for t in source for i in (0,8,16)]
minimum=[min(v[i] for v in vertices) for i in range(3)];maximum=[max(v[i] for v in vertices) for i in range(3)]
center=[f(f(minimum[i]+maximum[i])*.5) for i in range(3)];radius=max(f(f(maximum[i]-minimum[i])*.5) for i in range(3))
assert list(map(bits,center))==list(map(bits,report['center'])) and bits(radius)==bits(report['radius'])
paths=['HardwareShaders.Gradients.GreyHoloFade','CloneTextures.CloneTextures.CloneCommandoSmall','HardwareShaders.Hologram.NoiseDigital'];assert paths==report['texture_paths']
textures=[original['resolve'](p) for p in paths];pc=[[f(x) for x in row] for row in report['pixel_constants']]
serialized_state={key:n['ns']['first'](n['props'],key) for key in ['ZTest','ZWrite','AlphaBlending','AlphaTest','AlphaRef','SrcBlend','DestBlend']}
assert serialized_state=={'ZTest':True,'ZWrite':None,'AlphaBlending':True,'AlphaTest':True,'AlphaRef':None,'SrcBlend':b'\5','DestBlend':b'\2'}
old_pixel=json.loads((root/'analysis/reports/pixel-shader.json').read_text());assert [[bits(x) for x in row] for row in pc]==[[bits(x) for x in row] for row in old_pixel['constants']]
def sample(t,u,v):
    x=min(int(f(f(u%1.)*t['width'])),t['width']-1);y=min(int(f(f(v%1.)*t['height'])),t['height']-1)
    p=t['pixels'][y*t['width']+x];return [f(((p>>s)&255)/255.) for s in (16,8,0,24)]
def fragment(v,destination):
    t=[sample(textures[0],v[0],0.),sample(textures[1],v[1],v[2]),sample(textures[2],v[3],v[4])]
    luma=min(1.,max(0.,f(f(f(t[1][0]*pc[0][0])+f(t[1][1]*pc[0][1]))+f(t[1][2]*pc[0][2]))))
    color=[f(f(f(f(luma*t[0][i])*t[2][i])*min(1.,max(0.,v[5+i])))*pc[1][i]) for i in range(3)]+[f(t[0][3]*pc[1][3])]
    color=[min(1.,max(0.,x)) for x in color]
    if color[3]<=0.:return None
    result=0xff000000
    for i,s in enumerate((16,8,0)):
        channel=min(1.,max(0.,f(f(color[i]*color[3])+f(((destination>>s)&255)/255.))))
        result|=math.floor(f(channel*255.)+.5)<<s
    return result
def planes(p):
    x,y,z,w=p;return [w-1e-6,w+x,w-x,w+y,w-y,z,w-z]
def clip(triangle):
    polygon=[(v['clip'],v['varying']) for v in triangle]
    for plane in range(7):
        out=[]
        if not polygon:break
        prev=polygon[-1];pd=planes(prev[0])[plane]
        for current in polygon:
            cd=planes(current[0])[plane]
            if (pd>=0.)!=(cd>=0.):
                a=pd/(pd-cd)
                out.append(tuple([x+a*(y-x) for x,y in zip(prev[part],current[part])] for part in (0,1)))
            if cd>=0.:out.append(current)
            prev=current;pd=cd
        polygon=out
    return polygon
def edge(a,b,p):return (b[0]-a[0])*(p[1]-a[1])-(b[1]-a[1])*(p[0]-a[0])
def top_left(a,b):return b[1]<a[1] or b[1]==a[1] and b[0]>a[0]
def image(projected,width,height):
    pixels=[0xff101820]*(width*height);stats=[0]*4
    for triangle in projected:
        polygon=clip(triangle)
        for corner in range(1,len(polygon)-1):
            vs=[polygon[0],polygon[corner],polygon[corner+1]]
            p=[((v[0][0]/v[0][3]+1.)*width*.5,(1.-v[0][1]/v[0][3])*height*.5) for v in vs]
            area=edge(p[0],p[1],p[2])
            if area==0.:continue
            if area<0.:vs[1],vs[2]=vs[2],vs[1];p[1],p[2]=p[2],p[1];area=-area
            iw=[1./v[0][3] for v in vs]
            for y in range(max(0,math.floor(min(v[1] for v in p))),min(height,math.ceil(max(v[1] for v in p)))):
                for x in range(max(0,math.floor(min(v[0] for v in p))),min(width,math.ceil(max(v[0] for v in p)))):
                    q=(x+.5,y+.5);pairs=[(p[1],p[2]),(p[2],p[0]),(p[0],p[1])];es=[edge(a,b,q) for a,b in pairs]
                    if any(e<0. or e==0. and not top_left(a,b) for e,(a,b) in zip(es,pairs)):continue
                    stats[0]+=1;b=[e/area for e in es]
                    z=min(1.,max(0.,f((b[0]*vs[0][0][2]*iw[0]+b[1]*vs[1][0][2]*iw[1])+b[2]*vs[2][0][2]*iw[2])))
                    assert 0.<=z<=1.
                    denominator=(b[0]*iw[0]+b[1]*iw[1])+b[2]*iw[2]
                    varying=[f(((b[0]*vs[0][1][i]*iw[0]+b[1]*vs[1][1][i]*iw[1])+b[2]*vs[2][1][i]*iw[2])/denominator) for i in range(10)]
                    index=y*width+x;result=fragment(varying,pixels[index])
                    if result is None:stats[2]+=1
                    else:pixels[index]=result;stats[3]+=1
    return pixels,stats
frame_hashes={};total_vertices=0
assert [frame['phase'] for frame in report['frames']]==[0.,.5]
for index,frame in enumerate(report['frames']):
    phase=f(frame['phase']);c=[[0.]*4 for _ in range(96)]
    for b in n['bindings']:
        if b['kind']==1:c[b['slot']]=b['value'][:]
    c[0]=[0.,f(.9),0.,0.];c[1]=[0.,0.,f(.9),0.];c[2]=[f(.4),0.,0.,.5];c[3]=[f(.3),0.,0.,1.]
    for start in (5,11):
        for i in range(4):c[start+i][i]=1.
    c[10]=[4.,0.,0.,1.];c[16]=[2.25,f(.7),f(.2),0.];c[17]=[phase]*4;c[21]=[1.]*4
    c[22]=[f(f(.9)+f(phase*f(.05))),f(.01),f(.02),0.];c[25]=[f(f(.3)+f(phase*f(.1))),f(.2),f(.1),0.]
    assert [[bits(x) for x in row] for row in c]==[[bits(x) for x in row] for row in frame['constants']]
    expected_projected=[]
    for t in range(3500):
        tri=[]
        for v in vertices[t*3:t*3+3]:
            inputs=[[0.]*4 for _ in range(16)];inputs[0]=[f(f(v[i]-center[i])/radius) for i in range(3)]+[1.];inputs[1]=v[3:6]+[0.];inputs[2]=v[6:8]+[0.,1.]
            output,defined,_=algorithm(inputs,c,False)
            tri.append({'clip':output[0],'varying':[output[3][0],*output[4][:2],*output[5][:2],*output[1],output[11][0]]})
        expected_projected.append(tri);total_vertices+=3
    assert expected_projected==frame['projected'],index
    assert frame['width']==frame['height']==128
    pixels,stats=image(expected_projected,128,128);assert stats==frame['stats'],(index,stats,frame['stats'])
    assert pixels==frame['pixels'],index;assert frame['depth_words']==[bits(1.)]*16384
    output=root/f'analysis/reports/hologram-mesh-{index}.png';im=Image.new('RGB',(128,128));im.putdata([tuple((p>>s)&255 for s in (16,8,0)) for p in pixels]);im.save(output);frame_hashes[str(output.relative_to(root))]=sha(output)
assert len(set(frame_hashes.values()))==2
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/hologram-draw-tests.log').read_text())));assert tests==544,tests
sources=['crates/rc-render/src/shader_raster.rs','crates/rc-inspect/src/bin/rc-hologram-draw-check.rs','scripts/Record-HologramDraw.py','scripts/Record-VertexShader.py']
validation={'report_sha256':sha(path),'mesh_binary_sha256':sha(binary),'original_sha256':original['hashes'],'source_sha256':{s:sha(root/s) for s in sources},'diagnostic_state':{'depth_test':True,'depth_write':False,'depth_compare':'less_equal','alpha_reference':0,'alpha_compare':'greater_float','blend':'source_alpha_additive'},'triangles_per_frame':3500,'frames':2,'verified_vertex_outputs':total_vertices,'image_pixels':32768,'preview_sha256':frame_hashes,'rust_tests':tests,'scope':report['scope'],'diffuse_coverage_unchanged':{'resolved':271,'omitted':18}}
(root/'analysis/reports/hologram-draw-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
e=root/'analysis/evidence.json';evidence=json.loads(e.read_text());evidence['rust_tests']=tests;evidence['hologram_draw_validation']=validation;e.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({k:validation[k] for k in ['triangles_per_frame','frames','verified_vertex_outputs','image_pixels','rust_tests']}))
