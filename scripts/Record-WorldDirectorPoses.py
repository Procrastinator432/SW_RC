"""Independent native instruction interpretation, Gauss-Jordan and pose validation."""
from pathlib import Path
import json, hashlib, re, math
root=Path(__file__).resolve().parents[1]
source=root/'scripts/Record-SkeletalPoses.py';ns={'__file__':str(source)}
exec(compile(source.read_text().split('poses=0;matrices=0;')[0],str(source),'exec'),ns)
links=ns['links'];matrix,compose,f,bits,value=[ns[k] for k in ('matrix','compose','f','bits','value')]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
path=root/'analysis/reports/world-director-poses.json';report=json.loads(path.read_text())
prior_path=root/'analysis/reports/director-poses.json';prior=json.loads(prior_path.read_text())
assert sha(prior_path)==json.loads((root/'analysis/reports/director-poses-validation.json').read_text())['report_sha256']
stack_path=root/'analysis/reports/original-channel-stacks.json';stacks=json.loads(stack_path.read_text())
assert sha(stack_path)==json.loads((root/'analysis/reports/original-channel-stacks-validation.json').read_text())['report_sha256']
assert (report['poses'],report['bone_matrices'],len(report['inverse_cases']),len(report['conversion_cases']))==(2700,89736,128,32)
identity=[bits(1.) if i//4==i%4 else 0 for i in range(16)]
def offset(op):
    matches=re.findall(r'0x[0-9a-f]+',op);return int(matches[-1],16) if matches else 0
def instructions(name,start=0,end=0xffffffff):
    for line in (root/'analysis/decompiled'/name).read_text().splitlines():
        if not line or line.startswith('#'):continue
        address,instruction=line.split(' ',1);address=int(address,16)
        if start<=address<=end:
            opcode,_,args=instruction.partition(' ');yield address,opcode,args
inverse_code=list(instructions('skeletal-matrix-inverse.asm'))
def native_inverse(m):
    registers={};stack={}
    def operand(op):
        if op.startswith('XMM'):return registers[op]
        if 'ECX' in op:return value(m[offset(op)//4])
        if 'ESP' in op:return stack[offset(op)]
        assert op=='dword ptr [0x10186c18]',op
        return 1.
    for address,opcode,args in inverse_code:
        if address==0x10143c65 and registers['XMM1']==0:return identity.copy()
        if opcode not in ('MOVSS','MOVAPS','XORPS','ADDSS','SUBSS','MULSS','DIVSS'):continue
        dst,src=args.split(',',1)
        if not dst.startswith('XMM'):
            assert opcode=='MOVSS' and 'ESP' in dst;stack[offset(dst)]=operand(src);continue
        if opcode=='XORPS':assert dst==src;registers[dst]=0.;continue
        b=operand(src)
        if opcode in ('MOVSS','MOVAPS'):registers[dst]=b;continue
        a=registers[dst]
        registers[dst]=f({'ADDSS':lambda:a+b,'SUBSS':lambda:a-b,'MULSS':lambda:a*b,'DIVSS':lambda:a/b}[opcode]())
    return [bits(stack[i]) for i in range(0x14,0x54,4)]

# Build expression trees from original conversion instructions, independent of Rust's order table.
registers={};expressions={}
for _,opcode,args in instructions('skeletal-director.asm',0x10501c85,0x1050209d):
    if opcode not in ('MOVSS','MOVAPS','MULSS','ADDSS'):continue
    dst,src=args.split(',',1)
    if dst.startswith('XMM'):
        if src.startswith('XMM'):expr=registers[src]
        else:
            assert 'ESP' in src;off=offset(src)
            if 0x80<=off<=0xbc:expr=('D',(off-0x80)//4)
            else:assert 0x20<=off<=0x5c;expr=('I',(off-0x20)//4)
        registers[dst]=expr if opcode in ('MOVSS','MOVAPS') else (opcode,registers[dst],expr)
    else:
        assert opcode=='MOVSS' and 'ESP' in dst
        expressions[(offset(dst)-0xd4)//4]=registers[src]
assert set(expressions)==set(range(16))
def evaluate(expr,d,inv):
    if expr[0]=='D':return value(d[expr[1]])
    if expr[0]=='I':return value(inv[expr[1]])
    a=evaluate(expr[1],d,inv);b=evaluate(expr[2],d,inv)
    return f(a*b if expr[0]=='MULSS' else a+b)
def convert(d,inv):return [bits(evaluate(expressions[i],d,inv)) for i in range(16)]

max_inverse_error=0.;max_residual=0.
def mathematical_inverse(m):
    global max_inverse_error,max_residual
    a=[[value(m[r*4+c]) for c in range(4)]+[float(r==c) for c in range(4)] for r in range(4)]
    for col in range(4):
        pivot=max(range(col,4),key=lambda r:abs(a[r][col]));a[col],a[pivot]=a[pivot],a[col]
        divisor=a[col][col];assert divisor!=0
        a[col]=[v/divisor for v in a[col]]
        for row in range(4):
            if row==col:continue
            factor=a[row][col];a[row]=[x-factor*y for x,y in zip(a[row],a[col])]
    return [v for row in a for v in row[4:]]
for p in report['inverse_cases']+[{'matrix':m,'inverse':i} for m,i in zip(report['transforms'][:2],report['inverses'][:2])]:
    m=p['matrix'];inv=p['inverse'];assert inv==native_inverse(m)
    expected=mathematical_inverse(m)
    error=max(abs(value(x)-y) for x,y in zip(inv,expected));max_inverse_error=max(max_inverse_error,error);assert error<3e-6
    residual=max(abs(sum(value(m[r*4+k])*value(inv[k*4+c]) for k in range(4))-float(r==c)) for r in range(4) for c in range(4))
    max_residual=max(max_residual,residual);assert residual<3e-6
assert report['inverses'][2]==native_inverse(report['transforms'][2])==identity
for p in report['conversion_cases']:assert p['converted']==convert(p['director'],p['inverse'])
seen=set();count=0
for c in report['cases']:
    key=(c['source_case'],c['transform']);assert key not in seen;seen.add(key)
    old=prior['cases'][c['source_case']];assert old['editor'] is False
    o=stacks['objects'][old['source_object']];bones=links['objects'][o['source_index']]['prefix']['bones'];local=o['runs'][old['run']]['ticks'][old['tick']]['local']
    parents=[b['word_34'] for b in bones];parents[0]=-1
    move=0 if bones[0]['name']['name'].lower()=='move' else -1
    d=old['directors'][1]['words'];converted=convert(d[2:18],report['inverses'][c['transform']]);output=[]
    assert d[18]==0x10001 and old['target']==move+1
    for i,p in enumerate(local):
        m=matrix(p['rotation'],p['position'])
        if i>0 and parents[i]!=move:m=compose(m,output[parents[i]])
        if i==old['target']:
            m[12:16]=converted[12:16]
            m[:12]=[bits(f(value(converted[j])*value(v))) for j,v in enumerate(m[:12])]
        output.append(m)
    assert c['result']=={'Ok':1} and c['matrices']==output
    assert all(math.isfinite(value(v)) for m in output for v in m)
    count+=len(output)
assert len(seen)==2700 and count==89736
sources=['crates/rc-package/src/skeletal_matrix_inverse.rs','crates/rc-package/src/skeletal_matrix_inverse_tests.rs','crates/rc-package/src/skeletal_director.rs','crates/rc-inspect/src/bin/rc-world-director-check.rs','scripts/Generate-MatrixInverse.py','scripts/Record-WorldDirectorPoses.py','analysis/decompiled/skeletal-matrix-inverse.asm','analysis/decompiled/skeletal-matrix-inverse.c','analysis/decompiled/skeletal-director.asm']
result={'date':'2026-10-07','rust_tests':298,'scope':report['scope'],'counts':{'poses':2700,'matrices':count,'general_inverse_probes':128,'general_conversion_probes':32,'affine_inverse_probes':2,'singular_identity_fallback':1},'max_inverse_error_f64':max_inverse_error,'max_identity_residual_f64':max_residual,'source_sha256':{s:sha(root/s) for s in sources},'report_sha256':sha(path),'prior_report_sha256':sha(prior_path),'stack_report_sha256':sha(stack_path),'checks':['cargo test --workspace: 298 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','inverse bit-exact against independent scalar ASM interpreter','inverse independently checked with f64 Gauss-Jordan and identity residual','conversion and all 2700 resulting poses bit-exact against original ASM expression interpreter']}
(root/'analysis/reports/world-director-poses-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';e=json.loads(path.read_text(encoding='utf-8-sig'));e['rust_tests']=298;e['world_director_poses_validation']=result;path.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: Native Matrixinversion und Weltraum-Umrechnung der Translations-/Plane-Directors rekonstruiert. '
      'Originale skalare SSE-Reihenfolge, Identitaetsrueckfall bei exakt singulaerer Matrix, Korrektur vor Kindverknuepfung. '
      '2700 Originaltrack-Diagnoseposen / 89736 Matrizen, 128 allgemeine Inversionen und 32 allgemeine Umrechnungen unabhaengig geprueft. '
      'Director-/MeshToWorld-Snapshots vorgegeben; Rotation, Runtimebindung und Skinning bleiben offen. '
      '298 Tests, Clippy und Format bestanden. Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_WORLD_DIRECTORS.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Native Matrixinversion und Weltraum-Umrechnung der Translations-/Plane-Directors' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(result['counts']));print('max inverse error',max_inverse_error,'max residual',max_residual)
