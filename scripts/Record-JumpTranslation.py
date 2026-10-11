"""Replay original PostLoad Move-track endpoint translation instructions."""
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
BINARY=Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData/System/engine.dll')
raw=BINARY.read_bytes(); pe=struct.unpack_from('<I',raw,60)[0]
base=struct.unpack_from('<I',raw,pe+52)[0]; optional=struct.unpack_from('<H',raw,pe+20)[0]
sections=[struct.unpack_from('<IIII',raw,pe+24+optional+40*i+8) for i in range(struct.unpack_from('<H',raw,pe+6)[0])]
def word(address):
    for size,va,count,offset in sections:
        if va<=address-base<va+count:return struct.unpack_from('<I',raw,offset+address-base-va)[0]
    raise AssertionError(hex(address))
def bits(v):return struct.unpack('<I',struct.pack('<f',v))[0]
def number(v):return struct.unpack('<f',struct.pack('<I',v))[0]
for address,expected in [(0x10665bc8,0xbf000000),(0x1065efc4,0x3f000000),(0x1065efc8,0x40400000),(0x10653578,0x3f800000),(0x1066a268,0x3f733333),(0x10668d44,0x3fc00000),(0x1065efcc,0)]:
    assert word(address)==expected,(hex(address),hex(word(address)))
code={int(a,16):(op,args.split(',') if args else []) for path in [OUT/'animation-translation.asm'] for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z0-9.]+)(?: (.*))?$',path.read_text(),re.M)}
addresses=sorted(code); following=dict(zip(addresses,addresses[1:]))

class VM:
    def __init__(self):
        self.r={k:0 for k in ['EAX','EBX','ECX','EDX','ESI','EDI','ESP','EBP']}
        self.r['ESP']=0x30000000
        self.x={f'XMM{i}':0 for i in range(8)}
        self.m={}; self.z=self.c=self.p=False; self.steps=0; self.extensions=0; self.seeds=[]; self.seed_shift=0; self.less=False
    def store(self,at,value,size=4):
        for i in range(size):self.m[at+i]=(value>>(8*i))&255
    def load(self,at,size=4):
        if at in self.m:return sum(self.m.get(at+i,0)<<(8*i) for i in range(size))
        if base<=at<base+0x1000000:return word(at)&((1<<(size*8))-1)
        return 0
    def address(self,o):
        terms=o[o.index('[')+1:o.index(']')].split(' + ')
        return sum((self.r[t.split('*')[0]]*int(t.split('*')[1],0)) if '*' in t else self.r[t] if t in self.r else int(t,0) for t in terms)
    def get(self,o):
        if o in self.r:return self.r[o]
        if o in self.x:return self.x[o]
        if o=='AL':return self.r['EAX']&255
        if o=='AH':return (self.r['EAX']>>8)&255
        if '[' in o:return self.load(self.address(o),1 if o.startswith('byte') else 2 if o.startswith('word') else 4)
        return int(o,0)
    def put(self,o,value):
        value&=0xffffffff
        if o in self.r:self.r[o]=value
        elif o in self.x:self.x[o]=value
        elif o=='AL':self.r['EAX']=(self.r['EAX']&0xffffff00)|(value&255)
        elif o=='AH':self.r['EAX']=(self.r['EAX']&0xffff00ff)|((value&255)<<8)
        else:self.store(self.address(o),value,1 if o.startswith('byte') else 2 if o.startswith('word') else 4)
    def arg(self,n=0):return self.load(self.r['ESP']+4*n)
    def popargs(self,n):self.r['ESP']+=4*n
    def vector(self,at,v):
        for i,x in enumerate(v):self.store(at+4*i,bits(x))
    def run(self,pc,call,stops=()):
        while pc not in stops:
            self.steps+=1; assert self.steps<50000
            op,ops=code[pc]; nxt=following.get(pc)
            if pc==0x103bc410:self.extensions+=1
            if op=='RET':break
            if op in ['MOV','MOVSS','MOVAPS']:self.put(ops[0],self.get(ops[1]))
            elif op=='MOVSX':
                value=self.get(ops[1]);self.put(ops[0],value if value<32768 else value-65536)
            elif op in ['SHL','SAR']:
                value=self.get(ops[0]);n=self.get(ops[1]);self.put(ops[0],value<<n if op=='SHL' else (value if value<0x80000000 else value-0x100000000)>>n)
            elif op=='LEA':self.put(ops[0],self.address(ops[1]))
            elif op in ['ADD','SUB']:self.put(ops[0],self.get(ops[0])+(1 if op=='ADD' else -1)*self.get(ops[1]))
            elif op in ['AND','ANDPS','OR','XOR','XORPS']:
                a,b=self.get(ops[0]),self.get(ops[1]); v=a&b if op in ['AND','ANDPS'] else a|b if op=='OR' else a^b
                self.put(ops[0],v)
                if op not in ['XORPS','ANDPS']:self.z=v==0; self.c=False
            elif op=='CVTSI2SS':
                v=self.get(ops[1]);self.put(ops[0],bits(v if v<0x80000000 else v-0x100000000))
            elif op in ['ADDSS','SUBSS','MULSS','DIVSS']:
                a,b=number(self.get(ops[0])),number(self.get(ops[1]))
                self.put(ops[0],bits(a+b if op=='ADDSS' else a-b if op=='SUBSS' else a*b if op=='MULSS' else a/b))
            elif op in ['COMISS','UCOMISS']:
                a,b=number(self.get(ops[0])),number(self.get(ops[1])); unordered=math.isnan(a) or math.isnan(b)
                self.z=unordered or a==b; self.c=unordered or a<b; self.p=unordered
            elif op=='TEST':
                v=self.get(ops[0])&self.get(ops[1]);self.z=v==0;self.c=False;self.p=(v&255).bit_count()%2==0
            elif op=='CMP':a,b=self.get(ops[0]),self.get(ops[1]);self.z=a==b;self.c=a<b;self.less=(a if a<0x80000000 else a-0x100000000)<(b if b<0x80000000 else b-0x100000000)
            elif op=='SETZ':self.put(ops[0],int(self.z))
            elif op in ['JMP','JZ','JNZ','JBE','JC','JA','JL','JNP']:
                if op=='JMP' or (op=='JZ' and self.z) or (op=='JNZ' and not self.z) or (op=='JBE' and (self.z or self.c)) or (op=='JC' and self.c) or (op=='JA' and not self.c and not self.z) or (op=='JL' and self.less) or (op=='JNP' and not self.p):nxt=int(ops[0],0)
            elif op=='PUSH':self.r['ESP']-=4;self.store(self.r['ESP'],self.get(ops[0]))
            elif op=='POP':self.put(ops[0],self.load(self.r['ESP']));self.r['ESP']+=4
            elif op=='RSQRTSS':
                s=number(self.get(ops[1]));r=bits(1/number(bits(math.sqrt(s)))) if s else bits(float('inf'))
                if s:r+=self.seed_shift
                self.seeds.append(dict(input=self.get(ops[1]),seed=r));self.put(ops[0],r)
            elif op=='CMPNEQSS':
                a,b=number(self.get(ops[0])),number(self.get(ops[1]));self.put(ops[0],0xffffffff if a!=b else 0)
            elif op=='LAHF':self.put('AH',(64 if self.z else 0)|(4 if self.p else 0)|(1 if self.c else 0))
            elif op=='INC':self.put(ops[0],self.get(ops[0])+1)
            elif op=='NOP':pass
            elif op=='CALL':call(pc,ops[0])
            else:raise AssertionError((hex(pc),op,ops))
            pc=nxt



assert word(0x10664f44)==0x38000100
rng=random.Random(0x10513afe);cases=[]
for i in range(2048):
    first=[rng.randint(-32768,32767) for _ in range(3)];last=[rng.randint(-32768,32767) for _ in range(3)]
    scale=bits(rng.choice([0.,1.,165.,rng.uniform(0.01,1000)]))
    vm=VM();vm.r['EDI']=0x20000000;vm.store(0x2000005c,0x21000000)
    vm.store(0x21000008,0x22000000);vm.store(0x2100000c,2);vm.store(0x21000010,scale)
    for j,v in enumerate(first+last):vm.store(0x22000000+2*j,v&65535,2)
    vm.run(0x10513afe,lambda *a:(_ for _ in ()).throw(AssertionError(a)),stops=(0x10513b9f,))
    cases.append(dict(first=first,last=last,scale_bits=scale,expected=[vm.load(0x20000034+4*j) for j in range(3)]))
(OUT/'translation-oracle.json').write_text(json.dumps(dict(provenance=dict(binary_sha256=hashlib.sha256(raw).hexdigest(),assembly_sha256=hashlib.sha256((OUT/'animation-translation.asm').read_bytes()).hexdigest(),entry='10513afe..10513b9c',scope='Prepared PostLoad eligible Move track, nonempty positions; scalar endpoint translation only.'),cases=cases),indent=2),encoding='utf-8')
print('Recorded 2048 original PostLoad translations')
