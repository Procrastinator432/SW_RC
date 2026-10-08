"""Independent integrated local/world bounds and publication-before-flags oracle."""
from pathlib import Path
import json,struct,math,hashlib,collections
root=Path(__file__).resolve().parents[1]
script=root/'scripts/Record-WorldPoseBounds.py';ns={'__file__':str(script)}
exec(compile(script.read_text().split('seen=set()')[0],str(script),'exec'),ns)
f,bits,value,verify_world=[ns[k] for k in ('f','bits','value','verify')]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
path=root/'analysis/reports/directed-pose-bounds.json';report=json.loads(path.read_text())
prior_path=root/'analysis/reports/directed-rotation-poses.json';prior=json.loads(prior_path.read_text())
assert sha(prior_path)==json.loads((root/'analysis/reports/directed-rotation-poses-validation.json').read_text())['report_sha256']
directors=json.loads((root/'analysis/reports/director-poses.json').read_text());stacks=json.loads((root/'analysis/reports/original-channel-stacks.json').read_text());links=json.loads((root/'analysis/reports/original-skeletal-linkups.json').read_text())
assert report['poses']==len(report['cases'])==7200 and report['bone_matrices']==239296
assert report['padding']=={'minimum':list(map(bits,[.25,1.,2.])),'maximum':list(map(bits,[2.,.5,1.])),'k_one':[bits(1.)]*3}
low_pad=list(map(value,report['padding']['minimum']));high_pad=list(map(value,report['padding']['maximum']))
seen=set();counts=collections.Counter()
for c in report['cases']:
    index=c['source_case'];assert index not in seen;seen.add(index)
    p=prior['cases'][index];s=directors['cases'][p['source_case']];o=stacks['objects'][s['source_object']];bones=links['objects'][o['source_index']]['prefix']['bones']
    move=0 if bones[0]['name']['name'].lower()=='move' else -1;first=move+1;matrices=p['matrices'];h=0xcbf29ce484222325
    for m in matrices:
        for word in m:
            for byte in struct.pack('<I',word):h=((h^byte)*0x100000001b3)&0xffffffffffffffff
    assert c['matrix_fnv1a64']==f'{h:016x}' and c['matrices_and_directors_match_prior'] is True and c['result']=={'Ok':1}
    low=[99]*3;high=[100]*3
    for i,m in enumerate(matrices):
        point=m[12:15]
        if i==first:low=point.copy();high=point.copy()
        elif i>first:
            for a,v in enumerate(point):
                if value(low[a])>value(v):low[a]=v
                elif value(v)>value(high[a]):high[a]=v
    low=[f(value(v)*value(0x3f99999a)) for v in low];high=[f(value(v)*value(0x3f99999a)) for v in high]
    low=[f(v-f(1.+pad)) for v,pad in zip(low,low_pad)];high=[f(f(pad+1.)+v) for v,pad in zip(high,high_pad)]
    d=[f(y-x) for x,y in zip(low,high)];n=f(f(f(d[0]*d[0])+f(d[1]*d[1]))+f(d[2]*d[2]));r=f(1/f(math.sqrt(n))) if n else math.inf
    length=f(f(f(3.-f(f(r*n)*r))*f(r*.5))*n) if n else 0.
    center=[f(f(y+x)*.5) for x,y in zip(low,high)]
    published={'minimum':list(map(bits,low)),'maximum':list(map(bits,high)),'sphere':list(map(bits,center))+[bits(f(length*.5))],'byte_60':7,'byte_61':8,'byte_179':9}
    assert c['published']==published and c['local']=={**published,'byte_60':1,'byte_61':1,'byte_179':0}
    before={'minimum':[77]*3,'maximum':[88]*3,'sphere':[456]*4,'valid':0};scenario=p['scenario']
    if scenario==0:assert c['world']==before;counts['without_actor']+=1
    else:
        m=report['world_transforms'][scenario-1];fv=list(map(value,m));norms=[]
        for row in [8,4,0]:norms.append(bits(f(f(f(fv[row+2]*fv[row+2])+f(fv[row+1]*fv[row+1]))+f(fv[row]*fv[row]))))
        verify_world({'before':before,'after':c['world'],'squared':[max(norms)],'result':{'Ok':None}},published,m)
        counts['world_publications']+=1
    counts['poses']+=1;counts['matrices']+=len(matrices)
assert dict(counts)=={'without_actor':1800,'poses':7200,'matrices':239296,'world_publications':5400}
sources=['crates/rc-package/src/skeletal_directed_bounds.rs','crates/rc-package/src/skeletal_directed_bounds_tests.rs','crates/rc-package/src/skeletal_bounds.rs','crates/rc-inspect/src/bin/rc-directed-pose-bounds-check.rs','scripts/Record-DirectedPoseBounds.py','scripts/Record-WorldPoseBounds.py','analysis/decompiled/skeletal-root-frame.asm']
result={'date':'2026-10-07','rust_tests':324,'scope':report['scope'],'counts':dict(counts),'source_sha256':{s:sha(root/s) for s in sources},'report_sha256':sha(path),'prior_report_sha256':sha(prior_path),'checks':['cargo test --workspace: 324 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','7200 integrated local boxes/spheres and publication-before-flags snapshots independently checked bit-exact under portable math policy','5400 world bounds independently checked using original SSE expressions; 1800 absent-actor outputs unchanged','all integrated matrices and mutated directors compared directly to prior report by diagnostic CLI, all matrix fingerprints independently checked','six integration tests verify director/parent/local seed/world seed errors and Move-only completion']}
(root/'analysis/reports/directed-pose-bounds-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';e=json.loads(path.read_text(encoding='utf-8-sig'));e['rust_tests']=324;e['directed_pose_bounds_validation']=result;path.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: Director-Hierarchie mit knochenweiser Bounds-Sammlung, lokaler Sphaere, optionaler Weltpublikation und Abschlussflags verbunden. '
      '7200 integrierte Diagnoseposen / 239296 Matrizen; 5400 Weltpublikationen und 1800 Ohne-Actor-Faelle unabhaengig geprueft. '
      'Sechs neue Integrationstests pruefen Teilzustaende und Fehlergrenzen; 324 Tests, Clippy und Format bestanden. '
      'Vorbereitete Originaltrack-Posen und vorgegebene Director-/Szenen-/Padding-Snapshots, keine volle Runtime-/Skinning-Integration. Android zum Schluss. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_DIRECTED_BOUNDS.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Director-Hierarchie mit knochenweiser Bounds-Sammlung' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(dict(counts)))
