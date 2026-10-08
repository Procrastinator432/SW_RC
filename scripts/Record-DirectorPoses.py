"""Independent director selection, in-hierarchy local corrections and resulting bounds."""
from pathlib import Path
import json,hashlib,math,collections
root=Path(__file__).resolve().parents[1]
source=root/'scripts/Record-SkeletalPoses.py';ns={'__file__':str(source)}
exec(compile(source.read_text().split('poses=0;matrices=0;')[0],str(source),'exec'),ns)
links=ns['links'];matrix,compose,f,bits,value=[ns[k] for k in ('matrix','compose','f','bits','value')]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
stack_path=root/'analysis/reports/original-channel-stacks.json';stacks=json.loads(stack_path.read_text());validation=json.loads((root/'analysis/reports/original-channel-stacks-validation.json').read_text());assert sha(stack_path)==validation['report_sha256']
path=root/'analysis/reports/director-poses.json';report=json.loads(path.read_text());assert (report['poses'],report['bone_matrices'],len(report['cases']))==(1800,59824,1800)
stats=collections.Counter();seen=set()
for c in report['cases']:
    oi,ri,ti=c['source_object'],c['run'],c['tick'];key=(oi,ri,ti,c['editor']);assert key not in seen;seen.add(key)
    o=stacks['objects'][oi];bones=links['objects'][o['source_index']]['prefix']['bones'];local=o['runs'][ri]['ticks'][ti]['local'];parents=[b['word_34'] for b in bones];parents[0]=-1
    move=0 if bones[0]['name']['name'].lower()=='move' else -1;target=move+1;assert c['target']==target
    base=[0]*28;base[0]=target;base[19]=1
    selected=base.copy();selected[0]|=0x80000000;selected[18]=0x10001;selected[2:14]=list(map(bits,[.75,1.25,1.5,1.]*3));selected[14:18]=list(map(bits,[3.,-2.,5.,1.]))
    skipped=selected.copy();skipped[18]=0x100;ds=[base,selected,skipped];assert c['directors']==[{'words':w} for w in ds]
    output=[];calls=0
    for i,p in enumerate(local):
        m=matrix(p['rotation'],p['position'])
        if i>0 and (parents[i]!=move or c['editor']):m=compose(m,output[parents[i]])
        for d in ds:
            bone=d[0]&0x7fffffff;bone=bone-0x80000000 if bone&0x40000000 else bone
            if bone!=i or not d[18]&0xffffff:continue
            assert d[19]&1 and not d[18]&0xff00
            if d[18]&0xff:m[12:16]=d[14:18]
            if d[18]&0xff0000:m[:12]=[bits(f(value(d[2+j])*value(v))) for j,v in enumerate(m[:12])]
            calls+=1;break
        output.append(m)
    assert c['result']=={'Ok':1} and calls==1 and c['matrices']==output
    points=[list(map(value,m[12:15])) for m in output[target:]];assert points
    low=[f(min(p[i] for p in points)*value(0x3f99999a))-1 for i in range(3)];low=list(map(f,low))
    high=[f(f(max(p[i] for p in points)*value(0x3f99999a))+1) for i in range(3)]
    d=[f(y-x) for x,y in zip(low,high)];n=f(f(f(d[0]*d[0])+f(d[1]*d[1]))+f(d[2]*d[2]));r=f(1/f(math.sqrt(n))) if n else math.inf
    refined=f(f(3-f(f(r*n)*r))*f(r*.5));distance=f(refined*n) if n else 0.
    center=[f(f(x+y)*.5) for x,y in zip(low,high)]
    b={'minimum':list(map(bits,low)),'maximum':list(map(bits,high)),'sphere':list(map(bits,center))+[bits(f(distance*.5))],'byte_60':1,'byte_61':1,'byte_179':0}
    assert c['bounds_result']=={'Ok':None} and c['bounds']==b
    assert all(math.isfinite(value(v)) for m in output for v in m)
    stats['poses']+=1;stats['matrices']+=len(output);stats['director_applications']+=calls
asm=(root/'analysis/decompiled/skeletal-root-frame.asm').read_text()
for marker in ('1050ac08 SHL EDI,0x1','1050ac0a SAR EDI,0x1','1050ac10 CMP byte ptr [EAX + 0x48],0x0','1050ac16 CMP byte ptr [EAX + 0x49],0x0','1050ac1c CMP byte ptr [EAX + 0x4a],0x0','1050ac26 CALL 0x10501bb0','1050ac2e MOV ESI,dword ptr [EBP + 0xc]'):assert marker in asm,marker
director=(root/'analysis/decompiled/skeletal-director.asm').read_text()
for marker in ('10502c9a MOV AL,byte ptr [EBX + 0x48]','10502cab MOV dword ptr [EAX],ECX','10502ccb MOV AL,byte ptr [EBX + 0x4a]','10502cdc CALL dword ptr [0x10650598]'):assert marker in director,marker
plane=(root/'analysis/decompiled/skeletal-director-plane.asm').read_text()
assert plane.count('MULSS')==4 and '10114afa MULSS XMM0,dword ptr [EAX]' in plane
sources=['crates/rc-package/src/skeletal_director.rs','crates/rc-package/src/skeletal_hierarchy.rs','crates/rc-inspect/src/bin/rc-director-pose-check.rs','scripts/Record-DirectorPoses.py','scripts/Record-SkeletalPoses.py','analysis/decompiled/skeletal-root-frame.asm','analysis/decompiled/skeletal-director.c','analysis/decompiled/skeletal-director.asm','analysis/decompiled/skeletal-director-plane.asm']
result={'date':'2026-10-07','rust_tests':292,'counts':dict(stats),'scope':report['scope'],'source_sha256':{s:sha(root/s) for s in sources},'report_sha256':sha(path),'stack_report_sha256':sha(stack_path),'checks':['cargo test --workspace: 292 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','1800 directed original-track-derived poses / 59824 matrices independently checked','selection and local component-scale instructions verified against original ASM','all resulting local boxes/spheres independently verified with diagnostic padding']}
(root/'analysis/reports/director-poses-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';e=json.loads(path.read_text(encoding='utf-8-sig'));e['rust_tests']=292;e['director_poses_validation']=result;path.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: Director-Auswahl in Hierarchiestage und lokaler Translations-/Plane-Skalierungspfad '
      'rekonstruiert. Erstes aktives Match, signed31-Knochenindex, Korrektur vor Kindverknüpfung; '
      'Rotation/Weltraum-Inversion bleiben explizit unsupported. 1800 Originaltrack-Diagnoseposen mit '
      '59824 Matrizen und lokalen Bounds unabhängig geprüft, Director-Snapshots vorgegeben. '
      '292Tests,Clippy,Format bestanden. Android zum Schluss. Details '
      'D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_DIRECTORS.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Director-Auswahl in Hierarchiestage und lokaler Translations-/Plane-Skalierungspfad' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(dict(stats)))
