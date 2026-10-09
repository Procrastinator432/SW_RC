"""Original Engine source getters/copies and D3DDrv lifecycle, CPU mirror proof."""
from pathlib import Path
import json,re,collections,copy
root=Path(__file__).resolve().parents[1];s=root/'scripts/Record-D3DUpload.py';u={'__file__':str(s)};src=s.read_text()
exec(compile(src.split('\nfixtures=json.loads')[0],str(s),'exec'),u)
exec(compile(src[src.index('def put('):src.index('\nfor kind,key in')],str(s),'exec'),u)
g=u['g'];sha=u['sha'];FRAME=u['FRAME'];put=u['put'];get=u['get'];counts=collections.Counter();u['counts']=counts
assemblies=['d3d-index-source.asm','d3d-index-width.asm','d3d-skin-source.asm','d3d-skin-components.asm','d3d-skin-stride.asm']
for name in assemblies:
 asm=(root/'analysis/decompiled'/name).read_text();g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
fixtures=json.loads((root/'analysis/reports/d3d-source.input.json').read_text());path=root/'analysis/reports/d3d-source.json';report=json.loads(path.read_text());assert path.read_bytes()==(root/'analysis/reports/d3d-source-release.json').read_bytes()
assert fixtures['source_sha256']==report['source_sha256']==sha(root/'analysis/reports/d3d-upload.input.json')==json.loads((root/'analysis/reports/d3d-upload-validation.json').read_text())['input_sha256']
DEST=0x40000000;COMP=0x41000000;OUT=0x42000000
def initialize(c):
 vm=g['Native']();vm.r.update(ECX=c['address'],ESP=0x70000000,EBP=FRAME)
 for r in c['regions']:
  for k,b in enumerate(r['bytes']):vm.m[r['address']+k]=b
 put(vm,vm.r['ESP']+4,DEST);put(vm,FRAME+8,DEST);put(vm,FRAME-12,0)
 for k in range(272):vm.m[DEST+k]=0xcd
 return vm
def raw_index(vm,c):return dict(address=c['address'],data=get(vm,c['address']+0x10),count_word=get(vm,c['address']+0x14),width=c['width'])
def raw_skin(vm,c):return dict(address=c['address'],owner=get(vm,c['address']+0x10),mode=get(vm,c['address']+0x18),data=get(vm,c['address']+0x1c),count_word=get(vm,c['address']+0x20))
def index_size(vm,c):
 vm.r['ECX']=c['address'];vm.run(0x104c7530 if c['width']=='U16' else 0x104c7570,0x104c753b if c['width']=='U16' else 0x104c757c);size=vm.r['EAX']
 vm.run(0x104c6d80 if c['width']=='U16' else 0x104c6d90,0x104c6d85 if c['width']=='U16' else 0x104c6d95);return size,vm.r['EAX']
def index_payload(vm,c,size):
 if size>16*1024*1024:return None
 vm.r.update(ECX=c['address'],ESP=0x70000000);put(vm,vm.r['ESP']+4,DEST)
 vm.run(0x104c7540 if c['width']=='U16' else 0x104c7580,0x104c7564 if c['width']=='U16' else 0x104c75a5)
 return [vm.m[DEST+k] for k in range(size)]
def skin_size(vm,c,s):
 if s['mode'] and not s['owner']:return None
 vm.r['ECX']=c['address']
 if s['mode']:
  put(vm,s['owner'],0x24000000);vm.run(0x104ffce0,0x104ffcec);assert vm.r['ECX']==s['owner'];vm.r['EAX']=c['owner_count'];vm.run(0x104ffcf2,0x104ffcf5)
 else:vm.run(0x104ffce0,0x104ffd02)
 return vm.r['EAX']
def skin_payload(vm,c,s):
 vm.r.update(ECX=c['address'],ESP=0x70000000,EBP=FRAME);vm.run(0x104ffc0c,0x104ffc1f)
 if s['mode']:
  vm.run(0x104ffc21,0x104ffc26)
  if s['owner']:
   put(vm,s['owner'],0x24000000);vm.run(0x104ffc28,0x104ffc32);assert vm.r['ECX']==s['owner'] and vm.pop()==DEST;vm.steps+=1
   for k,b in enumerate(c['delegate']):vm.m[DEST+k]=b
   counts['owner_fill_fixtures']+=1;return c['delegate']
 vm.run(0x104ffc7d,0x104ffc94);size=vm.r['EDX']
 if size>16*1024*1024:return None
 vm.run(0x104ffc94,0x104ffc9d);return [vm.m[DEST+k] for k in range(size)]
for c,q in zip(fixtures['indices'],report['indices'],strict=True):
 vm=initialize(c);capture=raw_index(vm,c);size,width=index_size(vm,c);payload=index_payload(vm,c,size)
 assert q['id']==c['id'] and q['capture']==capture and q['size']==size and q['width']==width
 if payload is None:assert 'Err' in q['payload'];counts['bounded_index_rejections']+=1
 else:assert q['payload']==dict(Ok=payload);counts['index_payload_bytes']+=len(payload)
 counts['index_cases']+=1;counts['source_instructions']+=vm.steps
for c,q in zip(fixtures['skins'],report['skins'],strict=True):
 vm=initialize(c);capture=raw_skin(vm,c);size=skin_size(vm,c,capture);payload=skin_payload(vm,c,capture)
 vm.r.update(ECX=c['address'],ESP=0x70000000);put(vm,vm.r['ESP']+4,OUT);put(vm,vm.r['ESP']+8,c['vertex']);vm.run(0x104ffcb0,0x104ffcc1 if capture['mode'] else 0x104ffcd6);pointer=get(vm,OUT)
 for k,b in enumerate(c['initial']):vm.m[COMP+k]=b
 put(vm,vm.r['ESP']+4,COMP);vm.run(0x103b0800,0x103b081f);components=[get(vm,COMP+k*4) for k in range(4)]+[vm.r['EAX']];vm.run(0x104fde10,0x104fde15)
 assert q['id']==c['id'] and q['capture']==capture and q['pointer']==pointer and q['components']==components and q['stride']==vm.r['EAX']
 if size is None:assert 'Err' in q['size'];counts['owner_size_rejections']+=1
 else:assert q['size']==dict(Ok=size)
 if payload is None:assert 'Err' in q['payload'];counts['bounded_skin_rejections']+=1
 else:assert q['payload']==dict(Ok=payload);counts['skin_payload_bytes']+=len(payload)
 counts['skin_cases']+=1;counts['source_instructions']+=vm.steps
for c,q in zip(fixtures['errors'],report['errors'],strict=True):assert c['id']==q['id'] and 'Err' in q['result'];counts['capture_errors']+=1
for c,q in zip(fixtures['transfers'],report['transfers'],strict=True):
 vm=initialize(c);kind=c['kind']
 if kind=='vertex':
  capture=raw_skin(vm,c);size=skin_size(vm,c,capture);source=dict(address=c['address'],size=size,dynamic=c['dynamic'],special=c['special'],revision_after=c['revision_after']);plan,resource=u['vertex'](c['resource'],source,c['device'],c['responses']);payload=skin_payload(vm,c,capture)
 else:
  size,width=index_size(vm,c);source=dict(address=c['address'],size=size,width=width,revision_before=c['revision_before'],revision_after=c['revision_after']);plan,resource=u['index'](c['resource'],source,c['device'],c['responses']);payload=index_payload(vm,c,size) if plan['uploaded'] else []
 assert payload is not None
 destination=c['destination'].copy();destination[:len(payload)]=payload
 assert q==dict(id=c['id'],result=dict(plan=plan,copied=len(payload)),resource=resource,destination=destination),('transfer',c['id'])
 counts['transfers']+=1;counts['transfer_bytes']+=len(payload);counts['transfer_source_instructions']+=vm.steps;counts['transfer_upload_skips']+=int(not plan['uploaded'])
assert counts['index_cases']==144 and counts['skin_cases']==256 and counts['capture_errors']==6 and counts['transfers']==64
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/d3d-source-tests.log').read_text())));assert tests==689,tests
sources=['crates/rc-package/src/d3d_source.rs','crates/rc-package/src/d3d_upload.rs','crates/rc-package/src/shader_snapshot.rs','crates/rc-inspect/src/bin/rc-d3d-source-check.rs','scripts/Generate-D3DSource.py','scripts/Record-D3DSource.py','scripts/Record-D3DUpload.py','scripts/Record-D3DState.py','scripts/Record-MaterialState.py','analysis/decompiled/d3d-source-callbacks.c']+['analysis/decompiled/'+p for p in assemblies]
validation=dict(date='2026-10-09',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_engine_sha256=sha(Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData/System/Engine.dll')),original_d3ddrv_sha256=sha(g['dll']),input_sha256=sha(root/'analysis/reports/d3d-source.input.json'),report_sha256=sha(path),source_sha256={p:sha(root/p) for p in sources},scope=report['scope'],android='deferred until end per user')
(root/'analysis/reports/d3d-source-validation.json').write_text(json.dumps(validation,indent=2)+'\n');ledger=root/'analysis/evidence.json';e=json.loads(ledger.read_text(encoding='utf-8'));e['d3d_source_validation']=validation;e['rust_tests']=tests;ledger.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8');print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
