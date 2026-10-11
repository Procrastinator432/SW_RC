"""Bounded original SSE instruction replay; portable RSQRT seed is a supplied boundary.

Replays acceleration stores, AProjectile velocity cap and pre-MoveActor delta.
No world/actor callbacks, full physProjectile, hardware RSQRT or script VM claim.
"""
import hashlib
import json
import math
import random
import re
import struct
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/milestone3e'
ASM=OUT/'projectile-native.asm'
BINARY=Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData/System/engine.dll')
raw=BINARY.read_bytes();pe=struct.unpack_from('<I',raw,60)[0]
base=struct.unpack_from('<I',raw,pe+52)[0];optional=struct.unpack_from('<H',raw,pe+20)[0]
sections=[struct.unpack_from('<IIII',raw,pe+24+optional+40*i+8) for i in range(struct.unpack_from('<H',raw,pe+6)[0])]
def word(address):
    for size,va,count,offset in sections:
        if va<=address-base<va+count:return struct.unpack_from('<I',raw,offset+address-base-va)[0]
    raise AssertionError(hex(address))
def bits(v):return struct.unpack('<I',struct.pack('<f',v))[0]
def number(v):return struct.unpack('<f',struct.pack('<I',v))[0]
def f32(v):return number(bits(v))
assert [number(word(a)) for a in [0x1065efc4,0x1065efc8,0x1065efcc]]==[0.5,3.,0.]
code={int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z0-9.]+)(?: (.*))?$',ASM.read_text(),re.M)}
addresses=sorted(code);following=dict(zip(addresses,addresses[1:]))

def replay(v,a,cap,dt):
    actor=0x20000000;stack=0x30000000
    m={actor+0x150+4*i:bits(x) for i,x in enumerate(v)}
    m.update({actor+0x15c+4*i:bits(x) for i,x in enumerate(a)})
    m[actor+0x288]=bits(cap)
    r={k:0 for k in ['EAX','EBX','ECX','EDX','ESI','EDI','ESP','EBP']}
    r.update(ECX=actor,ESI=actor,ESP=stack,EBP=stack+0x100)
    m[r['EBP']+8]=bits(dt)
    x={f'XMM{i}':0 for i in range(8)}
    z=c=p=False;steps=0
    def address(o):
        o=o[o.index('[')+1:o.index(']')]
        terms=o.split(' + ')
        return sum(r[t] if t in r else int(t,0) for t in terms)
    def get(o):
        if o in r:return r[o]
        if o in x:return x[o]
        if o=='AH':return (r['EAX']>>8)&255
        if '[' in o:
            at=address(o)
            return word(at) if base<=at<base+0x1000000 else m.get(at,0)
        return int(o,0)
    def put(o,value):
        if o in r:r[o]=value&0xffffffff
        elif o in x:x[o]=value
        else:m[address(o)]=value&0xffffffff
    def run(pc,stop=None):
        nonlocal z,c,p,steps
        while pc!=stop:
            steps+=1;assert steps<1000
            op,ops=code[pc];nxt=following.get(pc)
            if op=='RET':break
            if op in ['MOV','MOVSS','MOVAPS']:put(ops[0],get(ops[1]))
            elif op=='LEA':put(ops[0],address(ops[1]))
            elif op in ['ADD','SUB']:put(ops[0],get(ops[0])+(1 if op=='ADD' else -1)*get(ops[1]))
            elif op in ['ADDSS','SUBSS','MULSS']:
                left,right=number(get(ops[0])),number(get(ops[1]))
                put(ops[0],bits(left+right if op=='ADDSS' else left-right if op=='SUBSS' else left*right))
            elif op=='RSQRTSS':put(ops[0],bits(1/f32(math.sqrt(number(get(ops[1]))))))
            elif op=='XORPS':put(ops[0],get(ops[0])^get(ops[1]))
            elif op in ['COMISS','UCOMISS']:
                left,right=number(get(ops[0])),number(get(ops[1]));z=left==right;c=left<right;p=False
            elif op=='LAHF':r['EAX']=(r['EAX']&0xffff00ff)|((int(z)*64+int(p)*4+int(c))<<8)
            elif op=='TEST':
                value=get(ops[0])&get(ops[1]);z=value==0;p=(value&255).bit_count()%2==0
            elif op=='JBE':
                if z or c:nxt=int(ops[0],0)
            elif op=='JNP':
                if not p:nxt=int(ops[0],0)
            elif op=='PUSH':r['ESP']-=4;m[r['ESP']]=get(ops[0])
            elif op=='POP':put(ops[0],m[r['ESP']]);r['ESP']+=4
            else:raise AssertionError((hex(pc),op,ops))
            pc=nxt
    if any(a):
        x['XMM0']=bits(dt)
        run(0x104924f3,0x10492569)
        r['ECX']=actor
        run(0x1048b800)
    run(0x10492577,0x104925eb)
    return dict(velocity=[m[actor+0x150+4*i] for i in range(3)],delta=[m[r['EBP']-0x38+4*i] for i in range(3)],instructions=steps)

cases=[]
rng=random.Random(10492300)
inputs=[([0.,0.,0.],[60000.,0.,0.],10000.,1/60),([4.,0.,0.],[1.,0.,0.],0.,1.),([4.,0.,0.],[0.,0.,0.],0.,1.)]
for _ in range(2048):
    inputs.append(([rng.uniform(-20000,20000) for _ in range(3)],
                   [rng.uniform(-60000,60000) for _ in range(3)],rng.choice([0.,100.,10000.,30000.]),rng.choice([0.,1/120,1/60,.1])))
for v,a,cap,dt in inputs:
    v=list(map(f32,v));a=list(map(f32,a));cap=f32(cap);dt=f32(dt)
    cases.append(dict(velocity=list(map(bits,v)),acceleration=list(map(bits,a)),max_speed=bits(cap),dt=bits(dt),expected=replay(v,a,cap,dt)))
result=dict(scope=__doc__,binary_sha256=hashlib.sha256(raw).hexdigest(),assembly_sha256=hashlib.sha256(ASM.read_bytes()).hexdigest(),cases=cases)
(OUT/'projectile-oracle.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(f'Recorded {len(cases)} prepared instruction cases with portable sqrt seed boundary')
