"""Whole original AL PlaySource with synthetic buffers and recorded AL calls.
No real audio device: validity/cache/category/listener, AL API/error checks and
base virtual Play/Pause are controlled boundaries. Original EAX logarithm,
RSQRTSS refinement, buffer branching and source lifetime instructions execute.
"""
import argparse,importlib.util,json,math,random,struct
from pathlib import Path
spec=importlib.util.spec_from_file_location('activation_original',Path(__file__).with_name('Test-AudioActivationOriginal.py'))
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
c=m.core;bits=m.bits
class Harness(m.Harness):
    def __init__(self,path):
        super().__init__(path);self.upload=[];self.seed=None;self.ready=None
        self.api={self.stub+0x3000+i*0x20:name for i,name in enumerate(['Float','Integer','Vector','Queue','Eax','Category'])}
        for at,name in self.api.items():
            if name=='Category':self.u.mem_write(at,b'\xd9\x05'+struct.pack('<I',self.result+0x3000)+b'\xc2\x04\x00')
            else:self.u.mem_write(at,b'\xc3')
        for iat,name in [(0x1001224c,'Float'),(0x1001220c,'Integer'),(0x100121ec,'Vector'),(0x10012198,'Queue')]:
            self.put(iat,'I',next(a for a,n in self.api.items() if n==name))
        self.put(0x1000a1e0,'I',self.result+0x3010);self.put(0x1000a014,'I',self.result+0x3020)
        self.put(self.result+0x3020,'III',0,0,0)
    def hook(self,u,at,size,data):
        super().hook(u,at,size,data)
        if self.mode!='play':return
        sp=u.reg_read(c.UC_X86_REG_ESP);context=u.reg_read(c.UC_X86_REG_ECX)
        if at==0x10008100:self.trace.append('Precache');self.ret(u,self.current_request['buffer'])
        elif at==self.reply:self.trace.append('IsQueued')
        elif at==0x103590a0:self.trace.append(dict(Copy=self.node(context)))
        elif at==0x10358f10:self.trace.append(dict(Stop=self.node(context)))
        elif at==0x10358eb0 and self.get(sp)==0x100088e8:self.trace.append(dict(Clear=self.node(context)))
        elif at==0x100088fb:
            bp=u.reg_read(c.UC_X86_REG_EBP);self.ready=dict(node=self.node(u.reg_read(c.UC_X86_REG_EDI)),pause=self.get(bp-0x1c,'i'))
            self.trace.append(dict(Ready=self.ready))
        elif at in [self.stub+0x1000+i*0x200+0x10 for i in range(3)]:self.upload.append(dict(Params=dict(sound=context)))
        elif at==0x10004df0:
            stages={0x1000bb88:'Gain',0x1000bb6c:'Pitch',0x1000bb54:'Looping',0x1000bb38:'Rolloff',0x1000bb0c:'Relative',0x1000bafc:'End'}
            self.upload.append(dict(Check=stages[context]));self.ret(u)
        elif at==self.stub+0x100:self.upload.append(dict(Listener=dict(index=self.get(sp+4,'i'))))
        elif at==0x10008b50:self.seed=u.reg_read(c.UC_X86_REG_XMM0)&0xffffffff
        elif at==0x10358ef0:self.upload.append(dict(Play=dict(node=self.node(context),lifetime_words=self.words(context+0x18,2))))
        elif at==0x10358f00:self.upload.append(dict(Pause=dict(node=self.node(context),request=self.get(sp+4,'i'),lifetime_words=self.words(context+0x18,2))))
        elif at in self.api:
            name=self.api[at];a=[self.get(sp+4+i*4) for i in range(5 if name=='Eax' else 3)]
            if name=='Category':self.upload.append(dict(Category=dict(index=a[0])))
            elif name in ['Float','Integer']:self.upload.append({name:dict(hardware=a[0],parameter=a[1],value=a[2])})
            elif name=='Vector':self.upload.append(dict(Vector=dict(hardware=a[0],parameter=a[1],value=self.words(a[2],3))))
            elif name=='Queue':self.upload.append(dict(Queue=dict(hardware=a[0],buffers=self.words(a[2],a[1]))))
            else:
                assert a[4]==4
                self.upload.append(dict(Eax=dict(version=3 if a[0]==0x1000a678 else 2,property=a[1],hardware=a[2],value=self.get(a[3],'i'))))

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--game-data',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--verify-fixture',type=Path);a=p.parse_args()
    h=Harness(a.game_data);r=random.Random(0x55504c35);cases=[]
    for i in range(512):
        h.setup_pool(r,1+i%5);src=h.node_at[-1];buffer=0x5070000;arr=buffer+0x100
        flags=[0,2,8,16,0x100,0x800,0x1000,0x1002,0x1800][i%9]
        h.put(src+0x20,'II',flags,0);h.put(src+0x30,'f',4.)
        h.put(src+0x18,'d',r.choice([0.,-1.,3.5,1e9]));h.put(src+0x38,'i',r.choice([-7,0,1,5]))
        h.put(src+0x8c,'f',r.choice([0.,.6999999,.7,1.,1.2,1.200001,4.,-2.]))
        h.put(src+0x94,'f',r.choice([-1.,0.,.0001,.5,1.,3.]))
        h.put(src+0x5c,'fff',*[r.uniform(-1e5,1e5) for _ in range(3)])
        h.put(h.listener+0x1c,'fff',*[r.uniform(-1e5,1e5) for _ in range(3)])
        if i%11==0:h.u.mem_write(h.listener+0x1c,bytes(h.u.mem_read(src+0x5c,12)))
        h.put(src+0x74,'f',r.choice([0.,1.,40.,1024.]));h.put(src+0x7c,'f',r.choice([0.,.00001,1.,40.,1000.]))
        if i%13==0:h.put(src+0x74,'f',1.);h.put(src+0x7c,'f',1.)
        if i%3==0:
            h.put(src+0x74,'f',1.)
            h.put(src+0x7c,'f',10**r.uniform(-6,5))
        # No NaN EAX ratio in this bounded contract; zero/zero only with EAX off.
        eax=i%4!=0
        if eax and h.get(src+0x74,'f')==0.:h.put(src+0x74,'f',1.)
        editor=i%7==0;version=r.choice([1,2,3,4]);category=r.randrange(3)
        h.put(0x500d000+((len(h.node_at)-1)%3)*0x100+0x28,'B',category)
        gain=bits(r.choice([-2.,0.,.25,1.,2.]));h.put(h.result+0x3000,'I',gain)
        h.put(h.result+0x3010,'I',int(editor));h.put(h.audio+0x284,'i',version)
        h.put(h.vtables+0x1000+0x170,'I',next(at for at,n in h.api.items() if n=='Category'))
        h.put(h.vtables+8,'I',0x10358ef0);h.put(h.vtables+0xc,'I',0x10358f00)
        h.put(0x10012274,'I',next(at for at,n in h.api.items() if n=='Eax') if eax else 0)
        cache_valid=i%3==0;cache_value=-1234 if cache_valid else 99
        h.put(0x10012398,'I',int(cache_valid));h.put(0x10012394,'i',cache_value)
        buffers=[100+j for j in range(i%5)]
        h.put(buffer+0x10,'IIII',arr,len(buffers)|((i%8)<<29),123,456)
        for j,b in enumerate(buffers):h.put(arr+j*4,'I',b)
        request=dict(engine_present=i%41!=0,sound_valid=i%37!=0,buffer=0 if i%31==0 else buffer,queued=r.choice([0,1,-3]))
        if i%43==0:h.put(src+0xc,'I',0)
        if i%47==0:
            for at in h.node_at[:-2]:h.put(at+8,'I',1);h.put(at+0x24,'I',0);h.put(at+0x30,'f',5.)
        before=h.pool_state();incoming=h.snapshot(src)
        env=dict(category=category,category_gain_bits=gain,editor=editor,eax_enabled=eax,eax_version=version,listener_bits=h.words(h.listener+0x1c,3),rsqrt_seed_bits=None)
        initial_buffer=dict(handle=buffer,buffers=buffers,counters=[123,456]);cache=dict(initialized=cache_valid,value=cache_value)
        h.current_request=request;h.upload=[];h.trace=[];h.ready=None;h.seed=None;h.mode='play'
        h.put(h.audio+0x2c,'I',h.engine if request['engine_present'] else 0);h.driver_replies[src]=request['queued']
        h.call(0x10008800,h.audio,[src]);h.mode=None
        env['rsqrt_seed_bits']=h.seed
        cases.append(dict(before=before,incoming=incoming,request=request,environment=env,buffer=initial_buffer,cache=cache,prepared=h.ready,prefix=h.trace[:],events=h.upload[:],expected=h.pool_state(),buffer_after=dict(handle=buffer,buffers=buffers,counters=h.words(buffer+0x18,2)),cache_after=dict(initialized=bool(h.get(0x10012398)&1),value=h.get(0x10012394,'i'))))
    # Fresh translation cache: the selected stop lies inside a previously run
    # whole-function block in Unicorn. Do not reuse that translation.
    h=Harness(a.game_data)
    attenuations=[]
    for i in range(2048):
        # Floats adjacent to integer attenuation boundaries stress truncation.
        k=r.randrange(-9999,10001)
        word=bits(math.exp(k/868.5889892578125))+[-1,0,1][i%3]
        h.put(h.source+0x74,'f',1.);h.put(h.source+0x7c,'I',word)
        h.u.reg_write(c.UC_X86_REG_EDI,h.source);h.u.reg_write(c.UC_X86_REG_EBP,h.stack+0x800)
        h.u.reg_write(c.UC_X86_REG_ESP,h.stack);h.u.reg_write(c.UC_X86_REG_FPCW,0x37f)
        h.u.reg_write(c.UC_X86_REG_FPSW,0);h.u.reg_write(c.UC_X86_REG_FPTAG,0xffff);h.u.reg_write(c.UC_X86_REG_MXCSR,0x1f80)
        try:h.u.emu_start(0x10008a33,0x10008a68,count=1000)
        except Exception as e:raise RuntimeError(f'attenuation {i} EIP={h.u.reg_read(c.UC_X86_REG_EIP):x}') from e
        assert h.u.reg_read(c.UC_X86_REG_EIP)==0x10008a68
        result=h.u.reg_read(c.UC_X86_REG_EAX);result=result if result<0x80000000 else result-0x100000000
        attenuations.append(dict(ratio_bits=word,value=result))
    data=dict(schema=1,binary_sha256=dict(h.hashes,alaudio=h.al_hash),cases=cases,attenuations=attenuations)
    if a.verify_fixture:assert json.loads(a.verify_fixture.read_text())==data,'Upload fixture mismatch'
    a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(data,separators=(',',':'))+'\n')
    report=dict(passed=True,whole_play_source_cases=len(cases),attenuation_boundary_cases=len(attenuations),successful_uploads=sum(x['prepared'] is not None for x in cases),fixture_verified=bool(a.verify_fixture))
    a.output.with_suffix('.validation.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
if __name__=='__main__':main()
