"""Original base copy, AL transient/channel selection, bounded PlaySource prefix,
voice-cue refresh and late Update census. No AL playback/parameter-upload claim.
Driver validity/cache/playing/update/Params replies are controlled boundaries.
"""
import argparse,hashlib,importlib.util,json,random,struct
from pathlib import Path
import pefile
spec=importlib.util.spec_from_file_location('commands_original',Path(__file__).with_name('Test-AudioCommandsOriginal.py'))
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
core=module.core;bits,f32=module.bits,module.f32

class Harness(module.Harness):
    def __init__(self,path):
        super().__init__(path)
        raw=(path/'System/alaudio.dll').read_bytes();self.al_hash=hashlib.sha256(raw).hexdigest()
        assert self.al_hash=='60e0c8495a94b297c36bc96f29e4c64a208f020e1ac55f5d6848d5697cff0dcd', 'Unsupported AL binary'
        al=pefile.PE(data=raw);self.u.mem_map(al.OPTIONAL_HEADER.ImageBase,(al.OPTIONAL_HEADER.SizeOfImage+4095)&~4095)
        self.u.mem_write(al.OPTIONAL_HEADER.ImageBase,al.get_memory_mapped_image())
        exports={s.name:self.engine_pe.OPTIONAL_HEADER.ImageBase+s.address for s in self.engine_pe.DIRECTORY_ENTRY_EXPORT.symbols}
        for lib in al.DIRECTORY_ENTRY_IMPORT:
            for imp in lib.imports:
                if imp.name in exports:self.put(imp.address,'I',exports[imp.name])
        self.mode=None;self.trace=[];self.node_at=[];self.replies={};self.call_counts={};self.mutations={};self.after={}
        self.validstub2,self.errorstub,self.finishedstub,self.playingstub,self.updatestub=[self.stub+n for n in [0x2000,0x2020,0x2040,0x2060,0x2080]]
        for address in [self.validstub2,self.errorstub,self.finishedstub,self.playingstub]:self.u.mem_write(address,b'\xc3')
        self.u.mem_write(self.updatestub,b'\xc2\x04\x00')
        self.put(0x10012174,'I',self.errorstub)
        self.current_request={}

    def node(self,at):return self.node_at.index(at)
    def ret(self,u,value=0,pop=0):
        sp=u.reg_read(core.UC_X86_REG_ESP);u.reg_write(core.UC_X86_REG_EAX,value&0xffffffff)
        u.reg_write(core.UC_X86_REG_EIP,self.get(sp));u.reg_write(core.UC_X86_REG_ESP,sp+4+pop)
    def hook(self,u,at,size,data):
        super().hook(u,at,size,data)
        if not self.mode:return
        context=u.reg_read(core.UC_X86_REG_ECX);sp=u.reg_read(core.UC_X86_REG_ESP)
        if at==self.validstub2:
            self.trace.append('SoundValid');u.reg_write(core.UC_X86_REG_EAX,int(self.current_request['sound_valid']))
        elif at==0x10008100 and self.mode=='prepare':
            self.trace.append('Precache');self.ret(u,self.current_request['buffer'])
        elif at==self.errorstub:self.trace.append('ClearError')
        elif at==self.reply and self.mode=='prepare':
            self.trace.append('IsQueued');u.reg_write(core.UC_X86_REG_EAX,self.current_request['queued']&0xffffffff)
        elif at in [0x103590a0,0x10358f70] and self.mode=='prepare':self.trace.append(dict(Copy=self.node(context)))
        elif at==0x10358f10 and self.mode in ['prepare','swap']:self.trace.append(dict(Stop=self.node(context)))
        elif at==0x10358eb0 and self.mode=='prepare' and self.get(sp)==0x100088e8:self.trace.append(dict(Clear=self.node(context)))
        elif at==0x100088fb and self.mode=='prepare':
            node=self.node(u.reg_read(core.UC_X86_REG_EDI));bp=u.reg_read(core.UC_X86_REG_EBP)
            pause=self.get(bp-0x1c,'i');self.prepared=dict(node=node,pause=pause)
            self.trace.append(dict(Ready=self.prepared));self.put(0,'I',self.get(bp-0xc))
            u.reg_write(core.UC_X86_REG_EIP,self.stub)
        elif at==self.finishedstub:
            node=self.node(context);u.reg_write(core.UC_X86_REG_EAX,self.finished[node]&0xffffffff)
        elif at==self.playingstub:
            node=self.node(context);index=self.slot_nodes().index(node);self.active_index=index
            self.trace.append(dict(IsPlaying=index));count=self.call_counts.get(node,0);self.call_counts[node]=count+1
            values=self.replies[node];u.reg_write(core.UC_X86_REG_EAX,values[min(count,len(values)-1)]&0xffffffff)
        elif at==self.updatestub and self.mode=='census':
            index=self.active_index;node=self.node(context)
            self.trace.append(dict(Update=dict(index=index,seconds_bits=self.get(sp+4))))
            change=self.mutations[node]
            self.put(context+0x34,'I',change['cursor']);self.put(context+0x3c,'I',int(change['voice_exempt']))
            self.put(context+0xc,'I',change['sound']);self.put(context+0x30,'I',change['priority_bits'])
            self.after[node]=self.snapshot(context)
        elif self.mode=='census':
            getters=[self.stub+0x1000+i*0x200+0x10 for i in range(3)]
            if at in getters:self.trace.append(dict(Params=dict(index=self.active_index,sound=context)))

    def slot_nodes(self):
        arr=self.get(self.audio+0x2a4);n=self.get(self.audio+0x2a8)&0x1fffffff
        return [None if not self.get(arr+i*4) else self.node(self.get(arr+i*4)) for i in range(n)]
    def pool_state(self):
        transient=self.get(self.audio+0x2ac)
        return dict(nodes=[dict(source=self.snapshot(at),hardware=self.get(at+0xac),buffer=self.get(at+0xb0)) for at in self.node_at],
                    slots=self.slot_nodes(),transient=None if not transient else self.node(transient))
    def setup_pool(self,rng,count):
        self.record=False;self.mode=None;self.trace=[];self.call_counts={};self.after={};self.replies={};self.finished={}
        self.bank();self.node_at=[0x5040000+i*0x100 for i in range(count+2)]
        for i,at in enumerate(self.node_at):
            self.construct(i,at);self.put(at,'I',self.vtables)
            self.put(at+8,'I',int(rng.choice([True,True,False])))
            self.put(at+0xc,'II',self.sound+(i%3)*0x100,self.sound)
            self.put(at+0x14,'I',self.actor if i%2 else 0)
            self.put(at+0x20,'II',rng.choice([0,4,0x100]),rng.choice([0,2,3,6,100,101]))
            self.put(at+0x2c,'i',-1);self.put(at+0x30,'f',rng.choice([0.,.1,.5,1.,2.]))
            self.put(at+0x34,'IiI',rng.randrange(3),rng.choice([-7,0,1,5]),rng.randrange(2))
            self.put(at+0x5c,'fff',float(i),2.,3.);self.put(at+0x68,'fff',4.,5.,6.)
            self.put(at+0x74,'ffffd',40.,40.,40.,40.,-1.)
            self.put(at+0x8c,'fff',1.1,.75,.5)
            self.call(0x103d1cf0,at+0x40,[bits(.75),0,0])
            self.put(at+0xac,'II',1000+i,2000+i)
            self.targets[at]={'Active':i}
        self.put(self.audio+0x2a4,'II',self.pool_array,count)
        for i in range(count):self.put(self.pool_array+i*4,'I',self.node_at[i])
        self.put(self.audio+0x2ac,'I',self.node_at[count]);self.put(self.audio+0x2c,'I',self.engine)
        for off,fn in [(0xb8,0x10001720),(0xbc,0x10001730),(0xc0,0x100014a0),(0xcc,0x103d0d10),(0xd0,0x10005890)]:self.put(self.vtables+0x1000+off,'I',fn)
        for off,fn in [(4,0x10358eb0),(0x10,0x10358f10),(0x14,self.playingstub),(0x18,self.finishedstub),(0x20,self.reply),(0x28,self.updatestub)]:self.put(self.vtables+off,'I',fn)
        self.put(self.audio+0x3c,'II',self.queue_array,0)
        for i in range(3):self.put(self.vtables+0x2000+i*0x100+0x78,'I',self.validstub2)
        self.put(0x1000a214,'I',0x103590a0)

    def run_prepare(self,incoming,request):
        self.mode='prepare';self.current_request=request;self.trace=[];self.prepared=None
        self.put(self.audio+0x2c,'I',self.engine if request['engine_present'] else 0)
        self.driver_replies[incoming]=request['queued']
        self.call(0x10008800,self.audio,[incoming]);self.mode=None
        return self.prepared,self.trace[:]

    def census(self,rng,pending):
        slots=self.slot_nodes();self.mode=None;self.trace=[];self.call_counts={};self.after={};self.mutations={}
        params=[]
        for i in range(3):
            flags=rng.choice([0,1,2,3,0x80]);self.put(0x500d000+i*0x100+0x2c,'I',flags)
            params.append(dict(sound=self.sound+i*0x100,flags=flags))
        frames=[]
        for node in slots:
            at=self.node_at[node];reply=rng.choice([0,1,-3]);self.replies[node]=[reply]
            dt=f32(rng.choice([0.,1/60,.1,1.]))
            self.mutations[node]=dict(cursor=rng.randrange(3),voice_exempt=bool(rng.randrange(2)),sound=self.sound+rng.randrange(3)*0x100,priority_bits=bits(rng.choice([0.,.4,1.])))
            # All listeners deliberately share this clock in a census case.
            frames.append(dict(playing=reply,seconds_bits=bits(dt),after=self.snapshot(at)))
        dt=f32(rng.choice([0.,1/60,.1,1.]))
        for frame in frames:frame['seconds_bits']=bits(dt)
        self.put(self.listener+0x14,'d',float(dt));self.put(self.audio+0x104,'I',pending)
        self.put(self.audio+0x100,'I',0xdeadbeef);self.put(0x107a8be4,'I',99)
        bp=self.stack+0x800;self.u.reg_write(core.UC_X86_REG_EBP,bp);self.u.reg_write(core.UC_X86_REG_ESI,self.audio)
        self.u.reg_write(core.UC_X86_REG_ESP,self.stack-0x2000)
        self.mode='census';self.u.emu_start(0x103d99cc,0x103d9a60,count=100000);self.mode=None
        assert self.u.reg_read(core.UC_X86_REG_EIP)==0x103d9a60
        for node,frame in zip(slots,frames):
            if node in self.after:frame['after']=self.after[node]
        return dict(frames=frames,params=params,events=self.trace[:],status=dict(pending=pending,current=self.get(self.audio+0x100),ducking_voices=self.get(0x107a8be4),any_playing=bool(self.get(bp-0x11,'B'))))


def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--game-data',type=Path,required=True);ap.add_argument('--output',type=Path,required=True);ap.add_argument('--verify-fixture',type=Path);a=ap.parse_args()
    h=Harness(a.game_data);rng=random.Random(0x41435435);copies=[];swaps=[];available=[];preparations=[];refresh=[];censuses=[];cycles=[]
    for i in range(128):
        h.setup_pool(rng,i%9);src=h.node_at[-1];dst=h.node_at[-2]
        before=h.pool_state();counter=h.get(0x107a8bec);op=0x10358f70 if i%2 else 0x103590a0
        h.call(op,dst,[src]);copies.append(dict(before=before,source=len(h.node_at)-1,destination=len(h.node_at)-2,constructor=bool(i%2),expected=h.pool_state(),counter_before=counter,counter_after=h.get(0x107a8bec)))
        h.setup_pool(rng,i%9)
        if i%7==0:h.put(h.audio+0x2ac,'I',0)
        if i%5==0 and i%9>1:h.put(h.pool_array+4,'I',h.node_at[0])
        node=rng.randrange(len(h.node_at));before=h.pool_state();h.mode='swap';h.trace=[]
        h.call(0x10005890,h.audio,[h.node_at[node]]);h.mode=None
        state=h.pool_state();getter=h.call(0x100014a0,h.audio)
        swaps.append(dict(before=before,node=node,events=h.trace[:],expected=state,getter=None if not getter else h.node(getter)))
        h.setup_pool(rng,i%9);before=h.pool_state();finished=[rng.choice([0,0,1,-7]) for _ in h.node_at]
        h.finished=dict(enumerate(finished));h.mode='available';result=h.call(0x10004380,h.audio);h.mode=None
        available.append(dict(before=before,finished=finished[:i%9],expected=None if not result else h.node(result)))
    for i in range(256):
        h.setup_pool(rng,i%9);incoming=h.node_at[-1]
        if i%13==0:h.put(incoming+0xc,'I',0)
        if i%17==0:h.put(h.audio+0x2ac,'I',0)
        request=dict(engine_present=i%19!=0,sound_valid=i%11!=0,buffer=0 if i%7==0 else 7000+i,queued=rng.choice([0,1,-2]))
        before=h.pool_state();source=h.snapshot(incoming);result,events=h.run_prepare(incoming,request)
        preparations.append(dict(before=before,incoming=source,request=request,prepared=result,events=events,expected=h.pool_state()))
    for i in range(128):
        h.setup_pool(rng,i%9);pending=rng.choice([0,1,2,0x80000000]);h.put(h.audio+0x104,'I',pending)
        slots=h.slot_nodes();replies=[]
        for node in slots:
            r=[rng.choice([0,1,-3]),rng.choice([0,1,-7])];replies.append(r);h.replies[node]=r
        before=h.pool_state();h.mode='refresh';h.trace=[];h.call_counts={};h.call(0x103d0e40,h.audio);h.mode=None
        refresh.append(dict(before=before,pending=pending,replies=replies,events=h.trace[:],current=h.get(h.audio+0x100)))
        h.setup_pool(rng,i%9);before=h.pool_state();c=h.census(rng,pending)
        censuses.append(dict(before=before,**c,expected=h.pool_state()))
    for i in range(64):
        h.setup_pool(rng,i%7);queue=[0x5068000+j*0x100 for j in range(3)]
        for j,at in enumerate(queue):
            h.u.mem_write(at,bytes(h.u.mem_read(h.node_at[-1],0xb4)));h.put(at+0x28,'I',100+j);h.targets[at]={'Queued':j};h.put(h.queue_array+j*4,'I',at)
        h.put(h.audio+0x3c,'II',h.queue_array,len(queue));pending=rng.choice([0,2,0x80000000]);counter=0xffffffff
        h.put(0x107a8bec,'I',counter)
        initial=dict(pool=h.pool_state(),waiting=[h.snapshot(at) for at in queue],counter=counter,status=dict(pending=pending,current=7,ducking_voices=3,any_playing=False))
        order=list(range(3));rng.shuffle(order);requests=[dict(engine_present=True,sound_valid=j!=i%5,buffer=8000+j,queued=j%2) for j in range(3)]
        events=[];prepared=[]
        for j in order:
            result,trace=h.run_prepare(queue[j],requests[j]);prepared.append(result);events.extend(dict(Prepare=dict(queue=j,event=e)) for e in trace)
        c=h.census(rng,pending);events.extend(dict(Census=e) for e in c['events'])
        # Native queue destruction after the census, through the existing oracle.
        h.mode=None;h.events=[];h.record=True;h.call(0x103d66f0,h.audio);h.record=False
        for event in h.events:
            deleted=event['Delete'];events.append(dict(Delete=dict(queue=deleted['target']['Queued'],unique_id=deleted['unique_id'])))
        assert len(h.events)==len(queue)
        expected=dict(pool=h.pool_state(),waiting=[],counter=h.get(0x107a8bec),status=c['status'])
        cycles.append(dict(before=initial,order=order,requests=requests,frames=c['frames'],params=c['params'],prepared=prepared,events=events,expected=expected))
    data=dict(schema=1,binary_sha256=dict(h.hashes,alaudio=h.al_hash),copies=copies,swaps=swaps,available=available,preparations=preparations,refresh=refresh,censuses=censuses,cycles=cycles)
    if a.verify_fixture:assert json.loads(a.verify_fixture.read_text())==data,'Activation fixture mismatch'
    a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(data,separators=(',',':'))+'\n')
    report=dict(passed=True,copy_cases=len(copies),transient_swaps=len(swaps),available_cases=len(available),preparation_cases=len(preparations),cue_refresh_cases=len(refresh),census_cases=len(censuses),handoff_cycles=len(cycles),fixture_verified=bool(a.verify_fixture))
    a.output.with_suffix('.validation.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
if __name__=='__main__':main()
