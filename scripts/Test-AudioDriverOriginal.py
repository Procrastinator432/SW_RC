"""Whole original FALSource controls/status/completion on synthetic resources.
AL play/pause/stop/get/unqueue and Sound Duration are controlled callbacks.
No device, PCM decoding/upload or stream refill. Records callback ordering and
all source/buffer writes, including four returned unqueue IDs.
"""
import argparse,importlib.util,json,random,struct
from pathlib import Path
spec=importlib.util.spec_from_file_location('activation_original',Path(__file__).with_name('Test-AudioActivationOriginal.py'))
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
c=m.core;bits=m.bits
FUNCTIONS=dict(Play=0x10004f90,Pause=0x10005040,Stop=0x10006760,IsPlaying=0x10004f40,IsPaused=0x10004f60,IsQueued=0x10004f50,IsStopped=0x10005110,IsFinished=0x100051c0)
class Harness(m.Harness):
    def __init__(self,path):
        super().__init__(path);self.driver_events=[];self.active=False
        self.apis={self.stub+0x4000+i*0x20:name for i,name in enumerate(['AlPlay','AlPause','AlStop','Integer','Unqueue','GetState','Duration'])}
        for at,name in self.apis.items():
            code=b'\xc3'
            if name=='Duration':code=b'\xd9\x05'+struct.pack('<I',self.result+0x4000)+b'\xc3'
            self.u.mem_write(at,code)
        def stub(name):return next(a for a,n in self.apis.items() if n==name)
        for iat,name in [(0x10012230,'AlPlay'),(0x10012260,'AlPause'),(0x10012168,'AlStop'),(0x1001220c,'Integer'),(0x10012220,'Unqueue'),(0x10012228,'GetState')]:self.put(iat,'I',stub(name))
        self.handle_fn=0x103d06c0;self.duration_stub=stub('Duration')
        self.buffer_at=0x5070000;self.storage_at=0x5071000;self.reply_state={}
    def hook(self,u,at,size,data):
        super().hook(u,at,size,data)
        if not self.active:return
        sp=u.reg_read(c.UC_X86_REG_ESP);context=u.reg_read(c.UC_X86_REG_ECX);node=self.node_at[0]
        if at in self.apis:
            name=self.apis[at];args=[self.get(sp+4+i*4) for i in range(3)]
            if name in ['AlPlay','AlPause','AlStop']:
                event={name:dict(hardware=args[0],playing_before=self.get(node+0x34),pause_before=self.get(node+0x38,'i'))}
            elif name=='Integer':event=dict(Integer=dict(hardware=args[0],parameter=args[1],value=args[2]))
            elif name=='Unqueue':
                assert args[1]==4
                event=dict(Unqueue=dict(hardware=args[0],count=args[1],buffers=self.words(args[2],4)))
                self.put(args[2],'4I',*self.reply_state['unqueued'])
            elif name=='GetState':
                event=dict(GetState=dict(hardware=args[0],parameter=args[1]));self.put(args[2],'i',self.reply_state['source_state'])
            else:event=dict(Duration=dict(sound=context))
            self.driver_events.append(event)
        elif at==0x103d06c0:self.driver_events.append(dict(SetHandle=dict(sound=context,request=self.get(sp+4),previous=self.get(context+0x64,'i'),buffer_before=self.get(node+0xb0),allocated_before=bool(self.get(node+8)))))
        elif at==0x10358eb0:self.driver_events.append(dict(Clear=dict(buffer_before=self.get(node+0xb0))))
    def setup(self,rng,i):
        self.active=False;self.setup_pool(rng,1);node=self.node_at[0]
        for off,addr in [(8,FUNCTIONS['Play']),(0xc,FUNCTIONS['Pause']),(0x10,FUNCTIONS['Stop']),(0x14,FUNCTIONS['IsPlaying']),(0x18,FUNCTIONS['IsFinished']),(0x1c,FUNCTIONS['IsPaused']),(0x20,FUNCTIONS['IsQueued'])]:self.put(self.vtables+off,'I',addr)
        self.put(node+8,'I',int(i%7!=0));self.put(node+0xc,'I',self.sound)
        self.put(node+0x34,'Ii',rng.choice([0,1,3,0xffffffff]),rng.choice([-7,0,1,5]))
        self.put(node+0x20,'I',rng.choice([0,2,4,0x4004,0x4104]))
        self.put(node+0xa4,'If',rng.randrange(3),rng.choice([0.,-1.,.0001,.00010001,1.]))
        duration=rng.choice([0.,.125,1.,10.,1e6]);self.put(self.result+0x4000,'f',duration)
        lifetime=float(duration)+.5+rng.choice([-.000001,0.,.000001,1.]);self.put(node+0x18,'d',lifetime)
        if i%13==0:self.put(node+0x18,'d',-1.)
        self.put(node+0xb0,'I',self.buffer_at if i%11!=0 else 0)
        self.count=rng.randrange(6);self.put(self.buffer_at+4,'I',0 if i%3==0 else 0x12345678)
        self.put(self.buffer_at+0x10,'II',self.storage_at,self.count|((i%8)<<29))
        self.storage=[101+j for j in range(max(4,self.count))];self.put(self.storage_at,'I'*len(self.storage),*self.storage)
        flags=rng.randrange(4);self.put(self.sound+0x30,'B',flags)
        self.put(self.vtables+0x2000+0x94,'I',self.handle_fn);self.put(self.vtables+0x2000+0xa0,'I',self.duration_stub)
        self.put(self.sound+0x64,'i',rng.choice([-1,0,300,9999]))
        self.sound_flags=flags;self.duration_bits=bits(duration)
        return node
    def state(self):
        at=self.node_at[0]
        return dict(node=dict(source=self.snapshot(at),hardware=self.get(at+0xac),buffer=self.get(at+0xb0)),buffers=[dict(handle=self.buffer_at,stream_token=self.get(self.buffer_at+4),count_word=self.get(self.buffer_at+0x14),storage=self.words(self.storage_at,len(self.storage)))],sounds=[dict(handle=self.sound,flags=self.sound_flags,duration_bits=self.duration_bits,native_handle=self.get(self.sound+0x64,'i'))])
    def execute(self,command,reply):
        node=self.node_at[0];name='Pause' if isinstance(command,dict) else command;args=[command['Pause']&0xffffffff] if name=='Pause' else []
        self.reply_state=reply;self.driver_events=[];self.active=True
        result=self.call(FUNCTIONS[name],node,args);self.active=False
        return dict(result=result,events=self.driver_events[:])

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--game-data',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--verify-fixture',type=Path);a=p.parse_args()
    h=Harness(a.game_data);rng=random.Random(0x44525635);cases=[];sequences=[]
    commands=['Play',dict(Pause=0),dict(Pause=-7),dict(Pause=1),'Stop','IsPlaying','IsPaused','IsQueued','IsStopped','IsFinished']
    for i in range(800):
        node=h.setup(rng,i);command=commands[i%len(commands)]
        # Stop assumes valid sound and buffer if allocated; keep native pointer
        # contract valid instead of crashing the original on a fabricated null.
        if command=='Stop' and h.get(node+8):h.put(node+0xb0,'I',h.buffer_at)
        # Enumerate completion flags/token/fade/duration boundaries independently
        # of operation-index periodicity.
        if command=='IsFinished':
            j=i//10;h.put(node+8,'I',int(j%9!=0));h.put(node+0xb0,'I',0 if j%11==0 else h.buffer_at)
            h.put(node+0x20,'I',[0,4,0x4004][j%3]);h.put(h.buffer_at+4,'I',0 if j%4<2 else 0x12345678)
        before=h.state();reply=dict(source_state=rng.choice([0,0x1011,0x1012,0x1013,0x1014,-7]),unqueued=[rng.randrange(1000,2000) for _ in range(4)])
        result=h.execute(command,reply);cases.append(dict(before=before,command=command,reply=reply,outcome=result,expected=h.state()))
    for i in range(96):
        node=h.setup(rng,i+1);h.put(node+0xb0,'I',h.buffer_at);before=h.state();steps=[]
        order=['IsPlaying','IsQueued','Play','IsPlaying',dict(Pause=-7),'IsPaused','IsPlaying','IsStopped','IsFinished',dict(Pause=0),'IsPaused','Play','IsFinished','Stop','IsFinished','IsPlaying','IsStopped',dict(Pause=1),'Play','Stop']
        for command in order:
            reply=dict(source_state=rng.choice([0x1012,0x1013,0x1014]),unqueued=[rng.randrange(2000,3000) for _ in range(4)])
            outcome=h.execute(command,reply);steps.append(dict(command=command,reply=reply,outcome=outcome,expected=h.state()))
        sequences.append(dict(before=before,steps=steps))
    data=dict(schema=1,binary_sha256=dict(h.hashes,alaudio=h.al_hash),vtable=[dict(offset=off,address=h.get(0x1000b0b4+off)) for off in [4,8,0xc,0x10,0x14,0x18,0x1c,0x20]],sound_vtable=[dict(offset=off,address=h.get(0x10667880+off)) for off in [0x94,0xa0,0xac]],cases=cases,sequences=sequences)
    if a.verify_fixture:assert json.loads(a.verify_fixture.read_text())==data,'Driver fixture mismatch'
    a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(data,separators=(',',':'))+'\n')
    report=dict(passed=True,whole_method_cases=len(cases),native_sequences=len(sequences),sequence_steps=sum(len(s['steps']) for s in sequences),fixture_verified=bool(a.verify_fixture))
    a.output.with_suffix('.validation.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
if __name__=='__main__':main()
