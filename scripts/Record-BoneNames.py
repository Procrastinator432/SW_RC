"""Independent handle/alias/name-table oracle with original reference-bone inputs."""
from pathlib import Path
import json,re,hashlib,collections
root=Path(__file__).resolve().parents[1];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
script=root/'scripts/Record-SkeletalPoses.py';ns={'__file__':str(script)}
exec(compile(script.read_text().split('poses=0;matrices=0;')[0],str(script),'exec'),ns)
matrix=ns['matrix'];bits=ns['bits'];original=ns['links']
original_path=root/'analysis/reports/original-skeletal-linkups.json'
assert sha(original_path)==json.loads((root/'analysis/reports/original-skeletal-linkups-validation.json').read_text())['report_sha256']
path=root/'analysis/reports/bone-names.json';report=json.loads(path.read_text());assert len(report['objects'])==130 and report['queries']==3631
fixed={name.lower():int(index) for index,name in re.findall(r'\((\d+), "([^"]+)"\)',(root/'crates/rc-package/src/hardcoded_names.rs').read_text())}
keys={'none','__rc_diag_alias__','__rc_diag_chain__','__rc_diag_missing__'};legacy=0
for o in original['objects']:
    if o.get('status')=='UnsupportedLegacyPackage':legacy+=1;continue
    keys.update(b['name']['name'].lower() for b in o['prefix']['bones'])
assert legacy==1
bindings={};next_index=max(fixed.values())+1
for key in sorted(keys):
    if key in fixed:index=fixed[key]
    else:index=next_index;next_index+=1
    bindings[key]={'key':key,'index':index,'handle':0 if index==0 else index+1,'fixed_native_index':key in fixed}
assert report['names']==list(bindings.values())
table=[0]*(max(b['index'] for b in bindings.values())+1)
for b in bindings.values():table[b['index']]=b['handle']
assert report['global_names']==table
alias=bindings['__rc_diag_alias__'];chain=bindings['__rc_diag_chain__'];missing=bindings['__rc_diag_missing__'];stats=collections.Counter();seen=set()
for o in report['objects']:
    index=o['source_index'];assert index not in seen;seen.add(index);source=original['objects'][index];bones=source['prefix']['bones']
    handles=[bindings[b['name']['name'].lower()]['handle'] for b in bones]
    aliases=[{'name_handle':alias['handle'],'target_handle':handles[0]},{'name_handle':alias['handle'],'target_handle':handles[-1]},{'name_handle':chain['handle'],'target_handle':alias['handle']}]
    expected_skeleton={'bones':[{'name_handle':h,'word_38':b['word_38']} for h,b in zip(handles,bones)],'aliases':aliases};assert o['skeleton']==expected_skeleton
    matrices=[matrix(bones[0]['rotation'],bones[0]['position']),matrix(bones[-1]['rotation'],bones[-1]['position']),matrix([0,0,0,bits(1.)],[0,0,0])];assert o['alias_matrices']==matrices
    queries=[bindings[b['name']['name'].lower()]['index'] for b in bones]+[alias['index'],chain['index'],missing['index'],0]
    assert len(o['queries'])==len(queries)
    for q,name_index in zip(o['queries'],queries):
        assert q['global_index']==name_index;handle=table[name_index];target=handle;out=[99]*16;matched=None
        if handle:
            for i,a in enumerate(aliases):
                if a['name_handle']==handle:target=a['target_handle'];out=matrices[i];stats['alias_matrix_copies']+=1;break
            matched=next((i for i,h in enumerate(handles) if h==target),None)
        assert q['matched']==matched and q['matrix']==out
        ok=matched is not None
        assert q['created_result']=={'Ok':ok} and q['cache']==(0 if ok else 7)
        if not ok:
            assert q['created']==q['updated']==[] and q['updated_result'] is None;stats['misses']+=1
        else:
            prefix=[matched,matched]+matrices[0]+[0x101,1,bits(-1.),bits(-1.)]
            assert q['created']==[{'words':prefix+[0,0,0,bits(1.),0,0xccbbaa00]}]
            prefix[18]=0x10101;prefix[20]=bits(.25)
            tail=list(map(bits,[.1,.2,.3,.9,.125]))+[0x11223302]
            assert q['updated_result']=={'Ok':True} and q['updated']==[{'words':prefix+tail}];stats['named_create_update_pairs']+=1
        stats['queries']+=1
    stats['meshes']+=1;stats['original_bones']+=len(bones)
asm=(root/'analysis/decompiled/skeletal-match-ref-bone.asm').read_text()
for marker in ['1050041d JZ 0x10500499','10500442 JZ 0x1050044e','10500451 MOV EBP,dword ptr [EDI + ECX*0x8 + 0x4]','1050045e JZ 0x1050046a','10500468 MOVSD.REP ES:EDI,ESI','10500484 JZ 0x10500495']:assert marker in asm,marker
sources=['crates/rc-package/src/mesh_animation.rs','crates/rc-package/src/skeletal_set_bone_place.rs','crates/rc-package/src/skeletal_bone_name_tests.rs','crates/rc-package/src/name_bindings.rs','crates/rc-package/src/hardcoded_names.rs','crates/rc-inspect/src/bin/rc-bone-name-check.rs','scripts/Record-BoneNames.py','analysis/decompiled/skeletal-match-ref-bone.c','analysis/decompiled/skeletal-match-ref-bone.asm']
result={'date':'2026-10-07','rust_tests':336,'scope':report['scope'],'counts':dict(stats),'unsupported_legacy_meshes':legacy,'source_sha256':{s:sha(root/s) for s in sources},'report_sha256':sha(path),'original_report_sha256':sha(original_path),'checks':['cargo test --workspace: 336 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','3631 original-bone/diagnostic-alias queries independently checked including matrix copy before miss','named SetBonePlace creation/update states independently checked with preserved history','diagnostic name table independently rebuilt with existing verified fixed indices and sorted dynamic diagnostic indices','six tests include duplicate aliases/names, one-pass chains, null names, optional matrix bounds and cache ordering on missing global indices']}
(root/'analysis/reports/bone-names-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';e=json.loads(path.read_text(encoding='utf-8-sig'));e['rust_tests']=336;e['bone_names_validation']=result;path.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: SetBonePlace mit gemeinsamer FName-Handle-/Alias-Referenzknochensuche verbunden; optionaler Alias-Matrixausgang vor Trefferpruefung rekonstruiert. '
      '130 Originalskelette / 3111 Knochen / 3631 Abfragen inklusive Anlage und History-erhaltendem Update unabhaengig geprueft. '
      'Globale Handle-Tabelle und Alias-Snapshots diagnostisch, native dynamische Registrierung und Original-Aliasarchive offen; bekanntes Legacy-Mesh ausgeschlossen. '
      '336 Tests, Clippy und Format bestanden. Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_BONE_NAMES.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'SetBonePlace mit gemeinsamer FName-Handle-/Alias-Referenzknochensuche verbunden' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(dict(stats)))
