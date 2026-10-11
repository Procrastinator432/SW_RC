"""Bounded original ordinary JumpTo landing arithmetic/branches."""
from pathlib import Path
import hashlib
import itertools
import json
import math
import random
import re
import struct

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/milestone3l'
BINARY=Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData/System/ctgame.dll')
raw=BINARY.read_bytes(); pe=struct.unpack_from('<I',raw,60)[0]
base=struct.unpack_from('<I',raw,pe+52)[0]; optional=struct.unpack_from('<H',raw,pe+20)[0]
sections=[struct.unpack_from('<IIII',raw,pe+24+optional+40*i+8) for i in range(struct.unpack_from('<H',raw,pe+6)[0])]
def word(address):
    for size,va,count,offset in sections:
        if va<=address-base<va+count:return struct.unpack_from('<I',raw,offset+address-base-va)[0]
    raise AssertionError(hex(address))
def bits(v):return struct.unpack('<I',struct.pack('<f',v))[0]
def number(v):return struct.unpack('<f',struct.pack('<I',v))[0]
code={int(a,16):(op,args.split(',') if args else []) for path in [ROOT/'analysis/milestone3j/defensive.asm'] for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z0-9.]+)(?: (.*))?$',path.read_text(),re.M)}
addresses=sorted(code); following=dict(zip(addresses,addresses[1:]))

class VM:
    def __init__(self):
        self.r={k:0 for k in ['EAX','EBX','ECX','EDX','ESI','EDI','ESP','EBP']}
        self.r['ESP']=0x30000000
        self.x={f'XMM{i}':0 for i in range(8)}
        self.m={}; self.z=self.c=False; self.steps=0
    def store(self,at,value,size=4):
        for i in range(size):self.m[at+i]=(value>>(8*i))&255
    def load(self,at,size=4):
        if at in self.m:return sum(self.m.get(at+i,0)<<(8*i) for i in range(size))
        if base<=at<base+0x1000000:return word(at)&((1<<(size*8))-1)
        return 0
    def address(self,o):
        terms=o[o.index('[')+1:o.index(']')].split(' + ')
        return sum(self.r[t] if t in self.r else int(t,0) for t in terms)
    def get(self,o):
        if o in self.r:return self.r[o]
        if o in self.x:return self.x[o]
        if o=='AL':return self.r['EAX']&255
        if '[' in o:return self.load(self.address(o),1 if o.startswith('byte') else 4)
        return int(o,0)
    def put(self,o,value):
        value&=0xffffffff
        if o in self.r:self.r[o]=value
        elif o in self.x:self.x[o]=value
        elif o=='AL':self.r['EAX']=(self.r['EAX']&0xffffff00)|(value&255)
        else:self.store(self.address(o),value,1 if o.startswith('byte') else 4)
    def arg(self,n=0):return self.load(self.r['ESP']+4*n)
    def popargs(self,n):self.r['ESP']+=4*n
    def vector(self,at,v):
        for i,x in enumerate(v):self.store(at+4*i,bits(x))
    def run(self,pc,call,stops=()):
        while pc not in stops:
            self.steps+=1; assert self.steps<50000
            op,ops=code[pc]; nxt=following.get(pc)
            if op=='RET':break
            if op in ['MOV','MOVSS','MOVAPS']:self.put(ops[0],self.get(ops[1]))
            elif op=='LEA':self.put(ops[0],self.address(ops[1]))
            elif op in ['ADD','SUB']:self.put(ops[0],self.get(ops[0])+(1 if op=='ADD' else -1)*self.get(ops[1]))
            elif op in ['AND','OR','XOR','XORPS']:
                a,b=self.get(ops[0]),self.get(ops[1]); v=a&b if op=='AND' else a|b if op=='OR' else a^b
                self.put(ops[0],v)
                if op!='XORPS':self.z=v==0; self.c=False
            elif op=='CVTSI2SS':
                v=self.get(ops[1]);self.put(ops[0],bits(v if v<0x80000000 else v-0x100000000))
            elif op in ['ADDSS','SUBSS','MULSS']:
                a,b=number(self.get(ops[0])),number(self.get(ops[1]))
                self.put(ops[0],bits(a+b if op=='ADDSS' else a-b if op=='SUBSS' else a*b))
            elif op=='COMISS':
                a,b=number(self.get(ops[0])),number(self.get(ops[1])); unordered=math.isnan(a) or math.isnan(b)
                self.z=unordered or a==b; self.c=unordered or a<b
            elif op=='TEST':self.z=self.get(ops[0])&self.get(ops[1])==0;self.c=False
            elif op=='CMP':a,b=self.get(ops[0]),self.get(ops[1]);self.z=a==b;self.c=a<b
            elif op=='SETZ':self.put(ops[0],int(self.z))
            elif op in ['JMP','JZ','JNZ','JBE','JC']:
                if op=='JMP' or (op=='JZ' and self.z) or (op=='JNZ' and not self.z) or (op=='JBE' and (self.z or self.c)) or (op=='JC' and self.c):nxt=int(ops[0],0)
            elif op=='PUSH':self.r['ESP']-=4;self.store(self.r['ESP'],self.get(ops[0]))
            elif op=='POP':self.put(ops[0],self.load(self.r['ESP']));self.r['ESP']+=4
            elif op=='CALL':call(pc,ops[0])
            else:raise AssertionError((hex(pc),op,ops))
            pc=nxt
        return pc


CASES=[];rng=random.Random(0x10032b07)
def landing(started,completed,physics,position,target,radius):
    vm=VM();goal=0x20000000;bot=0x21000000;pawn=0x22000000
    vm.r['ESI']=goal;vm.r['EDI']=0;vm.store(goal+0x34,bot);vm.store(bot+0x284,pawn)
    # Prepared falling observation is supplied at the arithmetic phase boundary.
    started=started or physics==2
    vm.store(goal+0x60,int(started)|(int(completed)<<1));vm.store(pawn+0x3a,physics,1)
    vm.vector(pawn+0x138,position);vm.vector(goal+0x50,target);vm.store(goal+0x5c,bits(radius))
    pc=vm.run(0x10032b07,lambda *a:(_ for _ in ()).throw(AssertionError(a)),stops=(0x10032c15,0x10032eec,0x10032f84))
    decision={0x10032c15:'Land',0x10032eec:'Fail',0x10032f84:'Active'}[pc]
    CASES.append(dict(started=started,completed=completed,physics=physics,position=position,target=target,tolerance_squared=radius,decision=decision))
for started,completed,physics,x,radius in itertools.product([False,True],[False,True],[0,1,2,4,12],[0.,1.,2.,number(bits(2.)-1),number(bits(2.)+1),3.],[0.,1.,4.,9.]):landing(started,completed,physics,[x,0.,0.],[0.,0.,0.],radius)
for _ in range(1024):landing(bool(rng.getrandbits(1)),bool(rng.getrandbits(1)),rng.choice([1,2,4]),[number(bits(rng.uniform(-1000,1000))) for _ in range(3)],[number(bits(rng.uniform(-1000,1000))) for _ in range(3)],number(bits(rng.uniform(0,4e6))))
(OUT/'landing-oracle.json').write_text(json.dumps(dict(provenance=dict(binary_sha256=hashlib.sha256(raw).hexdigest(),assembly_sha256=hashlib.sha256((ROOT/'analysis/milestone3j/defensive.asm').read_bytes()).hexdigest(),entry='10032b07',scope='Ordinary null target, prepared started/falling observation, stop before landing/failure side effects. No Perch, callbacks or rotation proof.'),cases=CASES),indent=2),encoding='utf-8')
print(f'Recorded {len(CASES)} original landing branch cases')
