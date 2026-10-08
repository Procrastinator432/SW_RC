"""Independent continuous tick -> full pose -> cached repeat integration oracle."""
from pathlib import Path
import collections
import hashlib
import json
import math
import re

root = Path(__file__).resolve().parents[1]
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
source = root / 'scripts/Record-LoadedAnimationTicks.py'
ns = {'__file__': str(source)}
# Definitions and original archive oracles only; no earlier evidence/wiki writes.
exec(compile(source.read_text().split("for obj in report['objects']:")[0], str(source), 'exec'), ns)
links, animations, tracks, f, bits, value, matrix, sample, tick = [ns[k] for k in (
    'links', 'animations', 'tracks', 'f', 'bits', 'value', 'matrix', 'sample', 'tick')]
compose = ns['ns']['compose']
path = root / 'analysis/reports/loaded-full-animation-ticks.json'
report = json.loads(path.read_text())
assert report['cases'] == 900 and report['mode'] == 'Full' and len(report['objects']) == 130
stats = collections.Counter()
max_rotation = max_position = 0.0
for obj in report['objects']:
    original = links['objects'][obj['source_index']]
    bones = original['prefix']['bones']
    move = 0 if bones[0]['name']['name'].lower() == 'move' else -1
    built = False
    assert len(obj['runs']) == len(original['mappings'])
    for run, mapping in zip(obj['runs'], original['mappings']):
        assert (run['animation'], run['entry'], run['sequence']) == (mapping['animation'], mapping['entry'], 0)
        animation = animations[run['animation']]
        metadata = animation['sequences'][0]['metadata']
        ts = tracks(animation, 0)
        indices = sorted(set(n['object_index'] for n in metadata['notifies'] if n['object_index']))
        objects = {index: i + 1 for i, index in enumerate(indices)}
        assert run['bindings'] == [{'index': i, 'identity': objects[i]} for i in indices]
        assert run['mapping'] == mapping['mapping']
        words = [0] * 18
        words[0], words[1], words[17] = 101, 1, 1
        words[6] = words[11] = words[14] = bits(1.0)
        words[9] = bits(f(1.0 - f(1.0 / f(metadata['frames']))))
        words[16] = len(bones)
        last_bounds = {'minimum': [99] * 3, 'maximum': [100] * 3, 'sphere': [123] * 4, 'byte_60': 7, 'byte_61': 7, 'byte_179': 9}
        assert len(run['cases']) == 4
        for step, case in enumerate(run['cases']):
            delta = [0.125, 0.25, 0.5, 1.25][step]
            assert case['step'] == step and case['delta_bits'] == bits(delta) and case['editor'] == bool(step % 2)
            assert case['before'] == [{'words': words}] and case['cache_before'] == last_bounds['byte_61']
            words, events, attempts = tick(words, metadata, delta, objects)
            assert case['after_tick'] == [{'words': words}]
            assert case['tick_events'] == events
            assert case['tick'] == {'exit': 'Unlocked', 'attempts': attempts, 'replicated': 1}
            details = case['full']
            before = {**last_bounds, 'byte_61': 0}
            assert details['bounds_before'] == before
            frame = value(words[7])
            normalized = 0.0 if math.isnan(frame) or frame < 0 else min(frame, 1.0)
            time = f(f(metadata['frames']) * normalized)
            output = []
            assert len(details['local']) == len(bones)
            for i, (bone, actual, track) in enumerate(zip(bones, details['local'], mapping['mapping'])):
                if not ts or track < 0:
                    assert actual == {'rotation': bone['rotation'], 'position': bone['position']}
                    stats['reference_bones'] += 1
                else:
                    q, p = sample(ts[track], time)
                    actual_q, actual_p = list(map(value, actual['rotation'])), list(map(value, actual['position']))
                    qe = max(abs(a - b) for a, b in zip(q, actual_q))
                    pe = max(abs(a - b) / max(1.0, abs(a)) for a, b in zip(p, actual_p))
                    assert qe <= 2e-6 and pe <= 2e-5, (obj['source_index'], step, i, qe, pe)
                    max_rotation, max_position = max(max_rotation, qe), max(max_position, pe)
                    stats['sampled_bones'] += 1
                m = matrix(actual['rotation'], actual['position'])
                if i > 0 and bone['word_34'] != move:
                    m = compose(m, output[bone['word_34']])
                output.append(m)
            assert details['matrices'] == output
            assert case['matrix'] == output[0] and case['root'] == details['local'][0]
            scratch = details['scratch']
            if ts:
                assert scratch == {'rotations': [b['rotation'] for b in details['local']], 'positions': [b['position'] for b in details['local']]}
            else:
                assert scratch == {'rotations': [[0] * 4 for _ in bones], 'positions': [[0] * 3 for _ in bones]}
            words[12], words[13] = words[7], words[11]
            assert case['after_pose'] == [{'words': words}]
            prep = {'buffers': {'local_resized': step == 0, 'matrices_resized': step == 0}, 'inverse_built': not built, 'linkups': 1}
            if not built: stats['inverse_builds'] += 1
            built = True
            channel_result = {'Channels': {'called': 1, 'applied': 1}} if ts else 'Reference'
            assert case['pose_result'] == {'preparation': prep, 'scratch_resized': step == 0, 'pose': {'channels': channel_result, 'directors': 0}}
            cached = {'preparation': {'buffers': {'local_resized': False, 'matrices_resized': False}, 'inverse_built': False, 'linkups': 1},
                      'scratch_resized': False, 'pose': {'channels': 'Cached', 'directors': 0}}
            assert details['cached'] == cached and details['cached_unchanged'] is True
            assert details['publication_calls'] == step + 1
            matched = sum(i >= 0 for i in mapping['mapping'])
            refresh = {'Rebuilt': {'matched': matched, 'warning': matched == 0}} if step == 0 else 'SameLength'
            expected_events = ['Transform', {'Refresh': {'index': 0, 'result': refresh}}, {'Sequence': {'channel': 0, 'identity': 1}}]
            if ts: expected_events += [{'Linkup': {'key': 7, 'index': 0}}]
            expected_events += ['Transform', {'Refresh': {'index': 0, 'result': 'SameLength'}}]
            assert case['pose_events'] == expected_events
            low, high = before['minimum'].copy(), before['maximum'].copy()
            for i, m in enumerate(output):
                point = m[12:15]
                if i == move + 1: low, high = point.copy(), point.copy()
                elif i > move + 1:
                    for axis, v in enumerate(point):
                        if value(low[axis]) > value(v): low[axis] = v
                        elif value(v) > value(high[axis]): high[axis] = v
            low = [f(f(value(v) * value(0x3f99999a)) - f(1.0 + p)) for v, p in zip(low, [.25, 1., 2.])]
            high = [f(f(p + 1.0) + f(value(v) * value(0x3f99999a))) for v, p in zip(high, [2., .5, 1.])]
            d = [f(y - x) for x, y in zip(low, high)]
            squared = f(f(f(d[0] * d[0]) + f(d[1] * d[1])) + f(d[2] * d[2]))
            if squared:
                seed = f(1.0 / f(math.sqrt(squared)))
                length = f(f(f(3.0 - f(f(seed * squared) * seed)) * f(seed * .5)) * squared)
            else: length = 0.0
            sphere = [bits(f(f(y + x) * .5)) for x, y in zip(low, high)] + [bits(f(length * .5))]
            published = {**before, 'minimum': list(map(bits, low)), 'maximum': list(map(bits, high)), 'sphere': sphere}
            assert details['published'] == published
            last_bounds = {**published, 'byte_60': 1, 'byte_61': 1, 'byte_179': 0}
            assert case['bounds'] == last_bounds
            stats['full_poses'] += 1
            stats['matrices'] += len(output)
            stats['cached_repeats'] += 1
        stats['linkup_runs'] += 1
stats.update(ns['stats'])
assert (stats['full_poses'], stats['matrices'], stats['cached_repeats'], stats['linkup_runs'], stats['inverse_builds']) == (900, 29912, 900, 225, 125)
log = (root / 'analysis/reports/loaded-full-tick-tests.log').read_text()
assert sum(map(int, re.findall(r'test result: ok\. (\d+) passed', log))) == 432 and 'test result: FAILED' not in log
sources = ['crates/rc-inspect/src/bin/rc-loaded-tick-check.rs', 'crates/rc-package/src/skeletal_runtime_tick.rs',
           'crates/rc-package/src/skeletal_animation_entry.rs', 'crates/rc-package/src/skeletal_runtime_hosts.rs',
           'scripts/Record-LoadedAnimationTicks.py', 'scripts/Record-LoadedFullAnimationTicks.py', 'docs/SKELETAL_LOADED_FULL_TICK.md']
result = {'date': '2026-10-08', 'rust_tests': 432, 'counts': dict(stats), 'scope': report['scope'],
          'max_rotation_error': max_rotation, 'max_position_relative_error': max_position,
          'report_sha256': sha(path), 'source_sha256': {s: sha(root / s) for s in sources},
          'checks': ['cargo test --workspace: 432 passed', 'cargo clippy --workspace --all-targets -- -D warnings', 'cargo fmt --all -- --check',
                     '900 continuous original-metadata ticks independently recomputed with f32 rounding',
                     '29912 local bone transforms independently sampled from original tracks or checked against reference fallback',
                     '29912 hierarchy matrices and 900 bounds/publication snapshots independently recomputed',
                     '900 cached repeats preserve arrays/scratch/channels/bounds with no extra sequence or publication calls',
                     'history updates, event order, mapping refresh and 125 mesh-owned inverse-cache builds checked'],
          'remaining': ['actual Actor/Script/notify execution and object registration', 'real RNG/lifecycle/replication',
                        'end callback storage relocation', 'actual game frame loop', 'skinning/rendering', 'final Android verification']}
(root / 'analysis/reports/loaded-full-animation-ticks-validation.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
path = root / 'analysis/evidence.json'
evidence = json.loads(path.read_text(encoding='utf-8-sig'))
evidence['rust_tests'] = 432
evidence['loaded_full_animation_ticks_validation'] = result
path.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
note = ('2026-10-08: Fortlaufenden Originalsequenz-Tick mit kompletter Skelettpose verbunden. '
        '900 Vollposen / 29912 Knochenmatrizen und 900 unveraenderte Cachewiederholungen unabhaengig '
        'aus Originaltracks, Tickmetadaten und Matrix-/Boundsformeln geprueft. Kanalhistorie, '
        'Cacheinvalidierung vor neuer Pose, Publikation vor Abschlussflags und 125 mesh-eigene '
        'Inversecache-Aufbauten bestaetigt. 432 Workspace-Tests, Clippy und Format bestanden. '
        'Diagnostische Szeneneingaben/Rueckrufhosts; echte Actor-/Script-Spielschleife, Skinning/Rendering '
        'und abschliessende Android-Pruefung offen. Details '
        'D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_LOADED_FULL_TICK.md.')
wiki = Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for target in [root / 'README.md', wiki / 'index.md', wiki / 'architecture/porting.md', wiki / 'log.md']:
    if 'Fortlaufenden Originalsequenz-Tick mit kompletter Skelettpose verbunden' not in target.read_text(encoding='utf-8-sig'):
        with target.open('a', encoding='utf-8') as stream: stream.write('\n\n' + note + '\n')
print(json.dumps({'counts': dict(stats), 'max_rotation_error': max_rotation, 'max_position_relative_error': max_position}))
