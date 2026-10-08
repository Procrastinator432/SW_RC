"""Independent archive point-field reader and native SSE skin palette oracle."""
from pathlib import Path
import collections
import hashlib
import json
import re

root = Path(__file__).resolve().parents[1]
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
source = root / 'scripts/Record-SkeletalLinkups.py'
ns = {'__file__': str(source)}
exec(compile(source.read_text().split("for o in report['objects']:")[0], str(source), 'exec'), ns)
Reader, package, links, f, bits, value = [ns[k] for k in ('Reader', 'package', 'report', 'f', 'bits', 'value')]
path = root / 'analysis/reports/skeletal-rigid-skin.json'
report = json.loads(path.read_text())
def checked(name, validation):
    p = root / 'analysis/reports' / name
    assert sha(p) == json.loads((root / 'analysis/reports' / validation).read_text())['report_sha256']
    return json.loads(p.read_text())
poses = checked('loaded-full-animation-ticks.json', 'loaded-full-animation-ticks-validation.json')
inverses = checked('reference-caches.json', 'reference-caches-validation.json')
inverse_map = {o['source_index']: o['inverse'] for o in inverses['objects']}
asm = (root / 'analysis/decompiled/skeletal-skin-vertices.asm').read_text()
def offset(op):
    numbers = re.findall(r'-?0x[0-9a-f]+', op)
    n = int(numbers[-1], 16) if numbers else 0
    return n - 0x100000000 if n >= 0x80000000 else n
registers, expressions = {}, {}
def operand(op):
    if op.startswith('XMM'): return registers[op]
    if 'EAX' in op: return ('pose', offset(op) // 4)
    assert 'ECX' in op, op
    return ('inverse', offset(op) // 4)
transpose = {}
for line in asm.splitlines():
    if not line or line.startswith('#'): continue
    address, instruction = line.split(' ', 1)
    address = int(address, 16)
    opcode, _, args = instruction.partition(' ')
    if 0x1050bd96 <= address <= 0x1050c14f and opcode in ('MOVSS', 'MOVAPS', 'ADDSS', 'MULSS'):
        dst, src = args.split(',', 1)
        expr = operand(src)
        if dst.startswith('XMM'):
            registers[dst] = expr if opcode in ('MOVSS', 'MOVAPS') else (opcode, registers[dst], expr)
        else:
            assert opcode == 'MOVSS' and 'EBP' in dst
            expressions[(offset(dst) + 0xd8) // 4] = expr
    if 0x1050c16f <= address <= 0x1050c279 and opcode == 'MOVSS':
        dst, src = args.split(',', 1)
        if dst.startswith('XMM'):
            assert 'EBP' in src
            registers[dst] = (offset(src) + 0x118) // 4
        else: transpose[(offset(dst) + 0x158) // 4] = registers[src]
assert set(expressions) == set(range(16))
assert transpose == {i: (i % 4) * 4 + i // 4 for i in range(16)}
def evaluate(expr, inverse, pose):
    if expr[0] == 'inverse': return value(inverse[expr[1]])
    if expr[0] == 'pose': return value(pose[expr[1]])
    a, b = evaluate(expr[1], inverse, pose), evaluate(expr[2], inverse, pose)
    return f(a * b) if expr[0] == 'MULSS' else f(a + b)
def palette(inverse, pose):
    return [bits(evaluate(expressions[transpose[i]], inverse, pose)) for i in range(16)]
def rigid(vertex, matrix):
    p = list(map(value, vertex['position']))
    m = list(map(value, matrix))
    word = vertex['packed_normal']
    n = [float(((word >> (i * 10)) & 1023) - 511) for i in range(3)]
    pos = [f(f(f(p[1]*m[1])+f(p[2]*m[2]))+f(m[0]*p[0])),
           f(f(f(m[6]*p[2])+f(m[5]*p[1]))+f(m[4]*p[0])),
           f(f(f(m[10]*p[2])+f(m[9]*p[1]))+f(p[0]*m[8]))]
    pos = [f(pos[i]+m[i*4+3]) for i in range(3)]
    normal = [f(f(f(n[0]*m[0])+f(m[2]*n[2]))+f(n[1]*m[1])),
              f(f(f(m[5]*n[1])+f(m[6]*n[2]))+f(n[0]*m[4])),
              f(f(f(m[9]*n[1])+f(m[10]*n[2]))+f(n[0]*m[8]))]
    return {'position': list(map(bits, pos)), 'normal': list(map(bits, normal))}
stats = collections.Counter()
assert len(report['objects']) == len(poses['objects']) == 130
for obj, prior in zip(report['objects'], poses['objects']):
    index = obj['source_index']
    assert index == prior['source_index']
    original = links['objects'][index]
    data, (version, licensee, names, exports, object_path) = package(original['file'])
    export = exports[original['export_index'] - 1]
    r = Reader(data[export[5]:export[5]+export[4]])
    assert names[r.index()] == 'None'
    r.take(41); assert r.i32() == 8; r.u32()
    r.take(r.count()*4)
    for _ in range(r.count()): object_path(r.index())
    r.take(36)
    for stride in [2, 8, 2, 10, 8]: r.take(r.count()*stride)
    r.take(28); object_path(r.index()); r.take(52+4+16)
    assert r.pos == obj['points_offset']
    points = [[r.u32(), r.u32(), r.u32()] for _ in range(r.count())]
    assert points == obj['points'] and r.pos == obj['bones_offset'] == original['prefix']['bones_offset']
    assert not points  # Observed original legacy field is empty in all supported meshes.
    stats['empty_legacy_point_arrays'] += 1
    stats['original_legacy_points'] += len(points)
    source_points = [b['position'] for b in original['prefix']['bones']]
    inverse = inverse_map[index]
    assert len(obj['runs']) == len(prior['runs'])
    for run, oldrun in zip(obj['runs'], prior['runs']):
        assert (run['animation'], run['entry']) == (oldrun['animation'], oldrun['entry'])
        for case, old in zip(run['cases'], oldrun['cases']):
            assert case['step'] == old['step'] and case['input_source'] == 'ReferenceBonePosition'
            expected = [palette(inv, pose) for inv, pose in zip(inverse, old['full']['matrices'])]
            assert case['palette'] == expected and len(expected) == len(inverse)
            count = min(len(source_points), 8)
            indices = [j*(len(source_points)-1)//(count-1) if count > 1 else 0 for j in range(count)]
            commands = [(i % len(inverse))*6 for i in indices] + [0xffffffff]
            inputs = [{'position': source_points[i], 'packed_normal': [0,0x3fffffff,0x1ff7fdff,0x20000000][j%4]} for j, i in enumerate(indices)]
            assert case['source_indices'] == indices and case['commands'] == commands and case['input'] == inputs
            assert case['written'] == count
            assert case['output'] == [rigid(v, expected[(cmd & 4095)//6]) for v, cmd in zip(inputs, commands)]
            stats['palette_matrices'] += len(expected)
            stats['diagnostic_rigid_vertices'] += count
            stats['poses'] += 1
for case in report['synthetic']:
    expected = palette(case['inverse'], case['pose'])
    assert case['palette'] == expected
    assert case['output'] == rigid(case['vertex'], expected)
    stats['synthetic_matrix_vertex_cases'] += 1
assert (stats['palette_matrices'], stats['diagnostic_rigid_vertices'], stats['poses'], stats['synthetic_matrix_vertex_cases']) == (29912, 6664, 900, 64)
assert report['points'] == 0 and report['palette_matrices'] == 29912 and report['rigid_vertices'] == 6664
for anchor in ['105140b7 LEA ECX,[EBX + 0x1d0]', '105140bd CALL 0x10373800',
               '1050c2ac CMP EDI,-0x1', '1050c2ba AND EAX,0xf0000000',
               '1050c2c5 AND EDI,0xfff', '1050c2d2 SHR EDX,0x2',
               '1050c2f7 SUB EDI,0x1ff', '1050c445 ADD ECX,0x10', '1050c44d ADD ESI,0x4']:
    assert anchor in asm, anchor
log = (root/'analysis/reports/skeletal-skin-tests.log').read_text()
assert sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log))) == 446 and 'test result: FAILED' not in log
sources = ['crates/rc-package/src/skeletal_mesh.rs', 'crates/rc-package/src/skeletal_skin.rs',
           'crates/rc-package/src/skeletal_skin_tests.rs', 'crates/rc-package/src/skeletal_skin_product.rs',
           'crates/rc-inspect/src/bin/rc-skin-rigid-check.rs', 'scripts/Generate-SkinProduct.py',
           'scripts/Record-RigidSkin.py', 'analysis/decompiled/skeletal-skin-vertices.c',
           'analysis/decompiled/skeletal-skin-vertices.asm', 'docs/SKELETAL_RIGID_SKIN.md']
result = {'date':'2026-10-08','rust_tests':446,'completed_tasks':[
    'legacy mesh point field decoded and independently checked in 130 original exports (all empty)',
    'native inverse-reference/pose product and palette transpose reconstructed',
    'top-nibble-zero rigid skin command, packed normals and prepared stream reconstructed'],
    'counts':dict(stats),'scope':report['scope'],'source_sha256':{s:sha(root/s) for s in sources},
    'report_sha256':sha(path),'pose_report_sha256':sha(root/'analysis/reports/loaded-full-animation-ticks.json'),
    'inverse_report_sha256':sha(root/'analysis/reports/reference-caches.json'),'original_package_sha256':ns['files'],
    'checks':['cargo test --workspace: 446 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check',
              '130 original legacy point arrays independently read from package bytes; all empty',
              '29912 palette matrices bit-exact against independent original SSE-expression interpreter',
              '6664 diagnostic rigid vertices and 64 general matrix/vertex cases bit-exact under f32 expression oracle',
              '14 new tests for raw archive words/truncation, palette order/partial writes, normal bias, command selection and stream errors'],
    'remaining':['actual LOD vertex/influence/normal/command decoding','weighted and cached-copy skin commands',
                 'mesh triangles/materials/rendering','Actor/Script/frame loop','final Android verification']}
(root/'analysis/reports/skeletal-rigid-skin-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json'; evidence=json.loads(path.read_text(encoding='utf-8-sig'))
evidence['rust_tests']=446; evidence['skeletal_rigid_skin_validation']=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-08: Drei Skinning-Aufgaben abgeschlossen: altes Meshpunktfeld gelesen (130 Originalarchive, alle leer), '
      'native Skinning-Palette aus Referenzinverse und Pose samt Transposition rekonstruiert, Einzelknochen-Kernel '
      'mit gepackten Normalen und vorbereiteter Befehlsfolge umgesetzt. 29912 Palettenmatrizen, 6664 explizit '
      'diagnostische Vertices aus Referenzknochenpositionen und 64 allgemeine Faelle unabhaengig bitgenau geprueft. '
      '446 Tests, Clippy und Format bestanden. Echte LOD-Vertices/Einfluesse/Normalen/Befehle noch offen; '
      'keine echte Meshverformung oder Darstellung behauptet. Android zum Schluss. Details '
      'D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_RIGID_SKIN.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for target in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Drei Skinning-Aufgaben abgeschlossen: altes Meshpunktfeld gelesen' not in target.read_text(encoding='utf-8-sig'):
        with target.open('a',encoding='utf-8') as stream: stream.write('\n\n'+note+'\n')
print(json.dumps(dict(stats)))
