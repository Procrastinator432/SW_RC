"""Independent numeric scalar-SSE interpreter for full original skin streams."""
from pathlib import Path
import collections
import hashlib
import json
import re
import struct

root=Path(__file__).resolve().parents[1]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=root/'scripts/Record-RigidSkin.py'; ns={'__file__':str(source)}
exec(compile(source.read_text().split('stats = collections.Counter()')[0],str(source),'exec'),ns)
f,bits,value,palette,rigid,links,poses,inverse_map=[ns[k] for k in ('f','bits','value','palette','rigid','links','poses','inverse_map')]
asm=(root/'analysis/decompiled/skeletal-skin-vertices.asm').read_text()
instructions=[]
for line in asm.splitlines():
    if line and not line.startswith('#'):
        addr,ins=line.split(' ',1); op,_,args=ins.partition(' ')
        instructions.append((int(addr,16),op,args))
def disp(op):
    n=re.findall(r'-?0x[0-9a-f]+',op); n=int(n[-1],16) if n else 0
    return n-0x100000000 if n>=0x80000000 else n

# Emit a numeric instruction-by-instruction interpreter, retaining independent
# temporaries instead of using the Rust generator or its expression trees.
def kernel(lo,hi,matrices,conversions,seed,stack_output=False):
    lines=['def run(p,n,m,m0,m1,w,previous):',
           ' s32,s28,s24=n', ' s60,s56=previous[4:6]']
    lines += [' '+x for x in seed]
    def read(op):
        if op.startswith('XMM'): return op.lower()
        if 'EBP' in op: return 's'+str(-disp(op))
        if 'ECX' in op: return f'p[{disp(op)//4}]'
        for reg,name in matrices.items():
            if reg in op:return f'{name}[{disp(op)//4}]'
        raise ValueError(op)
    for addr,op,args in instructions:
        if not lo<=addr<=hi:continue
        if op=='CVTSI2SS':
            dst,_=args.split(',',1); lines.append(f' {dst.lower()}=f({conversions[addr]})'); continue
        if op not in ('MOVSS','MOVAPS','MULSS','ADDSS'):continue
        dst,src=args.split(',',1); rhs=read(src)
        if dst.startswith('XMM'):
            target=dst.lower()
            if op in ('MULSS','ADDSS'):rhs=f'f({target} '+('*' if op=='MULSS' else '+')+f' {rhs})'
        elif 'EBP' in dst:target='s'+str(-disp(dst))
        elif 'EBX' in dst:target='o'+str(disp(dst)//4)
        else:
            assert 'EDX' in dst; continue
        lines.append(f' {target}={rhs} # {addr:08x}')
    lines.append(' return ['+','.join('s'+str(n) for n in (84,80,76,64,60,56))+']' if stack_output else ' return [o0,o1,o2,o3,o4,o5]')
    scope={'f':f,'scale':value(0x31800080)}
    exec('\n'.join(lines),scope)
    return scope['run']

two=kernel(0x1050c518,0x1050c7de,{'ESI':'m0','EAX':'m1'},
    {0x1050c549:'n[0]',0x1050c575:'n[1]',0x1050c581:'n[2]'},['xmm3=w[0]','xmm4=w[1]','xmm6=scale'])
cached=kernel(0x1050c813,0x1050c99b,{'EAX':'m'},
    {0x1050c829:'n[0]',0x1050c852:'n[1]',0x1050c85b:'n[2]'},[])
cache_two=kernel(0x1050c9f6,0x1050cce7,{'ESI':'m0','EAX':'m1'},
    {0x1050ca14:'w[1]',0x1050ca1b:'w[0]',0x1050ca41:'n[0]',0x1050ca6a:'n[1]',0x1050ca73:'n[2]'},['xmm6=scale'])
first=kernel(0x1050cd59,0x1050cee9,{'EAX':'m'},
    {0x1050cd67:'n[0]',0x1050cd98:'n[1]',0x1050cda4:'n[2]'},['xmm0=w','xmm6=scale'],True)
next_influence=kernel(0x1050cf00,0x1050d074,{'EAX':'m'},
    {0x1050cf30:'w'},['xmm1,xmm2,xmm3,xmm4=previous[:4]','xmm5=n[2]','xmm6=scale'],True)

def influence(commands,vertex,matrices):
    kind=commands[0]>>28
    if kind==0:return rigid(vertex,matrices[(commands[0]&4095)//6])
    p=list(map(value,vertex['position']))
    normal=vertex['packed_normal']; n=[float(((normal>>(10*i))&1023)-511) for i in range(3)]
    m=[list(map(value,matrices[(word&4095)//6])) for word in commands]
    w=[f(word&0x0fffffff) for word in commands]
    if kind==8:out=cached(p,n,m[0],None,None,None,[0]*6)
    elif kind in (1,9):out=(two if kind==1 else cache_two)(p,n,None,m[0],m[1],w,[0]*6)
    else:
        out=first(p,n,m[0],None,None,w[0],[0]*6)
        for matrix,weight in zip(m[1:],w[1:]):out=next_influence(p,n,matrix,None,None,weight,out)
    return dict(position=list(map(bits,out[:3])),normal=list(map(bits,out[3:])))

def stream(commands,inputs,matrices):
    pc=bind=0; cache=[]; output=[]; kinds=[0]*16
    while commands[pc]!=0xffffffff:
        command=commands[pc]; kind=command>>28; kinds[kind]+=1
        if kind==15:
            count=1; offset=(command&0x0fffffff)//3*3
            raw=cache[offset:offset+6]; assert len(raw)==6
            vertex=dict(position=raw[:3],normal=raw[3:])
        else:
            count=1+(kind&7)
            vertex=influence(commands[pc:pc+count],inputs[bind],matrices); bind+=1
            if kind&8:cache.extend(vertex['position']+vertex['normal'])
        output.append(dict(vertex=vertex,uv=commands[pc+count:pc+count+2]))
        pc+=count+2
    assert pc+1==len(commands) and bind==len(inputs)
    return dict(result=dict(written=len(output),consumed_bind=bind,command_words=pc+1,cache_words=len(cache),kinds=kinds),output=output,cache=cache)

def checked(name,validation):
    p=root/'analysis/reports'/name
    assert sha(p)==json.loads((root/'analysis/reports'/validation).read_text())['report_sha256']
    return json.loads(p.read_text())

def check():
    lods=checked('skeletal-lods.json','skeletal-lods-validation.json')
    path=root/'analysis/reports/skeletal-skin-streams.json'; report=json.loads(path.read_text())
    source=root/'scripts/Record-ReferenceCaches.py'; ref={'__file__':str(source)}
    exec(compile(source.read_text().split("original_path=root/")[0],str(source),'exec'),ref)
    pose_map={o['source_index']:o for o in poses['objects']}
    lod_map={o['source_index']:o for o in lods['objects']}
    stats=collections.Counter(); kinds=collections.Counter()
    assert len(report['objects'])==130
    for obj in report['objects']:
        index=obj['source_index']; old=pose_map[index]; bones=links['objects'][index]['prefix']['bones']
        if old['runs']:
            run=old['runs'][0]; case=run['cases'][0]
            assert (obj['animation'],obj['entry'],obj['step'])==(run['animation'],run['entry'],case['step'])
            assert obj['pose_source']=='AnimatedFullPose'
            matrices=case['full']['matrices']; stats['animated_meshes']+=1
        else:
            assert obj['pose_source']=='OriginalReferencePose'
            matrices=[]
            for i,bone in enumerate(bones):
                m=ref['matrix'](bone['rotation'],bone['position'])
                if i:m=ref['product'](m,matrices[bone['word_34']])
                matrices.append(m)
            stats['reference_pose_meshes']+=1
        transforms=[palette(inv,pose) for inv,pose in zip(inverse_map[index],matrices)]
        assert len(obj['lods'])==len(lod_map[index]['mesh']['lods'])
        for actual,original in zip(obj['lods'],lod_map[index]['mesh']['lods']):
            if not original['commands']:
                assert actual is None; stats['empty_lods']+=1; continue
            expected=stream(original['commands'],original['bind_vertices'],transforms)
            assert actual==expected,(index,'original stream mismatch')
            stats['streams']+=1; stats['output_vertices']+=len(actual['output'])
            stats['cache_words']+=len(actual['cache'])
            kinds.update({str(i):n for i,n in enumerate(actual['result']['kinds']) if n})
        print(f"checked mesh {index}",flush=True) if stats['streams']%40<4 else None
    for case in report['synthetic']:
        expected=stream(case['commands'],case['input'],case['palette'])
        assert {k:case[k] for k in ('result','output','cache')}==expected
        stats['synthetic_streams']+=1; stats['synthetic_vertices']+=len(case['output'])
    assert (stats['streams'],stats['output_vertices'],stats['animated_meshes'],stats['reference_pose_meshes'],stats['empty_lods'],stats['synthetic_streams'])==(414,289085,125,5,12,64)
    assert (report['streams'],report['vertices'])==(414,289085)
    # Recover the weight scale from the installed original PE, independently of Rust.
    engine=Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\Engine.dll')
    data=engine.read_bytes(); pe=struct.unpack_from('<I',data,60)[0]; sections=struct.unpack_from('<H',data,pe+6)[0]
    size=struct.unpack_from('<H',data,pe+20)[0]; opt=pe+24; base=struct.unpack_from('<I',data,opt+28)[0]
    rva=0x10679850-base; found=False
    for i in range(sections):
        vs,va,rs,raw=struct.unpack_from('<IIII',data,opt+size+40*i+8)
        if va<=rva<va+max(vs,rs):
            assert struct.unpack_from('<I',data,raw+rva-va)[0]==0x31800080; found=True
    assert found
    print(json.dumps(dict(stats)),flush=True)
    return report,path,stats,kinds,engine

if __name__=='__main__':check()
