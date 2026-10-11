"""Bounded x86 replay of CalculateDamageFrom after VM argument decoding.

Original 104f83f3..104f8560 (exclusive); UClass GetDefaultObject/GetDefaultActor
are supplied snapshots. Arrays and ancestry are prepared host memory. No VM,
constructor, array-default merging, TakeDamage, game rules or Android execution.
"""
from pathlib import Path
import hashlib, itertools, json, re, struct

root = Path(__file__).resolve().parent.parent
out = root / 'analysis/milestone3c'
out.mkdir(exist_ok=True)
assembly = root / 'analysis/decompiled/damage-variance.asm'
binary = Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData/System/engine.dll')
raw = binary.read_bytes()
pe = struct.unpack_from('<I', raw, 60)[0]
base = struct.unpack_from('<I', raw, pe+52)[0]
optional = struct.unpack_from('<H', raw, pe+20)[0]
sections = [struct.unpack_from('<IIII', raw, pe+24+optional+40*i+8) for i in range(struct.unpack_from('<H',raw,pe+6)[0])]
def word(address):
    for size, va, count, offset in sections:
        if va <= address-base < va+count:
            return struct.unpack_from('<I',raw,offset+address-base-va)[0]
    raise AssertionError(hex(address))
def bits(v): return struct.unpack('<I',struct.pack('<f',v))[0]
def number(v): return struct.unpack('<f',struct.pack('<I',v))[0]
def signed(v): return v if v < 0x80000000 else v-0x100000000
assert word(0x10653578) == 0x3f800000
code = {int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z0-9.]+)(?: (.*))?$',assembly.read_text(),re.M) if 0x104f83f3 <= int(a,16) < 0x104f8560}
following = dict(zip(sorted(code),sorted(code)[1:]+[0x104f8560]))

def replay(case):
    r = dict(ESP=0x70001000, EDI=0x20000000, EBP=0, ESI=0, EAX=0, EBX=0, ECX=0, EDX=0)
    m = {}; xmm = {'XMM0':0}; defaults = {}; zero=False; less=False
    def put(a,v): m[a]=v & 0xffffffff
    def node(i): return 0 if i is None else 0x40000000+i*0x100
    def variance(class_address, cdo, include, exclude, status):
        defaults[class_address] = 0 if status == 'missing' else cdo
        put(cdo+0x24,0x1075d088 if status == 'valid' else 0x50000000)
        put(0x50000000+0x30,0)
        put(cdo+0x28,cdo+0x100); put(cdo+0x2c,len(include))
        for i, rule in enumerate(include):
            put(cdo+0x100+i*8,node(rule['class'])); put(cdo+0x104+i*8,rule['multiplier_bits'])
        put(cdo+0x30,cdo+0x300); put(cdo+0x34,len(exclude))
        for i, cls in enumerate(exclude): put(cdo+0x300+i*4,node(cls))
    variance(0x31000000,0x32000000,case['include'],case['exclude'],case['actor'])
    variance(0x33000000,0x34000000,case['override_include'],case['override_exclude'],case['explicit'] or 'valid')
    put(r['EDI']+0x1ec,0x31000000 if case['actor'] != 'missing' else 0)
    put(r['ESP']+0x10,0 if case['explicit'] is None else 0x33000000)
    put(r['ESP']+0xc,0x35000000 if case['has_type'] else 0)
    put(r['ESP']+0x14,case['damage_bits']); put(r['ESP']+0x20,0x36000000)
    ancestry = case['ancestry']
    put(0x35000000+0x24,node(ancestry[0]) if ancestry else 0)
    for i, cls in enumerate(ancestry): put(node(cls)+0x30,node(ancestry[i+1]) if i+1<len(ancestry) else 0)
    def address(operand):
        expr = operand.split('[',1)[1].split(']',1)[0]
        total=0
        for term in expr.replace(' ','').split('+'):
            factors = term.split('*')
            product=1
            for f in factors: product *= r[f] if f in r else int(f,0)
            total += product
        return total & 0xffffffff
    def get(operand):
        if operand in r: return r[operand]
        if operand == 'BL': return r['EBX'] & 255
        if '[' in operand:
            a=address(operand)
            return m[a] if a in m else word(a)
        return int(operand,0)
    def write(operand,value):
        value &= 0xffffffff
        if operand == 'BL': r['EBX']=(r['EBX'] & 0xffffff00)|(value & 255)
        elif operand == 'AL': r['EAX']=(r['EAX'] & 0xffffff00)|(value & 255)
        elif operand in r: r[operand]=value
        else: put(address(operand),value)
    def push(v): r['ESP']-=4; put(r['ESP'],v)
    def pop():
        value=m[r['ESP']]; r['ESP']+=4
        return value
    pc=0x104f83f3; steps=0
    while pc != 0x104f8560:
        steps+=1; assert steps<2000
        op,a=code[pc]; next_pc=following[pc]
        if op=='MOV': write(a[0],get(a[1]))
        elif op=='MOVSS':
            if a[0] in xmm: xmm[a[0]]=get(a[1])
            else: write(a[0],xmm[a[1]])
        elif op=='XORPS': assert a==['XMM0','XMM0']; xmm['XMM0']=0
        elif op=='MULSS': xmm[a[0]]=bits(number(xmm[a[0]])*number(get(a[1])))
        elif op=='PUSH': push(get(a[0]))
        elif op=='POP': write(a[0],pop())
        elif op=='XOR': write(a[0],get(a[0])^get(a[1])); zero=get(a[0])==0; less=signed(get(a[0]))<0
        elif op=='TEST': value=get(a[0])&get(a[1]); zero=value==0; less=signed(value)<0
        elif op=='CMP': zero=get(a[0])==get(a[1]); less=signed(get(a[0]))<signed(get(a[1]))
        elif op=='SHL': write(a[0],get(a[0])<<get(a[1]))
        elif op=='SAR': write(a[0],signed(get(a[0]))>>get(a[1]))
        elif op=='INC': write(a[0],get(a[0])+1)
        elif op=='SETZ': write(a[0],int(zero))
        elif op=='LEA': write(a[0],address(a[1]))
        elif op in ('NOP',): pass
        elif op=='CALL':
            if a==['dword ptr [0x1065027c]']: r['EAX']=defaults[r['ECX']]
            elif a==['dword ptr [0x106502cc]']: assert r['ECX']==0x35000000; r['EAX']=0x35000000
            else: raise AssertionError(a)
        elif op in ('JMP','JZ','JNZ','JLE','JL'):
            if op=='JMP' or (op=='JZ' and zero) or (op=='JNZ' and not zero) or (op=='JLE' and (less or zero)) or (op=='JL' and less): next_pc=int(a[0],16)
        else: raise AssertionError((hex(pc),op,a))
        pc=next_pc
    return xmm['XMM0']

def include(cls,v): return {'class':cls,'multiplier_bits':bits(v)}
scenarios=[([],[]),([include(2,.5),include(1,3.)],[1]),([include(None,.3)],[None]),([],[2]),([],[None]),([include(9,-2.)],[]),([include(1,-2.)],[]),([include(3,0.)],[2])]
cases=[]
for (inc,exc),ancestry,damage,actor,explicit,has_type in itertools.product(scenarios,[[1,2,3],[9,3],[]],[0,0x80000000,0x41a00000,0x42fa0000,0xc1a00000,0x7f800000,0x7fc00000],['valid','invalid','missing'],[None,'valid','invalid','missing'],[True,False]):
    c=dict(include=inc,exclude=exc,ancestry=ancestry,damage_bits=damage,actor=actor,explicit=explicit,has_type=has_type,override_include=[include(None,2.)],override_exclude=[])
    c['result_bits']=replay(c); cases.append(c)
# Additional snapshots use the selected original serialized IncludeTypes lists.
# This is a prepared-array example, not execution of CDO/default construction.
assets_path=out/'damage-assets.json'
assets=json.loads(assets_path.read_text(encoding='utf-8'))
classes={k.lower():v for k,v in assets['raw_classes'].items()}
ids={k:i+1 for i,k in enumerate(sorted(classes))}
examples=[]
for variance_name in ['properties.DV_GeonoshansWarriorDamage','properties.DV_OrganicCTSquadDamage']:
    data=classes[variance_name.lower()]['decoded_arrays']
    inc=[]
    for element in data['IncludeTypes']:
        fields={p['name']:p['value']['value'] for p in element['values']}
        ref=fields['DamageTypeClass']
        inc.append(include(None if ref['index']==0 else ids[ref['path'].lower()], fields['Multiplier']))
    exc=[None if i==0 else ids[path.lower()] for i,path in data.get('ExcludeTypes',[])]
    for type_name,damage in [('properties.CTDamageMeleeWarrior',125.),('properties.CTDamageEnergyDC17',20.),('properties.CTDamageEnergyDC17Commando',20.)]:
        ancestry=[]; k=type_name.lower()
        while k in classes:
            assert ids[k] not in ancestry
            ancestry.append(ids[k]); k=classes[k]['parent'].lower()
        c=dict(include=inc,exclude=exc,ancestry=ancestry,damage_bits=bits(damage),actor='valid',explicit=None,has_type=True,override_include=[],override_exclude=[])
        c['result_bits']=replay(c); cases.append(c)
        examples.append(dict(variance=variance_name,damage_type=type_name,input=damage,result=number(c['result_bits'])))
(out/'selected-variance-examples.json').write_text(json.dumps(dict(scope='Selected serialized array snapshots and DamageType ancestry, before armor/shields/game reduction; not native CDO construction',examples=examples),indent=2),encoding='utf-8')
provenance=dict(binary_sha256=hashlib.sha256(raw).hexdigest(),assembly_sha256=hashlib.sha256(assembly.read_bytes()).hexdigest(),assets_sha256=hashlib.sha256(assets_path.read_bytes()).hexdigest(),boundary=__doc__)
(out/'variance-oracle.json').write_text(json.dumps(dict(provenance=provenance,cases=cases)),encoding='utf-8')
print(f'Recorded {len(cases)} prepared native damage cases')
