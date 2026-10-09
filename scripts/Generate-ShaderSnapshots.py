"""Synthetic sparse x86 memory images at relocated addresses, not live captures."""
from pathlib import Path
import struct,json,copy
root=Path(__file__).resolve().parents[1]
bits=lambda x:struct.unpack('<I',struct.pack('<f',x))[0]
identity=[[bits(1.) if r==c else 0 for c in range(4)] for r in range(4)]
def regions(mem):
    result=[]
    for address,byte in sorted(mem.items()):
        if not result or result[-1]['address']+len(result[-1]['bytes'])!=address:result.append({'address':address,'bytes':[]})
        result[-1]['bytes'].append(byte)
    return result
def build(i):
    mem={};base=0x100000+i*0x10000
    renderer=base;state=base+0xa000;viewport=base+0xb000;camera=base+0xc000;editor_actor=base+0xd000;manager=base+0xe000;cubemap=base+0xf000
    globals={'engine_time':base+0x9000,'editor':base+0x9004,'cubemap_manager':base+0x9008}
    def word(address,value):
        for k,b in enumerate(struct.pack('<I',value)):mem[address+k]=b
    def words(address,values):
        for k,v in enumerate(values):word(address+4*k,v)
    editor=i%2==0;time=bits(i*.125)
    word(renderer+0x9c0c,state);word(renderer+8,viewport);word(globals['editor'],1 if editor else 0);word(globals['engine_time'],time)
    manager_value=0 if i%4==0 else manager;actor_value=cubemap if i%4>=2 else 0
    word(globals['cubemap_manager'],manager_value)
    if manager_value:word(manager+0x2c,actor_value)
    actor_scale=None
    if actor_value:
        actor_scale=[0x7fc12345,0x80000000,bits(2.)];words(cubemap+0x1b4,actor_scale);word(cubemap+0x64,0x2000 if i%4==2 else 0)
    runtime_camera=0 if editor or i%11==0 else camera
    if editor:word(viewport+0x30,editor_actor);words(editor_actor+0x138,list(map(bits,[1.,2.,3.])))
    else:word(viewport+0x184,runtime_camera)
    camera_matrix=copy.deepcopy(identity);camera_matrix[3]=list(map(bits,[4.,0.,0.,1.]))
    if runtime_camera:words(camera+0x54,[v for row in camera_matrix for v in row]);words(camera+0x194,list(map(bits,[7.,8.,9.])))
    object=[list(map(bits,r)) for r in [[2.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,.5,0.],[.125,-.25,.5,1.]]]
    projection=[list(map(bits,r)) for r in [[0.,0.,.4,.3],[.9,0.,0.,0.],[0.,.9,0.,0.],[0.,0.,.5,1.]]]
    for offset,m in [(0x2c,object),(0x6c,identity),(0xac,projection)]:words(state+offset,[v for row in m for v in row])
    words(state+0x2f0,list(map(bits,[2.,8.])));word(state+0x148,0x99ff8040)
    slots=[]
    for s in range(4):
        source=base+0x800+s*0x100;actor=base+0x1000+s*0x100;mode=(i>>(2*s))&3
        word(state+0x328+s*4,0 if mode==0 else source)
        if mode==0:slots.append(None);continue
        word(source,0 if mode==1 else actor)
        empty={'kind':0,'radius':[0,0],'position':[0]*3,'actor_present':False,'cone':0,'color':[0]*4,'brightness':0,'direction':[0]*3,'flags':[0,0]}
        if mode==1:slots.append(empty);continue
        light={**empty,'actor_present':True,'kind':19 if (i+s)%2==0 else 7,'radius':list(map(bits,[4.,2.])),'position':list(map(bits,[2.,3.,4.])),'color':list(map(bits,[.25,.5,.75,1.])),'brightness':bits(.75),'direction':list(map(bits,[.125,.25,-.5])),'cone':[37,0,255,3][s]}
        mem[actor+0x2a]=light['kind'];mem[actor+0x39]=light['cone']
        for offset,values in [(8,light['color']),(0x18,light['position']),(0x24,light['direction']),(0x30,light['radius']),(0x38,light['flags'])]:words(source+offset,values)
        word(source+0x48,light['brightness']);slots.append(light)
    expected={'object':object,'view':identity,'projection':projection,'camera':camera_matrix if runtime_camera else None,'editor':editor,'time':time,'actor_scale':actor_scale,'editor_eye':list(map(bits,[1.,2.,3.])) if editor else [0]*3,'runtime_eye':list(map(bits,[7.,8.,9.])) if runtime_camera else None,'fog':list(map(bits,[2.,8.])),'lighting':{'slots':slots,'alpha_gate':i%4==3,'ambient_bgra':0x99ff8040}}
    return {'id':i,'renderer':renderer,'globals':globals,'expected':expected,'regions':regions(mem)},mem
cases=[build(i)[0] for i in range(256)]
for failure in range(8):
    case,mem=build(255);base=case['renderer'];case['id']=256+failure;case['expected']=None
    if failure==0:
        for k in range(4):mem.pop(base+0x9c0c+k)
    elif failure==1:case['renderer']=0xfffffff0
    elif failure==2:case['renderer']=0
    elif failure==3:
        for k in range(4):mem[base+0x9c0c+k]=0
    elif failure==4:mem.pop(base+0xc000+0x194)
    elif failure==5:
        for k,b in enumerate(struct.pack('<I',1)):mem[case['globals']['editor']+k]=b
        for k in range(4):mem[base+0xb000+0x30+k]=0
    elif failure==6:
        for k,b in enumerate(struct.pack('<I',0xdead0000)):mem[base+0xe000+0x2c+k]=b
    else:mem.pop(base+0x800+0x48)
    case['regions']=regions(mem);cases.append(case)
(root/'analysis/reports/shader-snapshots.input.json').write_text(json.dumps({'scope':'Synthetic sparse original-layout memory images, not process captures','cases':cases},separators=(',',':'))+'\n')
print('Generated 256 relocated snapshots and 8 malformed snapshots')
