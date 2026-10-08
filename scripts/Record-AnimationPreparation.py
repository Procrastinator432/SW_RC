"""Independent array-content/gate/order oracle, original skeleton inverse cross-check."""
from pathlib import Path
import json,hashlib,struct,re,collections
root=Path(__file__).resolve().parents[1];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
path=root/'analysis/reports/animation-preparation.json';report=json.loads(path.read_text())
def data(count,size,base):return [[base+i*size+j for j in range(size)] for i in range(count)]
def resize(values,count,size):return values[:count]+[[0]*size for _ in range(max(0,count-len(values)))]
seen=set();stats=collections.Counter()
for c in report['instance_cases']:
    n,q,p,m,b=[c[k] for k in ('count','q','p','m','byte_before')];key=(n,q,p,m,b);assert key not in seen;seen.add(key)
    local=q!=n or p!=n;mat=m!=n
    assert c['result']=={'local_resized':local,'matrices_resized':mat}
    assert c['byte_after']==(0 if local or mat else b)
    assert c['after']=={'rotations':resize(data(q,4,0x7fc10000),n,4),'positions':resize(data(p,3,0x80000000),n,3),'matrices':resize(data(m,16,0x3f000000),n,16),'mesh_to_world':[66]*16}
    stats['instance_cases']+=1;stats['instance_invalidations']+=bool(local or mat)
assert seen=={(n,q,p,m,b) for n in range(6) for q in range(6) for p in range(6) for m in range(6) for b in [0,7]}
seen=set()
for c in report['scratch_cases']:
    n,q,p=[c[k] for k in ('count','q','p')];key=(n,q,p);assert key not in seen;seen.add(key)
    grown=q<n;rot=data(q,4,0x7fc10000);pos=data(p,3,0x80000000)
    assert c['resized']==grown
    assert c['after']=={'rotations':resize(rot,n,4) if grown else rot,'positions':resize(pos,n,3) if grown else pos}
    stats['scratch_cases']+=1;stats['scratch_resizes']+=grown
assert seen=={(n,q,p) for n in range(6) for q in range(6) for p in range(6)}
original_path=root/'analysis/reports/original-skeletal-linkups.json';original=json.loads(original_path.read_text())
assert sha(original_path)==json.loads((root/'analysis/reports/original-skeletal-linkups-validation.json').read_text())['report_sha256']
inverse_path=root/'analysis/reports/reference-caches.json';inverses=json.loads(inverse_path.read_text())
assert sha(inverse_path)==json.loads((root/'analysis/reports/reference-caches-validation.json').read_text())['report_sha256']
inverse_map={o['source_index']:o['inverse'] for o in inverses['objects']}
seen=set()
for o in report['objects']:
    index=o['source_index'];assert index not in seen;seen.add(index);source=original['objects'][index];bones=source['prefix']['bones'];n=len(bones);links=len(source['mappings'])
    assert o['linkups']==links
    assert o['built']=={'buffers':{'local_resized':min(2,n)!=n or min(1,n)!=n,'matrices_resized':n!=0},'inverse_built':True,'linkups':links}
    assert o['reused']=={'buffers':{'local_resized':False,'matrices_resized':False},'inverse_built':False,'linkups':links}
    assert o['cache_build']==0 and o['cache_reuse']==7
    assert o['inverse']==inverse_map[index]
    matrix=[0]*16
    for i in [0,5,10,15]:matrix[i]=0x3f800000
    matrix[12]=struct.unpack('<I',struct.pack('<f',index))[0]
    assert o['buffers']=={'rotations':resize([b['rotation'] for b in bones[:2]],n,4),'positions':resize([b['position'] for b in bones[:1]],n,3),'matrices':[[0]*16 for _ in bones],'mesh_to_world':matrix}
    assert o['events']==(['transform']+[f'linkup {i}' for i in range(links)])*2
    stats['original_meshes']+=1;stats['original_inverse_matrices']+=n;stats['ordered_linkup_host_calls']+=links*2;stats['transform_host_calls']+=2
assert seen==set(inverse_map)
asm=(root/'analysis/decompiled/skeletal-root-frame.asm').read_text()
for marker in ['10509eb3 MOV byte ptr [ESI + 0x61],0x0','10509ecb CALL 0x103c3c70','10509ee4 CALL 0x103346b0','10509f14 CALL 0x103c3b90','10509f24 CALL dword ptr [EDX + 0xe8]','1050a4ae CALL 0x10509430','1050a648 JGE 0x1050a671']:assert marker in asm,marker
helpers=(root/'analysis/decompiled/skeletal-buffer-resize.asm').read_text()
for marker in ['103c3cc8 STOSD.REP ES:EDI','1033470c STOSD.REP ES:EDI','103c3be8 STOSD.REP ES:EDI']:assert marker in helpers,marker
storage=(root/'analysis/decompiled/skeletal-buffer-storage.c').read_text();assert storage.count('memmove(')==3 and storage.count('& 0x1fffffff')==3
log=(root/'analysis/reports/preparation-tests.log').read_text();assert sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)))==356 and 'test result: FAILED' not in log
sources=['crates/rc-package/src/skeletal_preparation.rs','crates/rc-package/src/skeletal_preparation_tests.rs','crates/rc-package/src/skeletal_reference_cache.rs','crates/rc-inspect/src/bin/rc-preparation-check.rs','scripts/Record-AnimationPreparation.py','analysis/decompiled/skeletal-buffer-resize.c','analysis/decompiled/skeletal-buffer-resize.asm','analysis/decompiled/skeletal-buffer-storage.c','analysis/decompiled/skeletal-root-frame.asm']
result={'date':'2026-10-08','rust_tests':356,'scope':report['scope'],'counts':dict(stats),'source_sha256':{s:sha(root/s) for s in sources},'report_sha256':sha(path),'original_report_sha256':sha(original_path),'inverse_report_sha256':sha(inverse_path),'checks':['cargo test --workspace: 356 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','2592 instance-array states independently checked including raw NaN/sign bits, zero append, shrink and cache preservation','216 scratch-array states independently checked including quaternion-only gate and position shrink','130 original skeleton preparations and cached reuses: all buffer words, supplied transform, ordered host events and inverse snapshots checked','3111 inverse matrices equal prior independently original-ASM-verified report','nine tests cover array gates and ordered partial states at transform/inverse/linkup error boundaries']}
(root/'analysis/reports/animation-preparation-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
evidence=root/'analysis/evidence.json';e=json.loads(evidence.read_text(encoding='utf-8-sig'));e['rust_tests']=356;e['animation_preparation_validation']=result;evidence.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-08: Animations-Instanzpuffer mit prefixerhaltender Groessenaenderung und Null-Anhang sowie separatem Quaternion-Scratch-Gate rekonstruiert. '
      'Vorbereitung in Originalreihenfolge mit Transform-Host, Referenzinverse und Linkup-Hosts verbunden; 2592 Instanz- und 216 Scratchzustaende sowie 130 Originalskelette inklusive Cachewiederholung unabhaengig geprueft. '
      '356 Tests, Clippy und Format bestanden. Speicherinhalt statt nativer Heapflags; Transform/Linkup-Hosts geliefert, Verbindung zum Root-/Vollpose-Einstieg und Skinning offen. Android zum Schluss. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_PREPARATION.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for p in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Animations-Instanzpuffer mit prefixerhaltender Groessenaenderung' not in p.read_text(encoding='utf-8-sig'):
        with p.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(dict(stats)))
