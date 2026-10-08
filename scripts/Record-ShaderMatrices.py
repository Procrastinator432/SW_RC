"""Independent numeric replay of native instructions, SSE words and bounded bank uploads."""
from pathlib import Path
import struct,json,re,math,hashlib,collections
root=Path(__file__).resolve().parents[1]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
bits=lambda x:struct.unpack('<I',struct.pack('<f',x))[0]
value=lambda x:struct.unpack('<f',struct.pack('<I',x))[0]
f=lambda x:value(bits(x))
path=root/'analysis/reports/shader-matrices.json';report=json.loads(path.read_text())
data=(root/'analysis/reports/shader-matrices.input.bin').read_bytes()
sse=(root/'analysis/reports/shader-matrices.sse.bin').read_bytes()
assert len(data)==len(sse)==1024*192
release=root/'analysis/reports/shader-matrices-release.json'
release_input=root/'analysis/reports/shader-matrices-release.input.bin'
assert release.read_bytes()==path.read_bytes(),'Optimized and debug reports differ'
assert release_input.read_bytes()==data,'Optimized and debug inputs differ'
asm_path=root/'analysis/decompiled/shader-constants.asm'
code=[]
for line in asm_path.read_text().splitlines():
    if line and not line.startswith('#'):
        address,instruction=line.split(' ',1);opcode,_,args=instruction.partition(' ')
        code.append((int(address,16),opcode,args))
# Numeric interpreter reads actual instruction order, not generated Rust order tables.
def run(start,end,state,stack):
    registers={};eax=0
    def operand(op):
        if op.startswith('XMM'):return registers[op]
        m=re.fullmatch(r'dword ptr \[(EAX|EBP)(?: \+ (0x[0-9a-f]+))?\]',op);assert m,op
        off=int(m[2] or '0',16);off=off-2**32 if off>=2**31 else off
        return (state if m[1]=='EAX' else stack)[off+(eax if m[1]=='EAX' else 0)]
    for address,opcode,args in code:
        if not start<=address<=end:continue
        if opcode=='ADD' and args=='EAX,0xac':eax+=0xac;continue
        if opcode not in ('MOVSS','MOVAPS','MULSS','ADDSS'):
            assert opcode in ('LEA','MOV'),(hex(address),opcode,args)
            continue
        dst,src=args.split(',',1);v=operand(src)
        if dst.startswith('XMM'):
            if opcode=='MULSS':v=bits(f(value(registers[dst])*value(v)))
            elif opcode=='ADDSS':v=bits(f(value(registers[dst])+value(v)))
            registers[dst]=v
        else:
            m=re.fullmatch(r'dword ptr \[EBP \+ (0x[0-9a-f]+)\]',dst);assert m
            off=int(m[1],16)-2**32;stack[off]=v
    return stack
def matrix_from(stack,base):return [[stack[base+(r*4+c)*4] for c in range(4)] for r in range(4)]
transpose=lambda m:[list(row) for row in zip(*m)]
def original(object,view,projection):
    state={base+(r*4+c)*4:m[r][c] for base,m in [(0x2c,object),(0x6c,view),(0xac,projection)] for r in range(4) for c in range(4)}
    world=matrix_from(run(0x1001c2bc,0x1001c7b8,state,{}),-0x380)
    first=run(0x1001c8fa,0x1001cd3b,state,{})
    stack={-0x500+i*4:first[-0x300+i*4] for i in range(16)}
    # The native ADD EAX,ac occurs just before the second stage.
    screen=matrix_from(run(0x1001cd56,0x1001d0f2,{i:state[0xac+i] for i in range(0,64,4)},stack),-0x480)
    camera=matrix_from(run(0x1001d721,0x1001db5d,state,{}),-0x5c0)
    return list(map(transpose,[world,screen,camera]))
def fixture(index,matrix):
    if index==0:return [[bits(1.) if r==c else 0 for c in range(4)] for r in range(4)]
    if index==1:
        return [[bits(1.)]*4 for _ in range(4)] if matrix==0 else [[bits(v)]*4 for v in [16777216.,1.,-16777216.,1.]] if matrix==1 else fixture(0,0)
    result=[]
    for r in range(4):
        row=[]
        for c in range(4):
            word=(index*0x9e3779b9+(matrix+1)*0x1020304+(r*4+c)*0x40507)&0xffffffff
            word^=word>>16;word=word*0x85ebca6b&0xffffffff;word^=word>>13
            mode=(index+r*4+c+matrix)%41
            row.append(word&0x80000000 if mode==0 else (word&0x807fffff)|1 if mode==1 else (word&0x807fffff)|((90+(word>>24)%46)<<23))
        result.append(row)
    return result
counts=collections.Counter()
for i,p in enumerate(report['probes']):
    matrices=[fixture(i,m) for m in range(3)]
    assert p['index']==i and matrices==[p['object'],p['view'],p['projection']],i
    assert data[i*192:(i+1)*192]==b''.join(struct.pack('<4I',*row) for m in matrices for row in m)
    expected=original(*matrices);assert p['outputs']==expected,(i,'interpreter')
    words=b''.join(struct.pack('<4I',*row) for m in expected for row in m)
    assert sse[i*192:(i+1)*192]==words,(i,'SSE')
    for j,d in enumerate(p['dispatches']):
        kind=[2,3,32][j];slot=i%5;size=8 if i%2==0 else 96
        before={'words':[[0x7fc12345,0x80000000,0xdeadbeef,i] for _ in range(size)],'count':slot+1 if i%7==0 else -3 if size==8 else -2}
        assert d['kind']==kind and d['slot']==slot and d['before']==before
        after={'words':[row[:] for row in before['words']],'count':slot+1 if i%7==0 else slot+2}
        # Last binding is the unsupported continuation at slot+1 (span one).
        # The matrix still writes four rows, extending beyond this iteration count.
        after['words'][slot:slot+4]=expected[j]
        assert d['after']==after and d['error'] is None,(i,kind,'bank')
        counts['bank_dispatches']+=1;counts['bank_words_checked']+=size*4*2
    counts['matrix_inputs']+=3;counts['matrix_outputs']+=3;counts['sse_words_checked']+=48
    counts['probes']+=1
for p in report['errors']:
    kind,mode,slot=p['kind'],p['mode'],p['slot'];assert kind in (2,3,32) and mode in range(4)
    assert slot==(6 if mode==3 else 1)
    before={'words':[[9]*4 for _ in range(8)],'count':7 if mode==3 else -3}
    assert p['before']==before
    after={'words':[row[:] for row in before['words']],'count':7 if mode==3 else 5}
    after['words'][0]=[0x80000000,0x7fc11111,7,8]
    assert p['after']==after
    message=['Nonfinite matrix composition input excluded by host contract','Nonfinite matrix composition product excluded by host contract','Nonfinite matrix composition sum excluded by host contract','Matrix upload exceeds safe buffer'][mode]
    assert p['error']==message,(kind,mode,p['error'])
    counts['expected_errors']+=1
assert counts['probes']==1024 and counts['expected_errors']==12
# Independently confirm native dispatcher targets and transpose/copy landing paths.
s=root/'scripts/Record-ShaderScalars.py';n={'__file__':str(s)}
exec(compile(s.read_text().split('pn,data,meta,e=')[0],str(s),'exec'),n)
memory=n['memory']
for kind,target in [(2,0x1001c2b3),(3,0x1001c8f1),(32,0x1001d718)]:
    entry=memory(0x1001ed44+kind-1,1)[0]
    assert struct.unpack('<I',memory(0x1001ecf0+entry*4,4))[0]==target
for address in [0x1001c8ec,0x1001d215]:
    assert memory(address,1)==b'\xe9'
    assert address+5+struct.unpack('<i',memory(address+1,4))[0]==0x1001dc91
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/shader-matrices-tests.log').read_text())));assert tests==560,tests
sources=['crates/rc-package/src/shader_constants.rs','crates/rc-package/src/shader_matrix_orders.rs','crates/rc-inspect/src/bin/rc-shader-matrix-check.rs','scripts/Generate-ShaderMatrixOrders.py','scripts/Probe-ShaderMatrices.rs','scripts/Record-ShaderMatrices.py','analysis/decompiled/shader-constants.asm','analysis/decompiled/shader-constants.c']
validation={'counts':dict(counts),'rust_tests':tests,'report_sha256':sha(path),'release_report_sha256':sha(release),'debug_release_identical':True,'input_sha256':sha(root/'analysis/reports/shader-matrices.input.bin'),'sse_sha256':sha(root/'analysis/reports/shader-matrices.sse.bin'),'original_d3ddrv_sha256':sha(n['dll']),'source_sha256':{s:sha(root/s) for s in sources},'scope':report['scope']}
(root/'analysis/reports/shader-matrices-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
e=root/'analysis/evidence.json';evidence=json.loads(e.read_text());evidence['rust_tests']=tests;evidence['shader_matrices_validation']=validation
e.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({'counts':dict(counts),'rust_tests':tests}))
