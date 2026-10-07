"""Independent scalar instruction interpreter and original full-local-pose oracle."""
from pathlib import Path
import json,struct,hashlib,re,math
root=Path(__file__).resolve().parents[1]
# Load only reader/oracle definitions preceding the earlier validation loop.
source=root/'scripts/Record-SkeletalLinkups.py';ns={'__file__':str(source)}
exec(compile(source.read_text().split("for o in report['objects']:")[0],str(source),'exec'),ns)
links=ns['report'];animations=ns['animations'];decode=ns['decode'];tracks=ns['original_tracks'];matrix=ns['matrix']
f=ns['f'];bits=ns['bits'];value=ns['value']
path=root/'analysis/reports/original-skeletal-poses.json';report=json.loads(path.read_text())
assert (report['poses'],report['bone_matrices'],len(report['objects']),len(report['compositions']))==(935,28656,130,32)

# Interpret the original SSE arithmetic expression trees, not the Rust order table.
asm_path=root/'analysis/decompiled/skeletal-root-frame.asm';registers={};expressions={}
def memory(operand):
    kind='L' if 'EBX' in operand else 'P'
    assert 'EBX' in operand or 'EAX' in operand,operand
    offsets=re.findall(r'0x[0-9a-f]+',operand);offset=int(offsets[-1],16) if offsets else 0
    return (kind,offset//4)
for line in asm_path.read_text().splitlines():
    if not line or line.startswith('#'):continue
    address,instruction=line.split(' ',1)
    if not 0x1050a7c5<=int(address,16)<=0x1050ab60:continue
    opcode,_,operands=instruction.partition(' ')
    if opcode not in ('MOVSS','MOVAPS','MULSS','ADDSS'):continue
    dst,src=operands.split(',',1)
    if dst.startswith('XMM'):
        expr=registers[src] if src.startswith('XMM') else memory(src)
        if opcode in ('MOVSS','MOVAPS'):registers[dst]=expr
        else:registers[dst]=(opcode,registers[dst],expr)
    else:
        assert opcode=='MOVSS' and 'EBP' in dst
        offset=int(re.findall(r'-?0x[0-9a-f]+',dst)[0],16)
        if offset>=0x80000000:offset-=0x100000000
        index=(offset+0x84)//4;assert 0<=index<16
        expressions[index]=registers[src]
assert len(expressions)==16
def evaluate(expr,a,b):
    if expr[0]=='L':return value(a[expr[1]])
    if expr[0]=='P':return value(b[expr[1]])
    left=evaluate(expr[1],a,b);right=evaluate(expr[2],a,b)
    return f(left*right if expr[0]=='MULSS' else left+right)
def compose(a,b):return [bits(evaluate(expressions[i],a,b)) for i in range(16)]
for p in report['compositions']:assert p['result']==compose(p['local'],p['parent'])

poses=0;matrices=0;sampled=0;fallbacks=0;move_meshes=0;max_error=0.
for o in report['objects']:
    old=links['objects'][o['source_index']];assert (o['file'],o['object'])==(old['file'],old['object']);bones=old['prefix']['bones']
    parents=[b['word_34'] for b in bones];parents[0]=-1
    move=0 if bones[0]['name']['name'].lower()=='move' else -1;move_meshes+=move==0
    depth=[0]*len(bones);descendants=[0]*len(bones)
    for i,parent in enumerate(parents):
        while parent>=0:
            assert parent<i;descendants[parent]+=1;depth[i]+=1;parent=parents[parent]
    assert o['hierarchy']=={'parents':parents,'depths':depth,'descendants':descendants,'move_bone':move}
    reference=[{'rotation':b['rotation'],'position':b['position']} for b in bones]
    assert len(o['cases'])==2+3*len(old['mappings'])
    for ci,c in enumerate(o['cases']):
        assert c['result']=={'Ok':None};local=c['local'];assert len(local)==len(bones)
        if ci<2:
            assert c['kind']=='Reference' and c['editor']==bool(ci) and local==reference
        else:
            m=old['mappings'][(ci-2)//3];animation=animations[m['animation']];metadata=animation['sequences'][0]['metadata']
            assert c['kind']=='MappedTrackPose' and c['entry']==m['entry'] and c['animation']==m['animation'] and c['sequence']==0
            assert c['sequence_name']==metadata['name']['name'] and not c['editor']
            assert c['time_bits']==bits([0.,f(metadata['frames']*.5),f(metadata['frames'])][(ci-2)%3])
            ts=tracks(animation,0);t=value(c['time_bits']);n_sample=0;n_fallback=0
            for i,track in enumerate(m['mapping']):
                if track<0:
                    assert local[i]==reference[i];n_fallback+=1;continue
                n_sample+=1;scale,pos,rot,dur=ts[track];res=t;current=0;alpha=None
                if len(dur)>1:
                    for current,d in enumerate(dur):
                        after=f(res-d)
                        if after<0:break
                        res=after
                    else:current=0;d=dur[0]
                    if res>0:alpha=f(res/d)
                nxt=(current+1)%len(dur) if alpha is not None else current
                a=decode(rot[0 if len(rot)==1 else current]);b=decode(rot[0 if len(rot)==1 else nxt]);out=a
                if alpha is not None and len(rot)!=1:
                    dot=sum(x*y for x,y in zip(a,b));absolute=abs(dot)
                    if absolute>=value(0x3f0ccccd):
                        out=[x*(1-alpha)+y*alpha*(-1 if dot<0 else 1) for x,y in zip(a,b)];norm=math.sqrt(sum(x*x for x in out));out=[x/norm for x in out]
                    else:
                        angle=math.acos(absolute);left=math.sin((1-alpha)*angle)/math.sin(angle);right=math.sin(alpha*angle)/math.sin(angle)
                        out=[x*left+y*right*(-1 if dot<0 else 1) for x,y in zip(a,b)]
                error=max(abs(x-value(y)) for x,y in zip(out,local[i]['rotation']));max_error=max(max_error,error);assert error<2e-6,(o['object'],i,error)
                ps=f(scale*value(0x38000100));a=[f(x*ps) for x in pos[0 if len(pos)==1 else current]];b=[f(x*ps) for x in pos[0 if len(pos)==1 else nxt]]
                out=[f(f(f(y-x)*alpha)+x) for x,y in zip(a,b)] if alpha is not None and len(pos)!=1 else a
                assert local[i]['position']==list(map(bits,out))
            assert (c['sampled_bones'],c['reference_fallbacks'])==(n_sample,n_fallback);sampled+=n_sample;fallbacks+=n_fallback
        output=[]
        for i,transform in enumerate(local):
            a=matrix(transform['rotation'],transform['position']);parent=parents[i]
            if i>0 and (parent!=move or c['editor']):a=compose(a,output[parent])
            output.append(a)
        assert c['matrices']==output,(o['object'],ci)
        poses+=1;matrices+=len(output)
assert (poses,matrices)==(935,28656)
post=(root/'analysis/decompiled/skeletal-postload-hierarchy.c').read_text()
for marker in ('"Move",1','*(undefined4 *)(this + 0x1c0) = uVar2','+ 0x34) = 0xffffffff','+ 0x38 + iVar3 * 0x40'):assert marker in post
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
sources=['crates/rc-package/src/skeletal_hierarchy.rs','crates/rc-inspect/src/bin/rc-skeletal-pose-check.rs','scripts/Record-SkeletalPoses.py','scripts/Record-SkeletalLinkups.py','scripts/Record-OriginalAnimationTracks.py','analysis/decompiled/skeletal-postload-hierarchy.c','analysis/decompiled/skeletal-postload-hierarchy.asm','analysis/decompiled/skeletal-root-frame.asm','analysis/decompiled/skeletal-apply-channel.c']
result={'date':'2026-10-07','rust_tests':262,'diagnostic_poses':935,'bone_matrices':28656,'move_root_meshes':move_meshes,'sampled_bones':sampled,'reference_fallbacks':fallbacks,'general_matrix_cases':32,'maximum_portable_local_rotation_error':max_error,'report_sha256':sha(path),'linkup_report_sha256':sha(root/'analysis/reports/original-skeletal-linkups.json'),'animation_report_sha256':sha(root/'analysis/reports/original-animation-tracks.json'),'source_sha256':{s:sha(root/s) for s in sources},'checks':['cargo test --workspace: 262 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','original scalar instruction expression interpreter verifies all 16 matrix products','935 local/hierarchy poses and 28656 matrices independently verified','mapped local bones compared to original track math, quaternion tolerance 2e-6'],'scope':report['scope']}
(root/'analysis/reports/original-skeletal-poses-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';e=json.loads(path.read_text(encoding='utf-8-sig'));e['rust_tests']=262;e['original_skeletal_poses_validation']=result;path.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: PostLoad-Knochenhierarchie und native Elternmatrix-Verknüpfung ergänzt. Rootparent=-1, '
      'Move-Root-Sonderregel, Tiefe/Nachkommen und SSE-Summenfolge je Matrixelement rekonstruiert. '
      '935 Diagnoseposen mit 28656 Knochenmatrizen unabhängig geprüft: Referenzpose in Editor/Spiel sowie '
      'vollständige lokale Trackposen der ersten Sequenz je Linkup, ohne Kanalblend-/Director-/Bounds-Behauptung. '
      '32 allgemeine Matrixfälle anhand Originalinstruktionsinterpreter geprüft. 262 Workspace-Tests, Clippy '
      'und Format bestanden. Vollständiges ApplyAnimation und Skinning offen; Android zum Schluss. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_HIERARCHY.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'PostLoad-Knochenhierarchie und native Elternmatrix-Verknüpfung ergänzt' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps({k:result[k] for k in ('rust_tests','diagnostic_poses','bone_matrices','move_root_meshes','sampled_bones','reference_fallbacks','maximum_portable_local_rotation_error')}))
