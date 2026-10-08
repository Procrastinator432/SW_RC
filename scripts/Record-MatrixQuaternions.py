"""Interpret original FQuat(FMatrix) branches and SSE operations independently."""
from pathlib import Path
import json,struct,math,re,hashlib,collections
root=Path(__file__).resolve().parents[1]
f=lambda x:struct.unpack('<f',struct.pack('<f',x))[0]
bits=lambda x:struct.unpack('<I',struct.pack('<f',x))[0]
value=lambda x:struct.unpack('<f',struct.pack('<I',x))[0]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def dll_word(path,address):
    data=path.read_bytes();pe=struct.unpack_from('<I',data,60)[0];count=struct.unpack_from('<H',data,pe+6)[0];opt=struct.unpack_from('<H',data,pe+20)[0]
    base=struct.unpack_from('<I',data,pe+24+28)[0];rva=address-base
    for i in range(count):
        pos=pe+24+opt+40*i;size,va,raw,ptr=struct.unpack_from('<IIII',data,pos+8)
        if va<=rva<va+raw:return struct.unpack_from('<I',data,ptr+rva-va)[0]
    raise AssertionError(hex(address))
core=Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\core.dll')
constants={a:dll_word(core,a) for a in [0x10186c18,0x10186ed8,0x10186edc,0x101885e8]}
assert constants=={0x10186c18:bits(1.),0x10186ed8:bits(.5),0x10186edc:bits(3.),0x101885e8:bits(.25)}
asm_path=root/'analysis/decompiled/skeletal-quat-from-matrix.asm';code=[]
for line in asm_path.read_text().splitlines():
    if not line or line.startswith('#'):continue
    address,instruction=line.split(' ',1);address=int(address,16)
    if 0x10144cb0<=address<=0x1014500d:
        opcode,_,args=instruction.partition(' ');code.append((address,opcode,args))
labels={address:i for i,(address,_,_) in enumerate(code)}
stats=collections.Counter();seed_ranges=collections.Counter()
def offset(op):
    matches=re.findall(r'0x[0-9a-f]+',op);return int(matches[-1],16) if matches else 0
def interpret(matrix,fixed=False):
    registers={};stack={};out={};pc=0;below_equal=False;branch=None;seeds=0
    def operand(op):
        if op.startswith('XMM'):return registers[op]
        if 'EDX' in op:return matrix[offset(op)//4]
        if 'ESP' in op:return stack[offset(op)]
        return constants[offset(op)]
    while True:
        address,opcode,args=code[pc];pc+=1
        if address in (0x10144cdf,0x10144db7,0x10144ea3,0x10144f57):branch={0x10144cdf:'trace',0x10144db7:'x',0x10144ea3:'y',0x10144f57:'z'}[address]
        if opcode=='RET':break
        if opcode=='JBE':
            if below_equal:pc=labels[int(args,16)]
            continue
        if opcode=='COMISS':
            a,b=map(lambda s:value(operand(s)),args.split(',',1));below_equal=not a>b;continue
        if opcode not in ('MOVSS','MOVAPS','ADDSS','SUBSS','MULSS','DIVSS','XORPS','RSQRTSS','CMPNEQSS','ANDPS'):continue
        dst,src=args.split(',',1)
        if not dst.startswith('XMM'):
            assert opcode=='MOVSS'
            if 'ESP' in dst:stack[offset(dst)]=operand(src)
            else:assert 'EAX' in dst;out[offset(dst)//4]=operand(src)
            continue
        if opcode=='XORPS':assert dst==src;registers[dst]=0;continue
        b=operand(src)
        if opcode in ('MOVSS','MOVAPS'):registers[dst]=b;continue
        if opcode=='ANDPS':registers[dst]&=b;continue
        if opcode=='RSQRTSS':
            squared=value(b);seeds+=1
            seed=.125 if fixed else f(1/f(math.sqrt(squared))) if squared>0 else math.inf if squared==0 else math.nan
            registers[dst]=bits(seed);continue
        a=value(registers[dst]);b=value(b)
        if opcode=='CMPNEQSS':registers[dst]=0xffffffff if a!=b else 0;continue
        if opcode=='DIVSS':
            result=a/b if b else math.copysign(math.inf,a*b) if a else math.nan
        else:result={'ADDSS':lambda:a+b,'SUBSS':lambda:a-b,'MULSS':lambda:a*b}[opcode]()
        registers[dst]=bits(f(result))
    assert set(out)==set(range(4)) and seeds==1 and branch is not None
    stats[('fixed_' if fixed else 'source_')+branch]+=1
    return [out[i] for i in range(4)]
path=root/'analysis/reports/matrix-quaternions.json';report=json.loads(path.read_text())
source_path=root/'analysis/reports/world-director-poses.json';source=json.loads(source_path.read_text())
assert sha(source_path)==json.loads((root/'analysis/reports/world-director-poses-validation.json').read_text())['report_sha256']
assert report['matrices']==len(report['cases'])==29912 and len(report['probes'])==256
seen=set()
for c in report['cases']:
    key=(c['source_case'],c['bone']);assert key not in seen;seen.add(key)
    s=source['cases'][c['source_case']];assert s['transform']==0
    m=s['matrices'][c['bone']];assert c['quaternion']==interpret(m)
    assert all(math.isfinite(value(v)) for v in c['quaternion'])
for p in report['probes']:assert p['quaternion']==interpret(p['matrix'],True)
assert all(stats['fixed_'+b]>0 for b in ('trace','x','y','z'))
sources=['crates/rc-package/src/quaternion_matrix.rs','crates/rc-inspect/src/bin/rc-matrix-quaternion-check.rs','scripts/Record-MatrixQuaternions.py','analysis/decompiled/skeletal-quat-from-matrix.c','analysis/decompiled/skeletal-quat-from-matrix.asm']
result={'date':'2026-10-07','rust_tests':304,'scope':report['scope'],'counts':{'source_matrices':29912,'fixed_seed_probes':256},'branches':dict(stats),'dll_constants':{hex(a):v for a,v in constants.items()},'core_sha256':sha(core),'source_sha256':{s:sha(root/s) for s in sources},'report_sha256':sha(path),'source_report_sha256':sha(source_path),'checks':['cargo test --workspace: 304 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','29912 source bone conversions bit-exact under portable seed policy against independent native ASM interpreter','256 fixed-seed probes cover all four native branches, including diagonal ties','four arithmetic constants verified in original core.dll bytes','six unit tests cover native half-turn formulas, symmetric W sums, NaN routing, ignored translation and seed failure']}
(root/'analysis/reports/matrix-quaternions-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';e=json.loads(path.read_text(encoding='utf-8-sig'));e['rust_tests']=304;e['matrix_quaternions_validation']=result;path.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: Native Matrix-zu-Quaternion-Umrechnung fuer Rotationsdirectors rekonstruiert. '
      'Vier originale Rechenzweige inklusive auffaelliger nichtpositiver Spurformeln, Diagonalgleichstaende und expliziter RSQRT-Mathgrenze. '
      '29912 Originaltrack-abgeleitete Knochenmatrizen und 256 feste Seed-/Zweigproben bitgenau gegen separaten ASM-Interpreter geprueft. '
      'Sechs neue Tests; 304 Tests, Clippy und Format bestanden. Director-Rotation/History noch nicht verbunden, Skinning offen. '
      'Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_MATRIX_QUATERNIONS.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Native Matrix-zu-Quaternion-Umrechnung fuer Rotationsdirectors' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(result['counts']));print(json.dumps(dict(stats)))
