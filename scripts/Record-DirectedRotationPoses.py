"""Independent full director pose oracle, with numeric original product interpreter."""
from pathlib import Path
import json,hashlib,re,collections,math
root=Path(__file__).resolve().parents[1]
script=root/'scripts/Record-DirectorRotations.py';ns={'__file__':str(script)}
exec(compile(script.read_text().split('seen=set();counts=collections.Counter()')[0],str(script),'exec'),ns)
f,bits,value,matrix,convert,slerp,power,angle_difference,angle_fast=[ns[k] for k in ('f','bits','value','matrix','convert','slerp','power','angle_difference','angle_fast')]
links=ns['pose_ns']['links'];compose=ns['pose_ns']['compose'];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
stack_path=root/'analysis/reports/original-channel-stacks.json';stacks=json.loads(stack_path.read_text())
assert sha(stack_path)==json.loads((root/'analysis/reports/original-channel-stacks-validation.json').read_text())['report_sha256']
prior_path=root/'analysis/reports/director-poses.json';prior=json.loads(prior_path.read_text())
assert sha(prior_path)==json.loads((root/'analysis/reports/director-poses-validation.json').read_text())['report_sha256']
path=root/'analysis/reports/directed-rotation-poses.json';report=json.loads(path.read_text())
assert (report['poses'],report['bone_matrices'],len(report['cases']),len(report['products']))==(7200,239296,7200,64)
mesh=list(map(bits,[2.,0.,0.,0.,0.,4.,0.,0.,0.,0.,.5,0.,10.,20.,30.,1.]))
inverse=list(map(bits,[.5,0.,0.,0.,0.,.25,0.,0.,0.,0.,2.,0.,-5.,-5.,-60.,1.]))
assert report['mesh_to_world']==mesh and report['actor_scale']==list(map(bits,[2.,.5,3.]))
assert ns['source']['transforms'][0]==mesh and ns['source']['inverses'][0]==inverse
def world_to_local(d):
    d=list(map(value,d));inv=list(map(value,inverse));out=[]
    for row in range(4):
        for col in range(4):
            terms=[f(d[row*4+k]*inv[k*4+col]) for k in [3,2,1,0]]
            out.append(bits(f(f(f(terms[0]+terms[1])+terms[2])+terms[3])))
    return out
code=[]
for line in (root/'analysis/decompiled/skeletal-director.asm').read_text().splitlines():
    if not line or line.startswith('#'):continue
    address,instruction=line.split(' ',1);opcode,_,args=instruction.partition(' ');code.append((int(address,16),opcode,args))
kernels={False:[x for x in code if 0x105022fb<=x[0]<=0x1050276f],True:[x for x in code if 0x10502921<=x[0]<=0x10502c84]}
def offset(op):
    words=re.findall(r'0x[0-9a-f]+',op);return int(words[-1],16) if words else 0
def product(current,director,ancestor=False):
    # Numeric execution with mutable memory, not generated expression evaluation.
    memory=list(map(value,current));d=list(map(value,director));stack={};registers={};integer_registers={};base=0
    def operand(op):
        if op.startswith('XMM'):return registers[op]
        off=offset(op)
        if 'ESP' in op:
            first=0x20 if ancestor else 0x80
            if first<=off<first+64:return d[(off-first)//4]
            return stack[off]
        if 'EBP' in op:return memory[off//4]
        assert 'EAX' in op
        return memory[((0 if 'ESI' in op else base)+off)//4]
    for _,opcode,args in kernels[ancestor]:
        if ancestor and opcode=='MOV' and args=='EAX,dword ptr [ECX + 0xa0]':base=0;continue
        if ancestor and opcode=='LEA' and args.startswith('EAX,[EAX + ESI'):base=offset(args);continue
        if ancestor and opcode=='MOV':
            dst,src=args.split(',',1)
            if src.startswith('dword ptr [ESP'):integer_registers[dst]=operand(src)
            elif dst.startswith('dword ptr [EAX'):memory[(base+offset(dst))//4]=integer_registers[src]
            continue
        if opcode not in ('MOVSS','MOVAPS','ADDSS','MULSS'):continue
        dst,src=args.split(',',1)
        if dst.startswith('XMM'):
            v=operand(src)
            registers[dst]=v if opcode in ('MOVSS','MOVAPS') else f(registers[dst]*v if opcode=='MULSS' else registers[dst]+v)
        else:assert opcode=='MOVSS' and 'ESP' in dst;stack[offset(dst)]=operand(src)
    return list(map(bits,memory if ancestor else [stack[i] for i in range(0xd4,0x114,4)]))
max_product_error=0.
for p in report['products']:
    m,d=p['current'],p['director'];assert p['relative']==product(m,d) and p['ancestor']==product(m,d,True)
    reference=[sum(value(m[r*4+k])*value(d[k*4+c]) for k in range(4)) for r in range(4) for c in range(4)]
    max_product_error=max(max_product_error,max(abs(value(v)-e) for v,e in zip(p['relative'],reference)))
    assert max_product_error<3e-6
    assert p['ancestor'][12:16]==m[12:16]
    assert max(abs(value(v)-e) for v,e in zip(p['ancestor'][:12],reference[:12]))<3e-6
stats=collections.Counter()
def apply(d,bone,output):
    working=d[2:18].copy() if d[19]&1 else world_to_local(d[2:18]);current=output[bone].copy()
    if d[18]&0xff00:
        if not d[27]&0xff:d[22:26]=convert(current);d[27]=(d[27]&0xffffff00)|1;stats['history_initializations']+=1
        q=convert(working);scale=report['actor_scale'].copy();translation=working[12:15].copy()
        if value(d[21])>=0:
            distance=angle_difference(d[22:26],q);budget=value(d[26])
            if distance>budget:
                q=slerp(d[22:26],q,f(budget/distance));working=matrix(q,translation);scale=[bits(1.)]*3;stats['temporal_limits']+=1
            d[26]=0
        if value(d[20])>=0:
            angle=angle_fast(q)
            if angle>value(d[20]):
                q=power(q,f(value(d[20])/angle));working=matrix(q,translation);scale=[bits(1.)]*3;stats['absolute_limits']+=1
        d[22:26]=q
        if d[19]&2:working=product(current,working);stats['relative_compositions']+=1
        output[bone][:12]=[bits(f(value(scale[i%4])*value(working[i]))) if i%4!=3 else bits(f(0.*value(working[i]))) for i in range(12)]
        first=d[1]&0x7fffffff;first=first-0x80000000 if first&0x40000000 else first
        if d[19]&2 and first<bone:
            for previous in range(bone-1,first-1,-1):
                alpha=f(f(previous-first+1)/f(bone-first+1));rotation=power(q,alpha)
                output[previous]=product(output[previous],matrix(rotation,[0,0,0]),True);stats['previous_bone_corrections']+=1
    if d[18]&0xff:output[bone][12:16]=working[12:16]
    if d[18]&0xff0000:output[bone][:12]=[bits(f(value(working[i])*value(v))) for i,v in enumerate(output[bone][:12])]
seen=set();last={};matrix_count=0
for c in report['cases']:
    si,scenario,iteration=c['source_case'],c['scenario'],c['iteration'];key=(si,scenario,iteration);assert key not in seen;seen.add(key)
    source=prior['cases'][si];assert not source['editor'];o=stacks['objects'][source['source_object']]
    bones=links['objects'][o['source_index']]['prefix']['bones'];parents=[b['word_34'] for b in bones];parents[0]=-1
    local=o['runs'][source['run']]['ticks'][source['tick']]['local'];move=0 if bones[0]['name']['name'].lower()=='move' else -1
    target=min(move+3,len(local)-1);assert c['target']==target;supplied=source['matrices'][target]
    if iteration==0:
        disabled=[0]*28;disabled[0]=target;disabled[19]=1;selected=disabled.copy();selected[0]|=0x80000000;selected[2:18]=supplied
        selected[18]=0x100 if scenario==2 else 0x101 if scenario==3 else 0x10101
        selected[19]=[1,3,2,0][scenario];selected[20]=bits(.3 if scenario%2 else -1.);selected[21]=bits(0. if scenario in (1,2) else -1.);selected[26]=bits(.25);selected[27]=0xaabbcc00
        skipped=selected.copy();skipped[19]=0xffffffff;ds=[disabled,selected,skipped]
    else:ds=[d.copy() for d in last[(si,scenario)]];ds[1][26]=bits(.125)
    assert c['before']==[{'words':d} for d in ds]
    output=[];calls=0
    for bone,p in enumerate(local):
        m=matrix(p['rotation'],p['position'])
        if bone>0 and parents[bone]!=move:m=compose(m,output[parents[bone]])
        output.append(m)
        for d in ds:
            index=d[0]&0x7fffffff;index=index-0x80000000 if index&0x40000000 else index
            if index==bone and d[18]&0xffffff:apply(d,bone,output);calls+=1;break
    assert c['result']=={'Ok':1} and calls==1
    assert c['matrices']==output,key
    assert c['after']==[{'words':d} for d in ds],key
    last[(si,scenario)]=[d.copy() for d in ds];matrix_count+=len(output);stats['poses']+=1
assert matrix_count==239296 and stats['poses']==7200
stats['matrices']=matrix_count
sources=['crates/rc-package/src/skeletal_director_apply.rs','crates/rc-package/src/skeletal_director_apply_tests.rs','crates/rc-package/src/skeletal_director_products.rs','crates/rc-inspect/src/bin/rc-directed-rotation-pose-check.rs','scripts/Generate-DirectorProducts.py','scripts/Record-DirectedRotationPoses.py','scripts/Record-DirectorRotations.py','analysis/decompiled/skeletal-director.asm','analysis/decompiled/skeletal-director-plane-product.asm','analysis/decompiled/skeletal-director-plane-product.c']
result={'date':'2026-10-07','rust_tests':318,'scope':report['scope'],'counts':dict(stats),'product_probes':64,'max_product_error_f64':max_product_error,'source_sha256':{s:sha(root/s) for s in sources},'report_sha256':sha(path),'prior_report_sha256':sha(prior_path),'stack_report_sha256':sha(stack_path),'checks':['cargo test --workspace: 318 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','7200 poses / 239296 matrices and mutable director snapshots independently recomputed bit-exact under portable math policy','64 general products independently checked with numeric original ASM interpreter and f64 mathematical product','six tests cover selected rows, column scaling, final translation/scale, relative composition, reverse previous-index correction and partial host failures']}
(root/'analysis/reports/directed-rotation-poses-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';e=json.loads(path.read_text(encoding='utf-8-sig'));e['rust_tests']=318;e['directed_rotation_poses_validation']=result;path.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: Director-Rotationsanwendung mit relativer Komposition und Korrektur frueherer Knochenmatrizen verbunden. '
      'Mutierbare Director-History, Actor-Spaltenskalierung, W-Nullskalierung, rueckwaerts verteilte Quaternion-Potenzen, Translation/Plane-Skalierung zuletzt. '
      '7200 Diagnoseposen / 239296 Matrizen und Director-Snapshots unabhaengig geprueft, einschliesslich zweier Folgeaufrufe pro Konfiguration. '
      '318 Tests, Clippy und Format bestanden. Runtime-/Director-Datenbindung, Pose-Vorbereitung, Bounds-Integration und Skinning offen; Android zum Schluss. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_DIRECTOR_APPLICATION.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Director-Rotationsanwendung mit relativer Komposition und Korrektur frueherer Knochenmatrizen' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(dict(stats)));print('max general product error',max_product_error)
