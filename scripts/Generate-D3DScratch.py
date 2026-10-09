"""Derive synthetic scratch integration fixtures from the checked resize fixtures."""
from pathlib import Path
import json,hashlib,copy
root=Path(__file__).resolve().parents[1]
prior=root/'analysis/reports/d3d-resize.input.json';old=json.loads(prior.read_text());sha=hashlib.sha256(prior.read_bytes()).hexdigest()
assert sha==json.loads((root/'analysis/reports/d3d-resize-validation.json').read_text())['input_sha256']
cases=[]
for i,base in enumerate(old['cases']):
 c=copy.deepcopy(base);w=c['world'];rt=w.pop('runtime');rt['device']['scratch']=0 if i%8==0 else 1+i%3
 size=len(w['target'])
 # Physical captured extent exceeds logical count in boundary probes.
 count=[0,size+8,size+4,size+5][i%4]
 flags=[0,0x20000000,0x80000000,0xe0000000][i%4]
 scratch=dict(address=0 if count==0 else 0x41000000,packed_count=flags|count,bytes=[(j*13+i)&255 for j in range(size+16)] if count else [])
 if i%12==4:scratch.update(address=0x41000000,packed_count=flags|0x1fffffff,bytes=[])
 if i%12==2:
  raw=c['steps'][0]['source']['size_before'];first=min(raw,0x100000000-raw)
  scratch.update(packed_count=flags|(first+4),bytes=[(j*13+i)&255 for j in range(first+5)])
 w['runtime']=dict(buffers=rt,scratch=scratch)
 for j,s in enumerate(c['steps']):
  s['array']=dict(reset_pointer=0x41010000,growth_pointer=0x42000000+j*0x100000)
  # Vary the caller lock tail; scratch must copy size bytes, not just payload length.
 cases.append(c)
errors=[]
def error(index,mutate):
 c=copy.deepcopy(cases[index]);c['id']=len(errors);c['steps']=c['steps'][:1];c['world']['runtime']['buffers']['device']['scratch']=1;mutate(c);errors.append(c)
error(1,lambda c:c['world']['target'].clear())
error(1,lambda c:c['steps'][0]['answers']['lock'].update(unlock_hresult=0x80004005))
error(1,lambda c:c['steps'][0]['binding']['shader'].update(address=0))
error(1,lambda c:c['steps'][0]['source'].update(stride=0))
error(0,lambda c:c['steps'][0]['array'].update(growth_pointer=0))
error(0,lambda c:c['steps'][0]['array'].update(growth_pointer=0xfffffff0))
def boundary(c):
 size=abs(c['steps'][0]['source']['size_before']);c['world']['runtime']['scratch'].update(address=0x41000000,packed_count=size+4,bytes=[1]*(size+4))
error(1,boundary)
error(1,lambda c:c['steps'][0]['answers']['lock'].update(lock_hresult=0x80004005))
error(1,lambda c:c['steps'][0]['answers']['lock'].update(lock_pointer=0))
error(1,lambda c:c['world']['runtime']['buffers']['vertex'].update(active=2))
def too_large(c):c['steps'][0]['source'].update(size_before=16*1024*1024+1)
error(1,too_large)
def index_error(c):
 c['steps']=[copy.deepcopy(cases[1]['steps'][1])];c['steps'][0]['source']['width']=0
error(1,index_error)
output=dict(source_sha256=sha,cases=cases,errors=errors)
(root/'analysis/reports/d3d-scratch.input.json').write_text(json.dumps(output),newline='\n')
print(json.dumps(dict(cases=len(cases),steps=sum(len(c['steps']) for c in cases),errors=len(errors))))
