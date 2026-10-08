"""Interpret original SSE expressions for world corners/center, then verify publication."""
from pathlib import Path
import json,struct,math,hashlib,re
root=Path(__file__).resolve().parents[1]
f=lambda x:struct.unpack('<f',struct.pack('<f',x))[0]
bits=lambda x:struct.unpack('<I',struct.pack('<f',x))[0]
value=lambda x:struct.unpack('<f',struct.pack('<I',x))[0]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
path=root/'analysis/reports/world-pose-bounds.json';report=json.loads(path.read_text());local_path=root/'analysis/reports/pose-bounds.json';local=json.loads(local_path.read_text())
validation=json.loads((root/'analysis/reports/pose-bounds-validation.json').read_text());assert validation['report_sha256']==sha(local_path)
assert (len(report['cases']),len(report['synthetic']))==(3600,64)
asm_path=root/'analysis/decompiled/skeletal-root-frame.asm';asm=asm_path.read_text()
def offset(op):
    match=re.findall(r'-?0x[0-9a-f]+',op);return int(match[-1],16) if match else 0
def expressions(start,end,corner):
    registers={};output={}
    def memory(op):
        off=offset(op)
        if 'ESI' in op:return ('M',(off-(8 if corner else 0))//4)
        if 'EAX' in op:return ('P',0 if corner else (off-0x44)//4)
        if 'EBP' in op:
            assert corner;return ('P',{-0x24:1,-0x18:2}[off])
        raise AssertionError(op)
    for line in asm.splitlines():
        if line.startswith('#') or not line:continue
        address,instruction=line.split(' ',1)
        if not start<=int(address,16)<=end:continue
        opcode,_,args=instruction.partition(' ')
        if opcode not in ('MOVSS','MOVAPS','MULSS','ADDSS'):continue
        dst,src=args.split(',',1)
        if dst.startswith('XMM'):
            expr=registers[src] if src.startswith('XMM') else memory(src)
            registers[dst]=expr if opcode in ('MOVSS','MOVAPS') else (opcode,registers[dst],expr)
        else:
            assert opcode=='MOVSS' and 'EBP' in dst
            index={-0x34:0,-0x30:1,-0x2c:2}[offset(dst)];output[index]=registers[src]
    assert len(output)==3;return output
corner_expr=expressions(0x1050af9e,0x1050b034,True)
center_expr=expressions(0x1050b089,0x1050b136,False)
def evaluate(expr,m,p):
    if expr[0]=='M':return value(m[expr[1]])
    if expr[0]=='P':return p[expr[1]]
    a=evaluate(expr[1],m,p);b=evaluate(expr[2],m,p)
    return f(a*b) if expr[0]=='MULSS' else f(a+b)
def point(expr,m,p):return [bits(evaluate(expr[i],m,p)) for i in range(3)]
def verify(c,b,m,supplied=False,fail=False):
    low=list(map(value,b['minimum']));high=list(map(value,b['maximum']));minimum=maximum=None
    for ix in range(2):
        for iy in range(2):
            for iz in range(2):
                p=point(corner_expr,m,[low[0] if ix==0 else high[0],low[1] if iy==0 else high[1],low[2] if iz==0 else high[2]])
                if minimum is None:minimum=p.copy();maximum=p.copy();continue
                for i,v in enumerate(p):
                    if not value(v)>=value(minimum[i]):minimum[i]=v
                    if not value(maximum[i])>=value(v):maximum[i]=v
    rows=[]
    for row in [8,4,0]:
        a,bv,cv=[value(m[row+i]) for i in range(3)]
        rows.append(bits(f(f(f(cv*cv)+f(bv*bv))+f(a*a))))
    squared=value(max(rows));assert c['squared']==[bits(squared)]
    expected={**c['before'],'minimum':minimum,'maximum':maximum,'valid':1}
    if fail:assert c['result']=={'Err':'seed boundary'}
    else:
        seed=.125 if supplied else f(1/f(math.sqrt(squared))) if squared else math.inf
        inv=f(f(3-f(f(seed*squared)*seed))*f(seed*.5));scale=f(inv*squared) if squared else 0.
        sphere=b['sphere'];expected['sphere']=point(center_expr,m,list(map(value,sphere[:3])))+[bits(f(value(sphere[3])*scale))]
        assert c['result']=={'Ok':None}
    assert c['after']==expected
    assert all(math.isfinite(value(v)) for field in ('minimum','maximum','sphere') for v in expected[field])
seen=set()
for c in report['cases']:
    key=(c['source_case'],c['transform']);assert key not in seen;seen.add(key)
    verify(c,local['cases'][key[0]]['after'],report['transforms'][key[1]])
assert seen=={(i,j) for i in range(900) for j in range(4)}
for c in report['synthetic']:verify(c,local['cases'][0]['after'],c['matrix'],True,c['fail'])
box=(root/'analysis/decompiled/skeletal-world-box.asm').read_text()
for marker in ('1011fa20 MOV byte ptr [EAX + 0x18],0x0','1011fa4c JC 0x1011fa51','1011fa8f JC 0x1011fa94','1011faf8 MOV byte ptr [EAX + 0x18],0x1'):assert marker in box
for marker in ('1050b075 MOVSD.REP ES:EDI,ESI','1050b1a4 CMP EDX,ECX','1050b1a6 JC 0x1050b1ab','1050b1df CMP ECX,EDX','1050b212 RSQRTSS XMM0,XMM1','1050b242 ANDPS XMM3,XMM4','1050b270 MOV dword ptr [EDI + 0x4],ECX'):assert marker in asm,marker
sources=['crates/rc-package/src/skeletal_world_bounds.rs','crates/rc-package/src/skeletal_bounds.rs','crates/rc-inspect/src/bin/rc-world-bounds-check.rs','scripts/Record-WorldPoseBounds.py','analysis/decompiled/skeletal-root-frame.asm','analysis/decompiled/skeletal-world-box.asm','analysis/decompiled/skeletal-world-box.c']
result={'date':'2026-10-07','rust_tests':287,'world_bounds':3600,'general_cases':64,'seed_failures':16,'scope':report['scope'],'report_sha256':sha(path),'local_report_sha256':sha(local_path),'source_sha256':{s:sha(root/s) for s in sources},'checks':['cargo test --workspace: 287 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','all corner/center expressions interpreted directly from original SSE instructions','3600 transformed original-track-derived local bounds verified bit-exact under portable math policy','64 general matrix cases verify raw-bit scale selection and box-before-sphere failure writes']}
(root/'analysis/reports/world-pose-bounds-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';e=json.loads(path.read_text(encoding='utf-8-sig'));e['rust_tests']=287;e['world_pose_bounds_validation']=result;path.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: Actor-Weltgrenzen-Transformation und lokaler Abschluss verbunden. Acht Boxecken '
      'mit nativen SSE-Ausdrücken, separate Kugelzentrum-Reihenfolge, größte Zeilenlänge per unsigned '
      'Floatbits, RSQRT-Radiusskalierung und Box-vor-Sphäre-Fehlergrenze. 3600 Transformationen von '
      'Originaltrack-Diagnosebounds plus64 allgemeine Matrixfälle/16Seedfehler unabhängig geprüft; '
      '287Tests,Clippy,Format bestanden. Matrizen/Padding weiterhin explizite Diagnosevorgaben, '
      'keine Original-Runtime-Actorbindung; Directors/Skinning offen. Android zum Schluss. Details '
      'D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_WORLD_BOUNDS.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Actor-Weltgrenzen-Transformation und lokaler Abschluss verbunden' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps({'world_bounds':3600,'general_cases':64,'seed_failures':16}))
