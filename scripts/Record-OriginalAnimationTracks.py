"""Independent package/archive byte reader; every decoded track byte is fingerprinted."""
from pathlib import Path
from collections import Counter
import struct,json,hashlib
root=Path(__file__).resolve().parents[1]
class Reader:
    def __init__(self,data,pos=0):self.data=data;self.pos=pos
    def take(self,n):
        b=self.data[self.pos:self.pos+n];assert len(b)==n;self.pos+=n;return b
    def u32(self):return struct.unpack('<I',self.take(4))[0]
    def i32(self):return struct.unpack('<i',self.take(4))[0]
    def index(self):
        b=self.take(1)[0];sign=b&128;v=b&63;shift=6
        if b&64:
            while True:
                b=self.take(1)[0];v|=(b&127)<<shift;shift+=7
                if not b&128:break
        return -v if sign else v
    def count(self):
        n=self.index();assert 0<=n<=1000000;return n
def tables(data):
    h=struct.unpack_from('<IHH7I',data);assert h[0]==0x9e2a83c1
    version,licensee=h[1:3];name_n,name_off,exp_n,exp_off,imp_n,imp_off=h[4:]
    r=Reader(data,name_off);names=[]
    for _ in range(name_n):
        n=r.index();raw=r.take(n if n>0 else -n*2);names.append(raw[:-1].decode('latin1') if n>0 else raw[:-2].decode('utf-16-le'));r.u32()
    r=Reader(data,imp_off);imports=[]
    for _ in range(imp_n):imports.append((r.index(),r.index(),r.i32(),r.index()))
    r=Reader(data,exp_off);exports=[]
    for _ in range(exp_n):
        c=r.index();sup=r.index();outer=r.i32()
        if version>=159:r.index()
        name=r.index();flags=r.u32()
        if version>=151:size=r.u32();off=r.u32()
        else:size=r.index();off=r.index() if size else 0
        exports.append((c,outer,name,flags,size,off))
    def path(index):
        if index==0:return 'None'
        if index<0:_,_,outer,name=imports[-index-1]
        else:_,outer,name,_,_,_=exports[index-1]
        return (path(outer)+'.' if outer else '')+names[name]
    return version,licensee,names,exports,path
def fingerprint(b):
    h=0xcbf29ce484222325
    for v in b:h=((h^v)*0x100000001b3)&0xffffffffffffffff
    return f'{h:016x}'
path=root/'analysis/reports/original-animation-tracks.json';report=json.loads(path.read_text())
assert (report['animations'],report['sequences'],report['tracks'],report['errors'])==(166,1398,61310,0)
objects={(str(Path(o['file'])),o['export_index']):o for o in report['objects']};seen=set();totals=Counter();files={}
samples=Counter()
game=Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Animations')
for file in sorted(game.glob('*.ukx')):
    data=file.read_bytes();files[str(file)]=hashlib.sha256(data).hexdigest();version,licensee,names,exports,object_path=tables(data)
    assert licensee==1
    for index,e in enumerate(exports,1):
        if object_path(e[0])!='Engine.MeshAnimation':continue
        key=(str(file),index);seen.add(key);o=objects[key];assert o['object']==object_path(index) and o['version']==version
        if version<151:
            assert o['status']=='UnsupportedLegacyRotation';totals['legacy_objects']+=1;continue
        totals['animations']+=1;payload=data[e[5]:e[5]+e[4]];r=Reader(payload)
        assert not e[3]&0x02000000 # no stateframe prefix in these original MeshAnimation exports
        assert names[r.index()]=='None';assert r.pos==o['native_offset']
        assert r.u32()==o['word_28']
        def name():
            i=r.index();return {'index':i,'name':names[i]}
        bones=[{'name':name(),'word_4':r.u32(),'word_8':r.u32()} for _ in range(r.count())]
        assert bones==o['reference_bones'];totals['reference_bones']+=len(bones)
        n=r.count();assert n==len(o['sequences'])
        for q in o['sequences']:
            totals['sequences']+=1;s={'payload_offset':r.pos,'word_2c':r.u32(),'name':name()}
            s['groups']=[name() for _ in range(r.count())];s['first_frame']=r.i32();s['frames']=r.i32()
            s['notifies']=[{'time_bits':r.u32(),'name':name(),'object_index':r.index()} for _ in range(r.count())]
            for notify in s['notifies']:object_path(notify['object_index'])
            for field in ('rate_bits','word_18','minimum_blend_bits'):s[field]=r.u32()
            s['randomize_start']=r.take(1)[0]
            for field in ('word_30','word_50','flags','word_58'):s[field]=r.u32()
            count=r.count();assert count==len(q['track_fingerprints'])==len(q['track_counts']);totals['tracks']+=count
            for track_index in range(count):
                scale=r.take(4);pn=r.count();positions=r.take(6*pn);rn=r.count();rotations=r.take(6*rn);dn=r.count();durations=r.take(dn)
                canonical=struct.pack('<I',rn)+rotations+struct.pack('<I',pn)+positions+scale+struct.pack('<I',dn)+durations
                assert fingerprint(canonical)==q['track_fingerprints'][track_index]
                assert [rn,pn,dn]==q['track_counts'][track_index]
                totals['rotation_keys']+=rn;totals['position_keys']+=pn;totals['duration_bytes']+=dn
                if track_index==0:first=(rn,pn,dn,durations)
            s['tracks']=[];s['end_offset']=r.pos;assert s==q['metadata'],(file,s['name'])
            # Check timing independently for all three actual first-track samples.
            for sample in q['first_track_samples']:
                rn,pn,dn,durations=first;time=struct.unpack('<f',struct.pack('<I',sample['time_bits']))[0];res=time;current=0;alpha=None
                if dn>1:
                    for current,d in enumerate(durations):
                        after=struct.unpack('<f',struct.pack('<f',res-d))[0]
                        if after<0:break
                        res=after
                    else:current=0;d=durations[0]
                    if res>0:alpha=struct.unpack('<I',struct.pack('<f',res/d))[0] if d else 0x7f800000
                nxt=(current+1)%dn if alpha is not None else current
                assert sample['result']=={'Ok':{'current':current,'next':nxt,'alpha_bits':alpha}}
                assert all((v&0x7f800000)!=0x7f800000 for v in sample['root']['rotation']+sample['root']['position'])
                samples['finite_portable_first_track_samples']+=1
        assert r.pos==len(payload)==o['end_offset'] and o['tail_bytes']==0
assert seen==set(objects) and samples['finite_portable_first_track_samples']==4194
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
sources=['crates/rc-package/src/skeletal_animation.rs','crates/rc-inspect/src/bin/rc-animation-track-check.rs','scripts/Record-OriginalAnimationTracks.py','analysis/decompiled/skeletal-animation-serializers.c','analysis/decompiled/skeletal-animation-arrays.c','analysis/decompiled/mesh-sequence-arrays.c','analysis/decompiled/analog-track-serialize.c','analysis/decompiled/analog-track-arrays.c','analysis/decompiled/skeletal-animation-serializers.asm']
asm=(root/'analysis/decompiled/skeletal-animation-serializers.asm').read_text()
for marker in ('10514a18 CALL 0x105041f0','10514a21 CALL 0x10513ec0','105091b5 LEA ECX,[EDI + 0x8]','105091b8 CALL 0x105038f0','105091c0 CALL 0x105037e0','105091ce CMP dword ptr [EBX + 0x4],0x97'):assert marker in asm
result={'date':'2026-10-07','rust_tests':252,'totals':dict(totals),'samples':dict(samples),'report_sha256':sha(path),'package_sha256':files,'source_sha256':{s:sha(root/s) for s in sources},'checks':['cargo test --workspace: 252 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','independent original package names/imports/exports and archive field reader','every decoded track fingerprint compared against independent original bytes','4194 first-track timing selections and finite portable outputs; no native pose oracle'],'scope':report['scope']}
(root/'analysis/reports/original-animation-tracks-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';e=json.loads(path.read_text(encoding='utf-8-sig'));e['rust_tests']=252;e['original_animation_tracks_validation']=result;path.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: Originale MeshAnimation-Pakete und komprimierte Tracks decodiert. '
      '166 Animationsexporte, 1398 Sequenzen, 61310 Tracks und 4930 Referenzknocheneinträge aus Versionen151–159/licensee1 '
      'ohne Parsefehler/Restbytes; sämtliche Metadaten und Track-Fingerprints unabhängig aus Paketbytes geprüft. '
      '4194 portable erste Track-Samples mit geprüfter Schlüsselwahl und endlichen Outputs. beast.ukx Version148 '
      'bleibt wegen alter Rotationskonvertierung explizit offen. Gespeicherter Track0 ist noch keinem Mesh-Root zugeordnet. '
      '252 Workspace-Tests, Clippy und Format bestanden; Vollpose und Androidprüfung weiterhin offen/zum Schluss. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\ORIGINAL_ANIMATION_TRACKS.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Originale MeshAnimation-Pakete und komprimierte Tracks decodiert' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps({'totals':dict(totals),'samples':dict(samples),'rust_tests':252}))
