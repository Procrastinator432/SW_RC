"""Independent persistent LOD archive, command boundary and original rigid vertex oracle."""
from pathlib import Path
import collections
import hashlib
import json
import re
import struct

root = Path(__file__).resolve().parents[1]
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
# Reuse the audited package reader and native SSE expression oracle definitions only.
source = root / 'scripts/Record-RigidSkin.py'
ns = {'__file__': str(source)}
exec(compile(source.read_text().split('stats = collections.Counter()')[0], str(source), 'exec'), ns)
Reader, package, links, poses, inverse_map, palette, rigid = [ns[k] for k in
    ('Reader', 'package', 'links', 'poses', 'inverse_map', 'palette', 'rigid')]
assert sha(root/'analysis/reports/original-skeletal-linkups.json') == json.loads(
    (root/'analysis/reports/original-skeletal-linkups-validation.json').read_text())['report_sha256']
path = root/'analysis/reports/skeletal-lods.json'
report = json.loads(path.read_text())
stats, kinds = collections.Counter(), collections.Counter()
pose_map = {o['source_index']:o for o in poses['objects']}

def words(r, n): return [r.u32() for _ in range(n)]
def shorts(r, n): return list(struct.unpack('<'+'H'*n, r.take(n*2)))
def array(r, read): return [read() for _ in range(r.count())]
def lazy(r, base, read):
    end = r.u32()
    items = array(r, read)
    assert end == base+r.pos
    return end, items

def decode_lod(r, base):
    d = {'offset':r.pos,'version':r.u32()}
    assert d['version'] == 1
    d['commands'] = array(r, lambda:r.u32())
    d['bind_vertices'] = array(r, lambda:dict(position=words(r,3),packed_normal=r.u32()))
    d['word_14'] = r.u32()
    d['sections'] = [array(r,lambda:shorts(r,9)) for _ in range(2)]
    i0 = array(r,lambda:shorts(r,1)[0]); w0 = r.u32()
    i1 = array(r,lambda:shorts(r,1)[0]); w1 = r.u32()
    d['indices'], d['index_words'] = [i0,i1], [w0,w1]
    d['stream_words'] = words(r,3)
    d['stream_vertices'] = array(r,lambda:words(r,8))
    offsets, ends = [], []
    for field, read in [
        ('influences',lambda:dict(weight=r.u32(),vertex_index=shorts(r,1)[0],bone_index=shorts(r,1)[0])),
        ('wedges',lambda:list(r.take(10))),
        ('faces',lambda:list(r.take(8))),
        ('points',lambda:words(r,3))]:
        offsets.append(r.pos)
        end, items = lazy(r,base,read)
        ends.append(end); d[field] = items
    d['lazy_offsets'], d['lazy_ends'] = offsets, ends
    d['tail_words'] = words(r,6)
    d['end_offset'] = r.pos
    return d

def inspect(commands, bind_count, bone_count):
    if not commands:
        assert bind_count == 0
        stats['lods_without_skin_commands'] += 1
        return None
    pc = bind = output = 0
    entries, histogram = [], [0]*16
    while commands[pc] != 0xffffffff:
        kind = commands[pc] >> 28
        histogram[kind] += 1
        if kind == 15:
            next_pc = pc+1
        else:
            # Native selector consumes one bind vertex, then 1..8 influence words.
            next_pc = pc+1+(kind & 7)
            assert bind < bind_count
            for word in commands[pc:next_pc]:
                assert (word & 4095)//6 < bone_count
            if kind == 0:
                entries.append(dict(command_index=pc,bind_index=bind,output_index=output))
            bind += 1
        pc = next_pc+2  # UV words belong to this output even for cache copies.
        assert pc < len(commands)
        output += 1
    assert pc+1 == len(commands) and bind == bind_count
    kinds.update({str(i):n for i,n in enumerate(histogram) if n})
    stats['skin_programs'] += 1
    stats['rigid_commands'] += len(entries)
    return dict(rigid=entries,consumed_bind=bind,outputs=output,command_words=pc+1,kinds=histogram)

for obj in report['objects']:
    index = obj['source_index']; original = links['objects'][index]
    data, (version, licensee, names, exports, _) = package(original['file'])
    assert 151 <= version <= 159 and licensee == 1
    export = exports[original['export_index']-1]
    payload = data[export[5]:export[5]+export[4]]
    r = Reader(payload, original['prefix']['end_offset'])
    mesh = dict(start_offset=r.pos,word_1bc=r.u32())
    mesh['material_links'] = array(r,lambda:[array(r,lambda:shorts(r,1)[0]),r.u32()])
    mesh['word_pairs'] = array(r,lambda:shorts(r,2))
    mesh['aliases'] = array(r,lambda:dict(names=[r.index(),r.index()],matrix=words(r,16)))
    for alias in mesh['aliases']:
        assert all(0 <= n < len(names) for n in alias['names'])
    mesh['word_1c4'] = r.u32()
    mesh['lods'] = array(r,lambda:decode_lod(r,export[5]))
    mesh['end_offset'],mesh['remaining_bytes'] = r.pos,len(payload)-r.pos
    assert mesh == obj['mesh'], original['object']
    programs = [inspect(l['commands'],len(l['bind_vertices']),len(original['prefix']['bones'])) for l in mesh['lods']]
    assert programs == obj['programs']
    stats['meshes'] += 1
    stats['aliases'] += len(mesh['aliases'])
    stats['lods'] += len(mesh['lods'])
    for lod in mesh['lods']:
        stats['command_words'] += len(lod['commands'])
        stats['bind_vertices'] += len(lod['bind_vertices'])
        stats['stream_vertices'] += len(lod['stream_vertices'])
        stats['indices'] += sum(map(len,lod['indices']))
        stats['sections'] += sum(map(len,lod['sections']))
        for field in ('influences','wedges','faces','points'):
            stats[field] += len(lod[field])
        stats['lazy_arrays'] += 4
    stats['outputs'] += sum(p['outputs'] for p in programs if p)
    prior = pose_map[index]
    assert len(obj['runs']) == len(prior['runs'])
    for run, oldrun in zip(obj['runs'],prior['runs']):
        assert (run['animation'],run['entry']) == (oldrun['animation'],oldrun['entry'])
        assert len(run['cases']) == len(oldrun['cases'])
        for case, old in zip(run['cases'],oldrun['cases']):
            assert case['step'] == old['step']
            transforms = [palette(inv,pose) for inv,pose in zip(inverse_map[index],old['full']['matrices'])]
            samples = []
            for li,(lod,program) in enumerate(zip(mesh['lods'],programs)):
                if program is None: continue
                count = min(8,len(program['rigid']))
                for j in range(count):
                    selected = j*(len(program['rigid'])-1)//(count-1) if count>1 else 0
                    entry = program['rigid'][selected]
                    command = lod['commands'][entry['command_index']]
                    vertex = lod['bind_vertices'][entry['bind_index']]
                    samples.append(dict(lod=li,rigid_index=selected,
                        output=rigid(vertex,transforms[(command & 4095)//6])))
            assert samples == case['samples']
            stats['original_rigid_samples'] += len(samples)
            stats['pose_cases'] += 1

assert (stats['meshes'],stats['lods'],stats['bind_vertices'],stats['outputs'],stats['original_rigid_samples']) == (130,426,186848,289085,25044)
assert [report[k] for k in ('lods','bind_vertices','outputs','rigid_samples')] == [426,186848,289085,25044]
asm = (root/'analysis/decompiled/skeletal-skin-vertices.asm').read_text()
for anchor in ['1050c2ac CMP EDI,-0x1','1050c2ba AND EAX,0xf0000000',
               '1050d113 MOV dword ptr [EBX + 0x18],EDX','1050d119 MOV dword ptr [EBX + 0x1c],EAX','1050d11f ADD ESI,0x8']:
    assert anchor in asm,anchor
log = (root/'analysis/reports/skeletal-lod-tests.log').read_text()
tests = sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)))
assert tests == 458 and 'test result: FAILED' not in log
sources = ['crates/rc-package/src/skeletal_lod.rs','crates/rc-inspect/src/bin/rc-skeletal-lod-check.rs',
           'scripts/Record-SkeletalLods.py','docs/SKELETAL_LOD_ARCHIVES.md',
           'analysis/decompiled/skeletal-lod-archive.c','analysis/decompiled/skeletal-lod-model.c',
           'analysis/decompiled/skeletal-lod-model.asm','analysis/decompiled/skeletal-lod-elements.c',
           'analysis/decompiled/skeletal-lod-streams.c','analysis/decompiled/skeletal-lod-fields.c',
           'analysis/decompiled/skeletal-lod-influence-alias.c','analysis/decompiled/skeletal-skin-vertices.asm']
result = dict(date='2026-10-08',rust_tests=tests,counts=dict(stats),command_kinds=dict(kinds),
    completed_tasks=['persistent original LOD model archive reader with exact serialized strides and lazy offsets',
                     'full influence/UV command boundaries and rigid input/output mapping',
                     'real LOD bind vertices, packed normals and bone commands checked under original full poses'],
    scope=report['scope'],report_sha256=sha(path),source_sha256={s:sha(root/s) for s in sources},
    pose_report_sha256=sha(root/'analysis/reports/loaded-full-animation-ticks.json'),
    inverse_report_sha256=sha(root/'analysis/reports/reference-caches.json'),
    original_package_sha256=ns['ns']['files'],
    checks=['cargo test --workspace: 458 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check',
            'all 426 original LOD archives independently decoded byte-for-byte including 1704 absolute lazy ends',
            'all nonempty skin programs exhaust command words and bind vertices with bounded bone indices',
            '25044 original rigid vertex results bit-exact against independent f32 oracle using native SSE palette expressions'],
    remaining=['weighted skin execution and cached-copy commands','complete native skin output/triangle/material rendering',
               'remaining mesh archive tail fields','Actor/Script/frame loop','final Android verification'])
(root/'analysis/reports/skeletal-lods-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
p = root/'analysis/evidence.json'; evidence = json.loads(p.read_text(encoding='utf-8-sig'))
evidence['rust_tests'] = tests; evidence['skeletal_lod_archives_validation'] = result
p.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note = ('2026-10-08: Drei LOD-Aufgaben abgeschlossen: persistente LOD-Archive gelesen (130 Meshes, 426 LODs, '
        '186848 Bind-Vertices), Befehlsgrenzen einschliesslich Einfluesse/UV/Cache-Kopien rekonstruiert '
        '(289085 Ausgaben; gewichtete und kopierte Vertices noch nicht ausgefuehrt), 25044 echte '
        'Einzelknochen-LOD-Vertices mit Originalpositionen, gepackten Normalen und Knochenbefehlen '
        'unter geprueften vollen Animationsposen unabhaengig bitgenau validiert. 458 Tests, Clippy und '
        'Format bestanden. Meshdarstellung und vollstaendiges gewichtetes Skinning offen; Android zum Schluss. '
        'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_LOD_ARCHIVES.md.')
wiki = Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for target in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Drei LOD-Aufgaben abgeschlossen: persistente LOD-Archive gelesen' not in target.read_text(encoding='utf-8-sig'):
        with target.open('a',encoding='utf-8') as stream: stream.write('\n\n'+note+'\n')
print(json.dumps(dict(stats)))
