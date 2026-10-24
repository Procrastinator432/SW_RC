"""Whole original source pause/stop and queue commands on synthetic sources.
Music, driver pause replies, destructor/free and GMalloc storage are controlled
boundaries. Native queue growth/empty, constructor, Clear, Pause, Stop and curve
setter execute their original instructions. No assets or original code exported.
"""
import argparse
import importlib.util
import json
import random
from pathlib import Path
import pefile

spec=importlib.util.spec_from_file_location('source_original',Path(__file__).with_name('Test-AudioSourcesOriginal.py'))
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
core=module.core
bits,f32=core.bits,core.f32

class Harness(module.Harness):
    def __init__(self,path):
        self.engine_pe=pefile.PE(str(path/'System/engine.dll'))
        super().__init__(path)
        self.record=False;self.events=[];self.targets={};self.driver_replies={}
        self.reply=self.stub+0x600;self.destroy=self.stub+0x620
        self.allocate=self.stub+0x640;self.reallocate=self.stub+0x660;self.outer=self.stub+0x680
        self.u.mem_write(self.reply,b'\xc3');self.u.mem_write(self.destroy,b'\xc2\x04\x00')
        self.u.mem_write(self.allocate,b'\xc2\x04\x00');self.u.mem_write(self.reallocate,b'\xc2\x0c\x00')
        self.u.mem_write(self.outer,b'\xc3')
        for off,fn in [(0,self.destroy),(4,0x10358eb0),(0xc,0x10358f00),(0x10,0x10358f10),(0x1c,self.reply)]:self.put(self.vtables+off,'I',fn)
        for lib in self.engine_pe.DIRECTORY_ENTRY_IMPORT:
            for imp in lib.imports:
                name=imp.name or b''
                if b'GMalloc' in name:self.put(imp.address,'I',self.result+0xa00)
                if b'?GetOuter@UObject' in name:self.put(imp.address,'I',self.outer)
        self.put(self.result+0xa00,'I',self.result+0xa10)
        self.put(self.result+0xa10,'I',self.result+0xa20)
        self.put(self.result+0xa24,'II',self.allocate,self.reallocate)
        self.queue_array=0x5051000;self.allocation_at=0x5060000

    def hook(self,u,at,size,data):
        super().hook(u,at,size,data)
        context=u.reg_read(core.UC_X86_REG_ECX);sp=u.reg_read(core.UC_X86_REG_ESP)
        if at==self.outer:u.reg_write(core.UC_X86_REG_EAX,0x500b400)
        if at==self.reply:u.reg_write(core.UC_X86_REG_EAX,self.driver_replies.get(context,0)&0xffffffff)
        if at==self.allocate:
            size=self.get(sp+4)
            if size==0xac:
                self.u.mem_write(self.allocation_at,bytes(0x100))
                u.reg_write(core.UC_X86_REG_EAX,self.allocation_at);self.allocation_at+=0x100
            else:
                assert size<=4096*4
                u.reg_write(core.UC_X86_REG_EAX,self.queue_array)
        if at==self.reallocate:
            u.reg_write(core.UC_X86_REG_EAX,self.queue_array if self.get(sp+8) else 0)
        if not self.record:return
        if at in [0x103ab310,0x103ab320,0x103aba80]:
            self.events.append({'MusicPause':at==0x103ab310} if at!=0x103aba80 else 'MusicStop')
            u.reg_write(core.UC_X86_REG_EIP,self.get(sp));u.reg_write(core.UC_X86_REG_ESP,sp+4)
        elif at==0x10358f00:
            self.events.append(dict(Pause=dict(target=self.targets[context],request=self.get(sp+4,'i'))))
        elif at==0x10358f10:
            self.events.append(dict(Stop=dict(target=self.targets[context],voice_exempt=bool(self.get(context+0x3c)))))
        elif at==self.destroy:
            assert self.get(sp+4)==1
            self.events.append(dict(Delete=dict(target=self.targets[context],unique_id=self.get(context+0x28))))
        elif at==0x103d1cf0 and context-0x40 in self.targets:
            self.events.append(dict(Fade=dict(target=self.targets[context-0x40],duration_bits=self.get(sp+8),kind=self.get(sp+12)&255)))

    def bank(self):
        bank=[]
        for i in range(3):
            sound=self.sound+i*0x100;params=0x500d000+i*0x100;vtable=self.vtables+0x2000+i*0x100
            row=dict(handle=sound,default_fade=f32([0.,.3,-1.][i]),streamed=i!=0,curve=i,category=i)
            bank.append(row)
            self.put(sound,'I',vtable);self.put(sound+0x24,'I',0x500c000+i*0x100)
            self.put(0x500c000+i*0x100+0x30,'I',0x107a8e20 if row['streamed'] else 0)
            self.put(params+0x28,'B',row['category'])
            for off,value,floating in [(0xa4,params,False),(0xb4,bits(row['default_fade']),True),(0xd0,row['curve'],False),(0xcc,row['curve'],False)]:
                stub=self.stub+0x1000+i*0x200+(off-0xa0)*4
                value_at=self.result+0x1000+i*0x200+(off-0xa0)*4
                self.put(value_at,'I',value)
                self.u.mem_write(stub,(b'\xd9\x05' if floating else b'\xa1')+core.struct.pack('<I',value_at)+b'\xc3')
                self.put(vtable+off,'I',stub)
        return bank

    def setup(self,rng,case):
        self.record=False;self.events=[];self.targets={};self.driver_replies={};self.allocation_at=0x5060000
        self.put(self.audio+0x2c,'I',self.engine);self.put(self.engine+0x30,'I',self.actor)
        bank=self.bank();active_n=case%9;waiting_n=(case*7)%9
        counter=[0xfffffffe,0,123][case%3]
        active=[];waiting=[]
        for i in range(active_n+waiting_n):
            at=self.pool+i*0x100
            self.construct(counter,at);counter=self.get(0x107a8bec)
            self.put(at,'I',self.vtables)
            self.put(at+8,'I',rng.randrange(2))
            self.put(at+0xc,'II',rng.choice(bank)['handle'],rng.choice([0]+[b['handle'] for b in bank]))
            self.put(at+0x14,'I',rng.choice([0,self.actor,self.actor+0x100]))
            self.put(at+0x20,'II',rng.choice([0,4,0x10,0x80,0x104]),rng.choice([0,1,2,6,0x100]))
            self.put(at+0x3c,'I',rng.randrange(2))
            self.call(0x103d1cf0,at+0x40,[bits(rng.choice([.2,.75,1.])),0,0])
            is_queue=i>=active_n;j=i-active_n if is_queue else i
            self.targets[at]={'Queued' if is_queue else 'Active':j}
            self.driver_replies[at]=rng.choice([-3,0,1,7])
            (waiting if is_queue else active).append(at)
            self.put((self.queue_array if is_queue else self.pool_array)+j*4,'I',at)
        self.put(self.result+0x114,'I',active_n)
        self.put(self.audio+0x3c,'II',self.queue_array,waiting_n)
        self.put(self.audio+0x114,'I',rng.choice([0,0x12345678]+[self.get(at+0x28) for at in active+waiting]))
        self.put(0x107a8bec,'I',0xffffffff if case%16==7 else counter)
        self.put(self.audio+0xd8,'fff',20.,80.,30.)
        self.active_at=active
        return bank

    def state(self):
        count=(self.get(self.audio+0x40)&0x1fffffff)
        arr=self.get(self.audio+0x3c)
        waiting=[self.get(arr+i*4) for i in range(count)]
        return dict(active=[self.snapshot(at) for at in self.active_at],waiting=[self.snapshot(at) for at in waiting],
                    counter=self.get(0x107a8bec),cue_id=self.get(self.audio+0x114))

    def invoke(self,command,bank):
        op=command['op'];self.events=[];self.record=True;result=None
        if op=='pause_sound':self.call(0x103d2e90,self.audio,[command['actor'],command['sound'],command['request']&0xffffffff])
        elif op=='pause_all':self.call(0x103d30a0,self.audio,[command['request']&0xffffffff,command['exclude']])
        elif op=='stop_sound':result=bool(self.call(0x103d6b10,self.audio,[command['actor'],command['sound'],bits(command['fade'])]))
        elif op=='stop_all':self.call(0x103d62e0,self.audio,[command['exclude']])
        elif op=='stop_voices':self.call(0x103d13e0,self.audio,[command['actor']])
        elif op=='empty':self.call(0x103d66f0,self.audio)
        elif op=='paused':
            query=command['query']
            if query['sound']:
                p=0x500d000+((query['sound']-self.sound)//0x100)*0x100
                self.put(p+0x54,'I',int(query['auto_actor']));self.put(query['sound']+0x30,'I',2 if query['auto_actor'] else 0)
            self.put(self.engine+0x30,'I',query['engine_actor'])
            result=self.call(0x103d69f0,self.audio,[query['actor'],query['sound']])
            if result>=0x80000000:result-=0x100000000
        elif op=='enqueue':
            # The original whole allocation and TArray growth execute; only
            # allocation storage is supplied by the host allocator boundary.
            self.record=False
            at=self.call(0x103d8c70,self.audio)
            index=(self.get(self.audio+0x40)&0x1fffffff)-1
            self.targets[at]={'Queued':index};self.driver_replies[at]=0
            self.put(at,'I',self.vtables)
            inp=command['input'];self.put(0x500b300,'fff',*inp['location'])
            meta=next(s for s in bank if s['handle']==inp['sound'])
            self.put(0x500d000+((meta['handle']-self.sound)//0x100)*0x100+0x2c,'I',int(inp['voice_exempt']))
            # Init fade curve is returned at +cc by the chosen sound object.
            self.call(0x103d2a40,at,[0,inp['actor'],inp['sound'],inp['original_sound'],inp['flags'],inp['identity'],
                 inp['engine_index']&0xffffffff,0x500b300,*[bits(inp[k]) for k in ['volume','initial_output','pitch','fade_seconds','radius','ambient_priority']]])
            result=index
        else:raise AssertionError(op)
        self.record=False
        return dict(command=command,result=result,events=self.events,expected=self.state())


def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--game-data',type=Path,required=True)
    ap.add_argument('--output',type=Path,required=True);ap.add_argument('--verify-fixture',type=Path);args=ap.parse_args()
    h=Harness(args.game_data);rng=random.Random(0x52435135);cases=[]
    # Independent whole-command cases prevent Clear from hiding later filters.
    names=['pause_sound','paused','pause_all','stop_sound','stop_all','stop_voices','empty','enqueue']
    for i in range(512):
        bank=h.setup(rng,i);op=names[i%len(names)]
        actor=rng.choice([0,h.actor,h.actor+0x100]);sound=rng.choice([0]+[s['handle'] for s in bank])
        if op=='pause_sound':cmd=dict(op=op,actor=actor,sound=sound,request=rng.choice([-7,0,1,5]))
        elif op=='paused':
            if actor==0 and sound==0:sound=bank[0]['handle']
            cmd=dict(op=op,query=dict(actor=actor,sound=sound,auto_actor=bool(i%3),engine_actor=rng.choice([0,h.actor])))
        elif op=='pause_all':cmd=dict(op=op,request=rng.choice([-7,0,1,5]),exclude=rng.choice([0,4,0x10,0x80,0xffffffff]))
        elif op=='stop_sound':cmd=dict(op=op,actor=actor,sound=sound,fade=f32(rng.choice([-2.,-1.,0.,.05,.3])))
        elif op=='stop_all':cmd=dict(op=op,exclude=rng.choice([0,4,0x10,0x80,0x90,0x80000000,0xffffffff]))
        elif op=='stop_voices':cmd=dict(op=op,actor=actor)
        elif op=='empty':cmd=dict(op=op)
        else:
            meta=rng.choice(bank)
            cmd=dict(op=op,input=dict(engine_present=True,sound=meta['handle'],original_sound=meta['handle'],actor=actor,
                flags=rng.choice([0,4,0x100]),identity=rng.choice([0,2,6,10]),engine_index=-1,location=[1.,2.,3.],
                volume=.75,pitch=1.1,initial_output=.75,fade_seconds=f32(.3),fade_curve=meta['curve'],radius=100.,
                ambient_priority=f32(.4),radius_caps=[20.,80.,30.],voice_exempt=bool(i%3)))
            cmd['input']['pitch']=f32(cmd['input']['pitch'])
        before=h.state();replies=dict(active=[h.driver_replies[at] for at in h.active_at],waiting=[h.driver_replies[h.get(h.queue_array+j*4)] for j in range(len(before['waiting']))])
        cases.append(dict(before=before,bank=bank,replies=replies,**h.invoke(cmd,bank)))
    sequences=[]
    for i in range(64):
        bank=h.setup(rng,1000+i);before=h.state()
        replies=dict(active=[h.driver_replies[at] for at in h.active_at],waiting=[h.driver_replies[h.get(h.queue_array+j*4)] for j in range(len(before['waiting']))])
        meta=bank[i%3]
        inp=dict(engine_present=True,sound=meta['handle'],original_sound=meta['handle'],actor=h.actor,flags=0,
                 identity=6,engine_index=-1,location=[1.,2.,3.],volume=.75,initial_output=.75,pitch=f32(1.1),
                 fade_seconds=f32(.3),fade_curve=meta['curve'],radius=100.,ambient_priority=f32(.4),radius_caps=[20.,80.,30.],voice_exempt=True)
        commands=[dict(op='pause_all',request=1,exclude=4),dict(op='pause_sound',actor=h.actor,sound=0,request=0),
                  dict(op='enqueue',input=inp),dict(op='stop_sound',actor=h.actor,sound=meta['handle'],fade=f32(.3)),
                  dict(op='pause_all',request=0,exclude=0xffffffff),dict(op='stop_all',exclude=0x90),
                  dict(op='empty'),dict(op='enqueue',input=inp),
                  dict(op='paused',query=dict(actor=h.actor,sound=meta['handle'],auto_actor=False,engine_actor=h.actor))]
        steps=[]
        for command in commands:
            state=h.state();arr=h.get(h.audio+0x3c)
            step_replies=dict(active=[h.driver_replies[at] for at in h.active_at],waiting=[h.driver_replies[h.get(arr+j*4)] for j in range(len(state['waiting']))])
            steps.append(dict(replies=step_replies,**h.invoke(command,bank)))
        sequences.append(dict(before=before,bank=bank,steps=steps))
    data=dict(schema=1,binary_sha256=h.hashes,vtable=[dict(offset=off,address=h.get(0x10666b68+off)) for off in [0x1c,0x20]],cases=cases,sequences=sequences)
    if args.verify_fixture:assert json.loads(args.verify_fixture.read_text())==data,'Command fixture mismatch'
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(data,separators=(',',':'))+'\n')
    report=dict(passed=True,cases=len(cases),per_command={n:sum(c['command']['op']==n for c in cases) for n in names},sequence_steps=sum(len(s['steps']) for s in sequences),fixture_verified=bool(args.verify_fixture))
    args.output.with_suffix('.validation.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
if __name__=='__main__':main()
