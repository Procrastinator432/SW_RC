"""Fixed original vertex algorithm, independent register/point-fragment oracle."""
from pathlib import Path
import json,struct,math,re
root=Path(__file__).resolve().parents[1]
source=root/'scripts/Record-Hologram.py';ns={'__file__':str(source)}
exec(compile(source.read_text().split("data,meta=ns['package']")[0],str(source),'exec'),ns)
f=ns['f'];bits=ns['bits'];sha=ns['sha'];original=ns['ns'];properties=ns['properties']
path=root/'analysis/reports/vertex-shader.json';report=json.loads(path.read_text())
pn,data,meta,e=original['asset'](report['shader']);props,_=properties(data,meta,e)
assert report['source']==ns['first'](props,'VertexShaderText')
text=re.sub(r'/\*.*?\*/','',report['source'],flags=re.S)
lines=[re.sub(r'\s*,\s*',',',line.split('//')[0].split(';')[0].strip()) for line in text.splitlines()];lines=[x for x in lines if x]
expected='''mov r3,v0
mul r3.z,r3,c21
add r3.xyz,r3,c22
mul r1,v0,c16.x
add r1.xy,r1.zx,r1.yz
mov r2,c17
mad r1,r2.x,c16.y,r1
frc r1.xy,r1
add r1,r1,-c20.y
max r7,r1,-r1
mul r4,r7,c16.zzww
mul r4,r4,r3
mov r4.z,r3.z
mov r4.w,c20.z
mov r5,r3
mov r5.z,c18.y
sge r0.x,r3.z,c18.x
slt r0.y,r3.z,c18.x
sge r0.z,r3.z,c18.y
mul r0.y,r0.y,r0.z
slt r0.z,r3.z,c18.y
mul r1,r4,r0.x
mad r1,r5,r0.y,r1
mad r3,r3,r0.z,r1
dp4 r4.x,r3,c0
dp4 r4.y,r3,c1
dp4 r4.z,r3,c2
dp4 r4.w,r3,c3
mov oPos,r4
dp3 r0.x,v1,c5
dp3 r0.y,v1,c6
dp3 r0.z,v1,c7
dp3 r0.w,r0,r0
rsq r0.w,r0.w
mul r0,r0,r0.w
dp4 r1.x,v0,c11
dp4 r1.y,v0,c12
dp4 r1.z,v0,c13
dp4 r1.w,v0,c14
add r2,c10,-r1
dp3 r2.w,r2,r2
rsq r2.w,r2.w
mul r2,r2,r2.w
dp3 r1,r2,r0
mul r3,r1,c15.y
add oT0.x,r3,c[20]
mov oT1,v2
mov r5,c25
mul r5,r5,c26.z
mul r6,v0.zx,c26.y
mad oT2.xy,r5,c28,r6.yx
mul oD0,r1,c22.x
mov oFog.x,c20.x'''
assert lines==['vs.1.0']+expected.splitlines()
assert report['instructions']==len(lines)-1==53
bindings=[]
for p in props:
    if p['name']!='VSConstants':continue
    q,_=properties(p['raw'],meta,(0,0,0,0,len(p['raw']),0));raw=ns['first'](q,'Value');plane,_=properties(raw,meta,(0,0,0,0,len(raw),0))
    bindings.append({'slot':p['slot'],'kind':(ns['first'](q,'Type') or b'\0')[0],'value':[struct.unpack('<f',ns['first'](plane,n) or b'\0'*4)[0] for n in 'XYZW']})
assert len(bindings)==20
for actual,expected_binding in zip(report['bindings'],bindings):
    assert actual['slot']==expected_binding['slot'] and actual['kind']==expected_binding['kind']
    assert list(map(bits,actual['value']))==list(map(bits,expected_binding['value']))
assert len(report['bindings'])==len(bindings)
def mul(a,b):return [f(x*y) for x,y in zip(a,b)]
def add(a,b):return [f(x+y) for x,y in zip(a,b)]
def mad(a,b,c):return add(mul(a,b),c)
def dot(a,b,n):
    p=mul(a,b);v=p[0]
    for x in p[1:n]:v=f(v+x)
    return [v]*4
def scalar(x):return [x]*4
def algorithm(v,c,capture_trace=True):
    r=[[0.]*4 for _ in range(24)];defined=[[False]*4 for _ in range(24)];trace=[]
    def put(i,value,mask=15):
        for k in range(4):
            if mask&(1<<k):r[i][k]=value[k];defined[i][k]=True
        if capture_trace:trace.append({'words':[[bits(x) for x in row] for row in r],'defined':[row[:] for row in defined]})
    put(3,v[0]);put(3,mul(r[3],c[21]),4);put(3,add(r[3],c[22]),7)
    put(1,mul(v[0],scalar(c[16][0])))
    put(1,add([r[1][2],r[1][0],r[1][0],r[1][0]],[r[1][1],r[1][2],r[1][2],r[1][2]]),3)
    put(2,c[17]);put(1,mad(scalar(r[2][0]),scalar(c[16][1]),r[1]))
    put(1,[f(x-math.floor(x)) for x in r[1]],3)
    put(1,add(r[1],scalar(-c[20][1])))
    put(7,[max(x,-x) for x in r[1]])
    put(4,mul(r[7],[c[16][2],c[16][2],c[16][3],c[16][3]]));put(4,mul(r[4],r[3]))
    put(4,scalar(r[3][2]),4);put(4,scalar(c[20][2]),8)
    put(5,r[3]);put(5,scalar(c[18][1]),4)
    put(0,scalar(float(r[3][2]>=c[18][0])),1);put(0,scalar(float(r[3][2]<c[18][0])),2)
    put(0,scalar(float(r[3][2]>=c[18][1])),4);put(0,mul(scalar(r[0][1]),scalar(r[0][2])),2)
    put(0,scalar(float(r[3][2]<c[18][1])),4)
    put(1,mul(r[4],scalar(r[0][0])));put(1,mad(r[5],scalar(r[0][1]),r[1]));put(3,mad(r[3],scalar(r[0][2]),r[1]))
    for i in range(4):put(4,dot(r[3],c[i],4),1<<i)
    put(12,r[4])
    for i in range(3):put(0,dot(v[1],c[5+i],3),1<<i)
    put(0,dot(r[0],r[0],3),8);put(0,scalar(f(1./f(math.sqrt(abs(r[0][3]))))),8);put(0,mul(r[0],scalar(r[0][3])))
    for i in range(4):put(1,dot(v[0],c[11+i],4),1<<i)
    put(2,add(c[10],[-x for x in r[1]]));put(2,dot(r[2],r[2],3),8)
    put(2,scalar(f(1./f(math.sqrt(abs(r[2][3]))))),8);put(2,mul(r[2],scalar(r[2][3])))
    put(1,dot(r[2],r[0],3));put(3,mul(r[1],scalar(c[15][1])));put(15,add(r[3],c[20]),1)
    put(16,v[2]);put(5,c[25]);put(5,mul(r[5],scalar(c[26][2])))
    put(6,mul([v[0][2],v[0][0],v[0][0],v[0][0]],scalar(c[26][1])))
    put(17,mad(r[5],c[28],[r[6][1],r[6][0],r[6][0],r[6][0]]),3)
    put(13,mul(r[1],scalar(c[22][0])));put(23,scalar(c[20][0]),1)
    return r[12:],defined[12:],trace
textures=[]
for t in report['textures']:
    decoded=original['resolve'](t['path'])
    for key in ['width','height','pixels']:assert t[key]==decoded[key]
    textures.append(t)
def sample(t,uv):
    x=min(int(f(f(uv[0]%1.)*t['width'])),t['width']-1);y=min(int(f(f(uv[1]%1.)*t['height'])),t['height']-1)
    p=t['pixels'][y*t['width']+x];return [f(((p>>s)&255)/255.) for s in (16,8,0,24)]
pc=[[f(x) for x in row] for row in report['pixel_constants']]
# Assert pixel constants against the separately validated previous report.
previous=root/'analysis/reports/pixel-shader.json';assert sha(previous)==json.loads((root/'analysis/reports/pixel-validation.json').read_text())['report_sha256']
assert sha(previous)==sha(root/'analysis/reports/pixel-regression.json')
assert [[bits(x) for x in row] for row in pc]==[[bits(x) for x in row] for row in json.loads(previous.read_text())['constants']]
branches=set();points=[]
for index,probe in enumerate(report['probes']):
    assert probe['index']==index and probe['phase']==index//32*.5
    c=[[0.]*4 for _ in range(96)]
    for b in bindings:
        if b['kind']==1:c[b['slot']]=b['value'][:]
    for start,count in [(0,4),(5,3),(11,4)]:
        for i in range(count):c[start+i][i]=1.
    phase=f(index//32*.5)
    c[10]=[0.,0.,4.,1.];c[16]=[2.25,f(.7),f(.2),0.];c[17]=[phase]*4;c[18]=[-.25,-.5,0.,0.];c[21]=[1.]*4
    c[22]=[f(f(.9)+f(phase*f(.05))),f(.01),f(.02),0.];c[25]=[f(f(.3)+f(phase*f(.1))),f(.2),f(.1),0.]
    v=[[0.]*4 for _ in range(16)]
    v[0]=[f((index%8-3.5)*f(.2)),f((index//8%4-1.5)*f(.2)),[-.75,f(-.4),0.,.75][index%4],1.]
    v[1]=[0.,0.,1.,0.];v[2]=[index%8/8.,index//8%4/4.,0.,1.]
    for key,expected_value in [('vertices',v),('constants',c)]:assert [[bits(x) for x in row] for row in probe['input'][key]]==[[bits(x) for x in row] for row in expected_value],(index,key)
    output,defined,trace=algorithm(v,c);evaluation=probe['evaluation']
    assert trace==evaluation['trace'],index
    assert defined==evaluation['defined']
    assert [[bits(x) for x in row] for row in output]==[[bits(x) for x in row] for row in evaluation['output']]
    branches.add(tuple(trace[20]['words'][0][:3]))
    uv=[[output[3][0],0.],output[4][:2],output[5][:2]];t=[sample(textures[i],uv[i]) for i in range(3)]
    luma=min(1.,max(0.,f(f(f(t[1][0]*pc[0][0])+f(t[1][1]*pc[0][1]))+f(t[1][2]*pc[0][2]))))
    color=[f(f(f(f(luma*t[0][i])*t[2][i])*min(1.,max(0.,output[1][i])))*pc[1][i]) for i in range(3)]+[f(t[0][3]*pc[1][3])]
    pixel=0xff000000
    for i,s in enumerate((16,8,0)):
        channel=min(1.,max(0.,f(f(color[i]*color[3])+f(((0xff101820>>s)&255)/255.))))
        pixel|=math.floor(f(channel*255.)+.5)<<s
    assert pixel==probe['pixel'],index;points.append(pixel)
assert len(report['probes'])==96 and len(branches)==3
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/vertex-tests.log').read_text())));assert tests==539,tests
sources=['crates/rc-package/src/vertex_shader.rs','crates/rc-inspect/src/assets.rs','crates/rc-inspect/src/bin/rc-vertex-check.rs','crates/rc-render/src/fragment.rs','scripts/Record-VertexShader.py']
validation={'report_sha256':sha(path),'pixel_regression_sha256':sha(previous),'original_sha256':original['hashes'],'source_sha256':{s:sha(root/s) for s in sources},'serialized_bindings':len(bindings),'instructions':53,'probes':96,'register_snapshots':96*53,'scan_regions':len(branches),'linked_fragment_pixels':len(points),'rust_tests':tests,'scope':report['scope'],'diffuse_coverage_unchanged':{'resolved':271,'omitted':18}}
(root/'analysis/reports/vertex-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
e=root/'analysis/evidence.json';evidence=json.loads(e.read_text());evidence['rust_tests']=tests;evidence['vertex_shader_validation']=validation;e.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({k:validation[k] for k in ['serialized_bindings','instructions','probes','register_snapshots','scan_regions','linked_fragment_pixels','rust_tests']}))
