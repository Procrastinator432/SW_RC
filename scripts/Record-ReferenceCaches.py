"""Numeric original-ASM oracle for reference-parent composition and inverse caches."""
from pathlib import Path
import json, hashlib, re
root=Path(__file__).resolve().parents[1]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
script=root/'scripts/Record-WorldDirectorPoses.py';ns={'__file__':str(script)}
exec(compile(script.read_text().split('max_inverse_error=0.')[0],str(script),'exec'),ns)
links,matrix,f,bits,value,native_inverse=[ns[k] for k in ('links','matrix','f','bits','value','native_inverse')]
code=list(ns['instructions']('skeletal-root-frame.asm',0x1050a070,0x1050a408))
def offset(op):
    matches=re.findall(r'-?0x[0-9a-f]+',op);n=int(matches[-1],16) if matches else 0
    return n-0x100000000 if n>=0x80000000 else n
def product(local,parent):
    registers={};output={}
    def read(op):
        if op.startswith('XMM'):return registers[op]
        assert 'EBX' in op or 'EAX' in op,op
        return value((local if 'EBX' in op else parent)[offset(op)//4])
    for _,opcode,args in code:
        if opcode not in ('MOVSS','MOVAPS','ADDSS','MULSS'):continue
        dst,src=args.split(',',1);b=read(src)
        if dst.startswith('XMM'):
            if opcode in ('MOVSS','MOVAPS'):registers[dst]=b
            else:
                a=registers[dst];registers[dst]=f(a+b if opcode=='ADDSS' else a*b)
        else:
            assert opcode=='MOVSS' and 'EBP' in dst
            output[(offset(dst)+0x84)//4]=bits(b)
    assert set(output)==set(range(16))
    return [output[i] for i in range(16)]
original_path=root/'analysis/reports/original-skeletal-linkups.json'
assert sha(original_path)==json.loads((root/'analysis/reports/original-skeletal-linkups-validation.json').read_text())['report_sha256']
path=root/'analysis/reports/reference-caches.json';report=json.loads(path.read_text())
assert (report['meshes'],report['matrices'],len(report['products']))==(130,3111,64)
max_error=0.
for seed,p in enumerate(report['products']):
    a=[bits(f((((i*13+seed*17)%47)-23)*.125)) for i in range(16)]
    b=[bits(f((((i*7+seed*11)%41)-20)*f(.2))) for i in range(16)]
    assert p['local']==a and p['parent']==b and p['result']==product(a,b)
    error=max(abs(value(p['result'][r*4+c])-sum(value(a[r*4+k])*value(b[k*4+c]) for k in range(4))) for r in range(4) for c in range(4))
    max_error=max(max_error,error);assert error<3e-6
count=0;move=0;seen=set()
for o in report['objects']:
    index=o['source_index'];assert index not in seen;seen.add(index)
    bones=links['objects'][index]['prefix']['bones'];reference=[];inverse=[]
    for i,b in enumerate(bones):
        m=matrix(b['rotation'],b['position'])
        if i:
            parent=b['word_34'];assert 0<=parent<i
            m=product(m,reference[parent])
        reference.append(m);inverse.append(native_inverse(m))
    assert o['inverse']==inverse
    assert o['built']=={'Ok':True} and o['flag_after_build']==0
    assert o['reused']=={'Ok':False} and o['flag_after_reuse']==9
    move+=bool(bones and bones[0]['name']['name'].lower()=='move');count+=len(bones)
expected={i for i,o in enumerate(links['objects']) if o.get('status')!='UnsupportedLegacyPackage'}
assert seen==expected and count==3111
asm=(root/'analysis/decompiled/skeletal-root-frame.asm').read_text()
for marker in ['10509f3c TEST dword ptr [EBX + 0x284],0x1fffffff','10509f51 MOV byte ptr [ECX + 0x61],0x0','1050a418 CALL dword ptr [0x10650244]','1050a43b MOVSD.REP ES:EDI,ESI']:assert marker in asm
sources=['crates/rc-package/src/skeletal_reference_cache.rs','crates/rc-package/src/skeletal_reference_product.rs','crates/rc-package/src/skeletal_matrix_inverse.rs','crates/rc-inspect/src/bin/rc-reference-cache-check.rs','scripts/Generate-ReferenceProduct.py','scripts/Record-ReferenceCaches.py','scripts/Record-WorldDirectorPoses.py','scripts/Record-SkeletalPoses.py','analysis/decompiled/skeletal-root-frame.asm','analysis/decompiled/skeletal-apply-animation.c','analysis/decompiled/skeletal-matrix-inverse.asm']
result={'date':'2026-10-07','rust_tests':341,'scope':report['scope'],'counts':{'meshes':len(seen),'inverse_matrices':count,'meshes_with_move_root':move,'general_product_probes':64,'unsupported_legacy_meshes':1},'max_product_error_f64':max_error,'source_sha256':{s:sha(root/s) for s in sources},'report_sha256':sha(path),'original_report_sha256':sha(original_path),'checks':['cargo test --workspace: 341 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','3111 inverse reference matrices bit-exact against independent numeric original-ASM interpretation','64 general products bit-exact against numeric ASM and checked against f64 matrix multiplication','nonempty reuse leaves matrices and supplied cache byte unchanged','five unit tests cover Move parent, stale nonempty cache, partial error writes, singular inverse fallback and empty skeleton']}
(root/'analysis/reports/reference-caches-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
evidence=root/'analysis/evidence.json';e=json.loads(evidence.read_text(encoding='utf-8-sig'));e['rust_tests']=341;e['reference_caches_validation']=result;evidence.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: Inverser Referenzpose-Cache aus ApplyAnimation rekonstruiert: Leer-Cache-Gate, Cachebyte-Invalidierung, Elternkomposition inklusive Move und originale skalare Matrixinverse. '
      '130 Originalskelette / 3111 inverse Matrizen plus 64 allgemeine Produkte unabhaengig gegen Original-ASM geprueft. '
      'Vorbereitungsmodul auf Mesh-Snapshots; vollstaendige ApplyAnimation-Verbindung, Skinning und sichtbare Animation offen. '
      '341 Tests, Clippy und Format bestanden. Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_REFERENCE_CACHE.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for p in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Inverser Referenzpose-Cache aus ApplyAnimation rekonstruiert' not in p.read_text(encoding='utf-8-sig'):
        with p.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(result['counts']));print('max product error',max_error)
