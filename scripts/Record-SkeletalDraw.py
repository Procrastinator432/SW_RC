"""Independent section/index, native rigid matrix and diagnostic geometry oracle."""
from pathlib import Path
import collections
import hashlib
import json
import re
import struct
from PIL import Image, ImageChops

root=Path(__file__).resolve().parents[1]
source=root/'scripts/Check-SkinStreams.py'; ns={'__file__':str(source),'__name__':'skin_oracle'}
exec(compile(source.read_text(),str(source),'exec'),ns)
f,bits,value,links,poses,inverse_map,palette=[ns[k] for k in ('f','bits','value','links','poses','inverse_map','palette')]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
lods=ns['checked']('skeletal-lods.json','skeletal-lods-validation.json')
skins=ns['checked']('skeletal-skin-streams.json','skeletal-skin-streams-validation.json')
source=root/'scripts/Record-ReferenceCaches.py'; ref={'__file__':str(source)}
exec(compile(source.read_text().split("original_path=root/")[0],str(source),'exec'),ref)
asm=(root/'analysis/decompiled/skeletal-render.asm').read_text()
code=[]
for line in asm.splitlines():
    if not line or line.startswith('#'):continue
    addr,ins=line.split(' ',1); op,_,args=ins.partition(' ')
    if 0x10510647<=int(addr,16)<=0x10510a12:code.append((op,args))
def product(inverse,pose):
    registers={}; output={}
    def read(op):
        if op.startswith('XMM'):return registers[op]
        return value((pose if 'EAX' in op else inverse)[ns['disp'](op)//4])
    for op,args in code:
        if op not in ('MOVSS','MOVAPS','ADDSS','MULSS'):continue
        dst,src=args.split(',',1)
        if 'EBP' in src:continue # actor matrix preload, outside this product
        v=read(src)
        if dst.startswith('XMM'):
            registers[dst]=v if op in ('MOVSS','MOVAPS') else f(registers[dst]+v if op=='ADDSS' else registers[dst]*v)
        else:output[(ns['disp'](dst)+0x1d8)//4]=bits(v)
    assert set(output)==set(range(16))
    return [output[i] for i in range(16)]
def transformed(words,matrix):
    p=list(map(value,words[:3])); n=list(map(value,words[3:6])); m=list(map(value,matrix))
    out=[bits(f(f(f(f(p[0]*m[i])+f(p[1]*m[4+i]))+f(p[2]*m[8+i]))+m[12+i])) for i in range(3)]
    out += [bits(f(f(f(n[0]*m[i])+f(n[1]*m[4+i]))+f(n[2]*m[8+i]))) for i in range(3)]
    return out+words[6:8]
def section_rows(lod):
    result=[]
    for bank in (0,1):
        cursor=0
        for i,row in enumerate(lod['sections'][bank]):
            result.append(dict(bank=bank,section_index=i,material=row[0],bone=row[5] if bank else None,
                first_index=row[1] if bank else cursor,triangle_count=row[8],min_vertex=row[2],max_vertex=row[3]))
            cursor+=row[8]*3
    return result
def triangles(lod,soft,matrices,inverse):
    for section in section_rows(lod):
        bank=section['bank']; count=section['triangle_count']; start=section['first_index']
        if not count:continue
        indices=lod['indices'][bank][start:start+count*3]
        assert len(indices)==count*3
        assert all(section['min_vertex']<=i<=section['max_vertex'] for i in indices)
        if bank:
            b=section['bone']; matrix=product(inverse[b],matrices[b])
            vertices={i:transformed(lod['stream_vertices'][i],matrix) for i in set(indices)}
        else:
            vertices={i:soft[i]['vertex']['position']+soft[i]['vertex']['normal']+soft[i]['uv'] for i in set(indices)}
        for offset in range(0,len(indices),3):
            corners=indices[offset:offset+3]
            yield section,corners,[vertices[i] for i in corners]

path=root/'analysis/reports/skeletal-draw.json'; report=json.loads(path.read_text())
binary=root/report['binary']; data=binary.read_bytes(); assert data[:12]==b'RCSKDRAW\x01\0\0\0'; position=12
pose_map={o['source_index']:o for o in poses['objects']}; lod_map={o['source_index']:o for o in lods['objects']}
skin_map={o['source_index']:o for o in skins['objects']}; stats=collections.Counter()
assert len(report['objects'])==130
for obj in report['objects']:
    index=obj['source_index']; prior=pose_map[index]; original=lod_map[index]
    if prior['runs']:
        assert obj['pose_source']=='AnimatedFullPose'; matrices=prior['runs'][0]['cases'][0]['full']['matrices']
    else:
        assert obj['pose_source']=='OriginalReferencePose'; matrices=[]
        for i,bone in enumerate(links['objects'][index]['prefix']['bones']):
            m=ref['matrix'](bone['rotation'],bone['position'])
            if i:m=ref['product'](m,matrices[bone['word_34']])
            matrices.append(m)
    assert len(obj['lods'])==len(original['mesh']['lods'])
    for li,(actual,lod) in enumerate(zip(obj['lods'],original['mesh']['lods'])):
        sections=section_rows(lod); assert actual['sections']==sections
        expected_products=[product(inverse_map[index][s['bone']],matrices[s['bone']]) if s['bone'] is not None and s['triangle_count'] else None for s in sections]
        assert actual['rigid_products']==expected_products
        stats['rigid_products']+=sum(m is not None for m in expected_products)
        assert actual['triangles']==sum(s['triangle_count'] for s in sections)
        used=[set(),set()]
        for s in sections:used[s['bank']].update(range(s['first_index'],s['first_index']+s['triangle_count']*3))
        assert actual['unused_indices']==[len(lod['indices'][i])-len(used[i]) for i in (0,1)]
        soft=skin_map[index]['lods'][li]
        for s,corners,vertices in triangles(lod,soft['output'] if soft else [],matrices,inverse_map[index]):
            expected=[index,li,s['bank'],s['section_index'],s['material'],s['bone'] if s['bone'] is not None else 0xffffffff]+corners+sum(vertices,[])
            assert struct.unpack_from('<33I',data,position)==tuple(expected),(index,li,s['section_index'],position)
            position+=132; stats['soft_triangles' if s['bank']==0 else 'rigid_triangles']+=1
        stats['lods']+=1;stats['sections']+=len(sections)
        stats['rigid_only_empty_command_lods']+=not lod['commands'] and bool(lod['sections'][1])
    stats['meshes']+=1
assert position==len(data) and report['triangle_counts']==[315754,252010]
for probe in report['product_probes']:
    assert probe['result']==product(probe['inverse'],probe['pose']);stats['general_matrix_probes']+=1

preview_hashes=[]
for preview in report['previews']:
    index=preview['source_index'];case=pose_map[index]['runs'][0]['cases'][preview['frame']]
    matrices=case['full']['matrices'];lod=lod_map[index]['mesh']['lods'][preview['lod']]
    transforms=[palette(inv,pose) for inv,pose in zip(inverse_map[index],matrices)]
    soft=ns['stream'](lod['commands'],lod['bind_vertices'],transforms)['output']
    snapshot=root/preview['snapshot'];raw=snapshot.read_bytes()
    assert raw[:8]==b'RCSC\x02\0\0\0';count=struct.unpack_from('<I',raw,8)[0];assert count==preview['triangles']
    for ti,(s,_,vertices) in enumerate(triangles(lod,soft,matrices,inverse_map[index])):
        chunk=struct.unpack_from('<16Ii',raw,16+ti*68)
        expected=sum([v[:3] for v in vertices],[])+[s['material']]+sum([v[6:8] for v in vertices],[])
        assert tuple(expected)==chunk[:16] and chunk[16]==s['material']
    ppm=root/preview['ppm']; im=Image.open(ppm);assert im.size==(512,512)
    # Lossless conversion of renderer output; no generated image or painted overlay.
    png=ppm.with_suffix('.png');im.save(png)
    bbox=ImageChops.difference(im,Image.new('RGB',im.size,im.getpixel((0,0)))).getbbox()
    assert bbox and min(bbox[:2])>0 and max(bbox[2:])<512,bbox
    preview_hashes.append(sha(ppm));stats['preview_frames']+=1
assert len(set(preview_hashes))==4
assert (stats['meshes'],stats['lods'],stats['soft_triangles'],stats['rigid_triangles'],stats['general_matrix_probes'])==(130,426,315754,252010,64)
log=(root/'analysis/reports/skeletal-draw-tests.log').read_text()
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)));assert tests==484 and 'test result: FAILED' not in log
sources=['crates/rc-package/src/skeletal_draw.rs','crates/rc-package/src/skeletal_render_product.rs',
         'crates/rc-render/src/skeletal.rs','crates/rc-inspect/src/bin/rc-skeletal-draw-check.rs',
         'scripts/Generate-RigidRenderProduct.py','scripts/Record-SkeletalDraw.py','docs/SKELETAL_DRAW.md',
         'analysis/decompiled/skeletal-render.c','analysis/decompiled/skeletal-render.asm']
result=dict(date='2026-10-08',rust_tests=tests,counts=dict(stats),scope=report['scope'],
    completed_tasks=['original soft section triangle/index/UV/material-slot mapping',
                     'rigid sections including empty-command LODs and native inverse/pose product',
                     'diagnostic renderer bridge and four verified original-animation preview frames'],
    report_sha256=sha(path),binary_sha256=sha(binary),source_sha256={s:sha(root/s) for s in sources},
    preview_sha256={str(root/p['ppm']):sha(root/p['ppm']) for p in report['previews']},
    prior_report_sha256={n:sha(root/'analysis/reports'/n) for n in ['skeletal-lods.json','skeletal-skin-streams.json','loaded-full-animation-ticks.json','reference-caches.json']},
    checks=['cargo test --workspace: 484 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check',
            'all 567764 triangle records checked independently: section, indices, positions, normals and UV words',
            'native rigid products independently interpreted from original scalar SSE; 64 general probes',
            'four snapshot frames independently rebuilt from original poses and indices; distinct complete visible previews'],
    remaining=['original material/texture resolution and render effects','actor/world transform and native instance lifecycle',
               'live runtime animation/render frame integration','remaining mesh tail fields','Actor/Script/game loop','final Android verification'])
(root/'analysis/reports/skeletal-draw-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
p=root/'analysis/evidence.json';e=json.loads(p.read_text(encoding='utf-8-sig'));e['rust_tests']=tests;e['skeletal_draw_validation']=result
p.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-08: Drei Meshdarstellungs-Aufgaben abgeschlossen: skinnierte Abschnitte/Indexdaten, starre Abschnitte '
      'einschliesslich LODs ohne Skinning-Befehle mit originalem inversen Referenz-/Poseprodukt und Diagnose-Renderer-Anbindung. '
      '130 Meshes / 426 LODs: 315754 skinnierte und 252010 starre Dreiecke unabhaengig geprueft. Vier sichtbare '
      'Clone-Commando-Animationsframes mit festen Kameraeinstellungen, Originalpositionen und UVs; Materialslots '
      'nur mit Diagnose-Schachbrett, Originalmaterialien/Effekte offen. 484 Tests, Clippy und Format bestanden. '
      'Keine laufende native Spielintegration behauptet; Android zum Schluss. Details '
      'D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_DRAW.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for target in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Drei Meshdarstellungs-Aufgaben abgeschlossen: skinnierte Abschnitte/Indexdaten' not in target.read_text(encoding='utf-8-sig'):
        with target.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(dict(stats)))
