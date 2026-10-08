from pathlib import Path
import json, struct
root=Path(__file__).resolve().parents[1]
p=root/'scripts/Record-SkeletalMaterials.py';ns={'__file__':str(p)}
exec(compile(p.read_text().split('texture_map={}')[0],str(p),'exec'),ns)
paths=sorted({b['material'] for o in ns['report']['objects'] for b in o['bindings'] if b['error']=='Unsupported material class Engine.TexPanner2D'})
for path in paths:
    pn,data,meta,e=ns['asset'](path);props,_=ns['properties'](data,meta,e)
    print(path,props)
data,meta=ns['package'](ns['files']['engine'])
for i,e in enumerate(meta[3],1):
    name=meta[4](i)
    if name.startswith('TexPanner2D.') or name in ['TexPanner2D','Default__TexPanner2D']:
        print('CLASS',name,hex(e[5]),data[e[5]:e[5]+e[4]].hex()[:500])
pe=(ns['game']/'System/engine.dll').read_bytes();h=int.from_bytes(pe[60:64],'little');n=int.from_bytes(pe[h+6:h+8],'little');opt=int.from_bytes(pe[h+20:h+22],'little')
for address in [0x10673b00,0x10653578]:
    rva=address-0x10300000
    for i in range(n):
        s=h+24+opt+40*i;size,va,rawsize,off=struct.unpack_from('<IIII',pe,s+8)
        if va<=rva<va+max(size,rawsize):
            offset=off+rva-va;print('CONSTANT',hex(address),pe[offset:offset+8].hex(),struct.unpack_from('<d',pe,offset)[0])
