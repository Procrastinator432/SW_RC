"""Replay bounded original x86 planning instructions with prepared host calls.

Runs exported FindNearestNavNode, DoDefensiveJump and the post-melee rand branch
in a small instruction interpreter. Calls supply snapshots; no live DLL, full
navigation graph, goal execution, allocation failure or Android flight claim.
"""
from pathlib import Path
import hashlib
import itertools
import json
import math
import random
import re
import struct

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/milestone3j'
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
assert word(0x1004dd98)==0x60ad78ec
assert word(0x1004e130)==0x38000100
assemblies=[OUT/'defensive.asm',ROOT/'analysis/decompiled/geonosian-attack.asm']
code={int(a,16):(op,args.split(',') if args else []) for path in assemblies for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z0-9.]+)(?: (.*))?$',path.read_text(),re.M)}
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

BOT=0x20000000;PAWN=0x20001000;LEVEL=0x20002000;STATE=0x20003000;ENEMY=0x20004000
ORIGIN=0x20005000;VTABLE=0x20006000;CLASS=0x20007000;OTHER=0x20008000
def common(vm):
    vm.store(BOT+0x284,PAWN);vm.store(BOT+0x94,LEVEL);vm.store(STATE+0x28,BOT);vm.store(BOT+0x2e8,ENEMY)
    vm.store(PAWN,VTABLE);vm.store(ENEMY,VTABLE);vm.store(PAWN+0xa0,77)
def search(nodes,q):
    vm=VM();common(vm);vm.r['ECX']=BOT;sp=vm.r['ESP'];calls=[]
    vm.vector(ORIGIN,q['origin'])
    for i,v in enumerate([ORIGIN,CLASS,bits(q['range']),q['require_clear'],q['require_reachable'],q['require_same_zone']]):vm.store(sp+4+4*i,int(v))
    locations={0x21000000+0x1000*i:n for i,n in enumerate(nodes)}
    vm.store(LEVEL+0x474,next(iter(locations),0))
    for i,(at,n) in enumerate(locations.items()):
        vm.store(at,VTABLE);vm.vector(at+0x138,n['position']);vm.store(at+0x2c4,0x4000000 if n['enabled'] else 0)
        vm.store(at+0xa0,77 if n['same_zone'] else 88);vm.store(at+0x24,CLASS if n['class_matches'] else OTHER)
        vm.store(at+0x2a0,at+0x1000 if i+1<len(nodes) else 0)
    def call(pc,operand):
        if pc==0x10012e3f:calls.append('Anchor');vm.r['EAX']=0x22000000 if q['anchor_present'] else 0
        elif pc==0x10012e84:
            n=locations[vm.r['ECX']];calls.append({'Available':n['id']});vm.r['EAX']=int(n['available'])
        elif pc==0x10012f5f:
            n=locations[vm.arg(1)];calls.append({'Reachable':n['id']});vm.r['EAX']=int(n['reachable']);vm.popargs(4)
        elif pc==0x10012f9a:
            n=locations[vm.arg()-0x138];calls.append({'Clear':n['id']});vm.r['EAX']=int(n['clear']);vm.popargs(5)
        else:raise AssertionError((hex(pc),operand))
    vm.run(0x10012e00,call)
    return dict(selected=locations[vm.r['EAX']]['id'] if vm.r['EAX'] else None,calls=calls)

def plan(case):
    vm=VM();common(vm);vm.r['ECX']=STATE;vm.store(STATE+0x90,3);vm.store(BOT+0x4c0,0x25)
    vm.vector(PAWN+0x138,case['origin']);targets={};calls=[];goals=[];objects={};next_object=0x24000000
    for key,at in [('perch',0x23000000),('jump',0x23001000)]:
        if case[key]:targets[at]=case[key];vm.vector(at+0x138,case[key]['position'])
    def token(at):return targets[at]['id'] if at in targets else 0 if at==0 else 99 if at==ENEMY else case['anchor']
    def subject(at):return 'Enemy' if at==ENEMY else {'Node':token(at)}
    def call(pc,operand):
        nonlocal next_object
        if pc in [0x100399c6,0x100399f0]:vm.r['EAX']=CLASS;return
        if pc in [0x100399db,0x10039a06]:
            kind='Perch' if pc==0x100399db else 'JumpDest';calls.append({'Find':kind});vm.r['EAX']=(0x23000000 if kind=='Perch' else 0x23001000) if case['perch' if kind=='Perch' else 'jump'] else 0;vm.popargs(6);return
        if pc==0x10039a2a:calls.append({'Reserve':token(vm.arg())});vm.popargs(1);return
        if pc==0x10039b6c:calls.append({'Clear':token(vm.arg()-0x138)});vm.r['EAX']=int(case['clear']);vm.popargs(5);return
        if pc==0x10039cd8:calls.append({'Reachable':token(vm.arg())});vm.r['EAX']=int(case['reachable']);vm.popargs(4);return
        if operand=='dword ptr [0x1004d0cc]':vm.r['EAX']=0x25000000;return
        if operand=='EDI':
            assert vm.r['ECX'] in [0x1007bb08,0x10079ae8,0x100777a0,0x10078098,0x1007d590,0x100779d8]
            vm.r['EAX']=next_object;next_object+=0x100;vm.popargs(6);return
        if operand in ['dword ptr [EAX + 0x88]','dword ptr [EDX + 0x88]']:
            if pc==0x10039cc7:vm.r['EAX']=ENEMY;return
            calls.append('OwnAnchor');vm.r['EAX']=0x26000000 if case['anchor'] else 0;return
        if operand=='0x100077f0':
            objects[vm.r['ECX']]={'ChangeAimTurn':subject(vm.arg())};assert [vm.arg(i) for i in [1,2,3,4]]==[1,1,0,0];vm.r['EAX']=vm.r['ECX'];vm.popargs(5);return
        if operand=='0x1001ce40':objects[vm.r['ECX']]={'JumpTo':token(vm.arg())};vm.r['EAX']=vm.r['ECX'];vm.popargs(1);return
        if operand=='0x1002ec30':objects[vm.r['ECX']]={'Goto':dict(node=token(vm.arg()),tolerance=number(vm.arg(1)))};vm.r['EAX']=vm.r['ECX'];vm.popargs(2);return
        if operand=='0x10008050':
            assert vm.arg()==ENEMY and vm.arg(1)==0;objects[vm.r['ECX']]='TurnToEnemy';vm.r['EAX']=vm.r['ECX'];vm.popargs(2);return
        if operand=='0x10034830':objects[vm.r['ECX']]={'Sleep':number(vm.arg())};vm.r['EAX']=vm.r['ECX'];vm.popargs(1);return
        if operand=='0x10007c20':
            assert vm.arg()==ENEMY;objects[vm.r['ECX']]={'FollowEnemy':dict(distance=number(vm.arg(1)),mode=vm.arg(2))};vm.r['EAX']=vm.r['ECX'];vm.popargs(3);return
        if operand=='0x1003e3e0':goals.append(objects[vm.arg()]);vm.popargs(1);return
        raise AssertionError((hex(pc),operand))
    vm.run(0x100399a0,call)
    selected=next((c['Reserve'] for c in calls if isinstance(c,dict) and 'Reserve' in c),None)
    kind='Perch' if selected and case['perch'] and selected==case['perch']['id'] else 'JumpDest'
    return dict(selected=[kind,selected] if selected else None,goals=goals,clear_strafe=vm.load(BOT+0x4c0)&0x20==0,flags=dict(must_jump=bool(vm.load(STATE+0x90)&2),is_melee_attacking=bool(vm.load(STATE+0x90)&1)),calls=calls)

def post_melee(player,p,pp,draw):
    vm=VM();vm.r['EAX']=int(player);vm.r['ESI']=STATE;vm.store(STATE+0x88,bits(p));vm.store(STATE+0x8c,bits(pp));vm.store(STATE+0x90,0xa5);calls=[]
    def call(pc,operand):assert pc in [0x1003a2f5,0x1003a305];calls.append('Random');vm.r['EAX']=draw
    vm.run(0x1003a2f1,call,stops=(0x1003a729,0x1003a32f))
    return dict(requested=bool(vm.load(STATE+0x90)&2),draws=len(calls))

def node(id,x):return dict(id=id,position=[x,0.,0.],enabled=True,available=True,same_zone=True,class_matches=True,reachable=True,clear=True)
def query():return dict(origin=[0.,0.,0.],range=10000.,anchor_present=True,require_reachable=True,require_same_zone=True,require_clear=False)
searches=[];rng=random.Random(0x10012e00)
for _ in range(768):
    q=query();q.update({k:rng.choice([False,True]) for k in ['anchor_present','require_reachable','require_same_zone','require_clear']})
    q['origin']=[rng.uniform(-2000,2000) for _ in range(3)];q['range']=rng.choice([0.,1.,10000.,2e10])
    nodes=[node(i+1,rng.choice([0.,1.,10000.,1e10])) for i in range(rng.randrange(0,9))]
    for n in nodes:
        n['position']=[rng.choice([0.,1.,-1.,10000.,rng.uniform(-1000,1000)]) for _ in range(3)]
        n.update({k:rng.choice([False,True]) for k in ['enabled','available','same_zone','class_matches','reachable','clear']})
    searches.append(dict(nodes=nodes,query=q,expected=search(nodes,q)))
for x in [0.,1.,number(bits(10000.)-1),10000.,number(bits(10000.)+1),1e10]:
    q=query();nodes=[node(1,x),node(2,-x)];searches.append(dict(nodes=nodes,query=q,expected=search(nodes,q)))
for flags,options in itertools.product(range(64),range(16)):
    n=node(1,1.);n.update({k:bool(flags&(1<<i)) for i,k in enumerate(['enabled','available','same_zone','class_matches','reachable','clear'])})
    q=query();q.update({k:bool(options&(1<<i)) for i,k in enumerate(['anchor_present','require_reachable','require_same_zone','require_clear'])})
    searches.append(dict(nodes=[n],query=q,expected=search([n],q)))
plans=[]
positions=[None,[1.,0.,0.],[-1.,0.,0.],[2.,3.,4.],[10000.,-1.,100.],[4096.,1.,1.],[4096.,0.,0.]]
for p,j,clear,reachable,anchor in itertools.product(positions,positions,[False,True],[False,True],[None,17]):
    c=dict(origin=[0.,0.,0.],perch=dict(id=1,position=p) if p else None,jump=dict(id=2,position=j) if j else None,clear=clear,reachable=reachable,anchor=anchor)
    plans.append(dict(input=c,expected=plan(c)))
post=[]
for player,p,pp,draw in itertools.product([False,True],[0.,number(0x3e4ccccd),number(0x3f4ccccd),1.],[0.,number(0x3e4ccccd),number(0x3f4ccccd),1.],[0,1,6553,6554,26213,26214,32767]):
    post.append(dict(player=player,probability=p,player_probability=pp,random=draw,expected=post_melee(player,p,pp,draw)))
provenance=dict(binary_sha256=hashlib.sha256(raw).hexdigest(),assembly_sha256={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in assemblies},ranges=['10012e00..10012fcc','100399a0..10039ed2','1003a2f1..1003a328'],scope=__doc__)
(OUT/'defensive-oracle.json').write_text(json.dumps(dict(provenance=provenance,searches=searches,plans=plans,post_melee=post)),encoding='utf-8')
print(f'Recorded {len(searches)} searches, {len(plans)} goal plans and {len(post)} post-melee random branches')
