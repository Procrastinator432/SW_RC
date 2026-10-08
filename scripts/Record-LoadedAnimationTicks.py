"""Independent f32 tick oracle followed by original archive root-track sampling."""
from pathlib import Path
import collections
import hashlib
import json
import math
import re

root = Path(__file__).resolve().parents[1]
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
source = root / 'scripts/Record-OriginalChannelPoses.py'
ns = {'__file__': str(source)}
exec(compile(source.read_text().split('max_rotation=max_position_relative=')[0], str(source), 'exec'), ns)
links, animations, tracks, f, bits, value, matrix, sample = [ns[k] for k in (
    'links', 'animations', 'tracks', 'f', 'bits', 'value', 'matrix', 'sample')]
path = root / 'analysis/reports/loaded-animation-ticks.json'
report = json.loads(path.read_text())
assert report['cases'] == 900 and len(report['objects']) == 130
stats = collections.Counter()
max_rotation = max_position = 0.0

def div(a, b):
    if b == 0:
        return math.nan if a == 0 else math.copysign(math.inf, a * math.copysign(1.0, b))
    return f(a / b)

def tick(words, metadata, delta, objects):
    words = words.copy()
    events = ['lock true', 'flags']
    blend = f(f(value(words[10]) * delta) + value(words[11]))
    words[11] = bits(blend if blend <= 1 else 1.0)
    target, weight, speed = [value(words[i]) for i in (4, 14, 5)]
    if target > weight:
        weight = f(f(speed * delta) + weight)
        words[14] = bits(weight if target >= weight else target)
    elif weight > target:
        weight = f(weight - f(speed * delta))
        words[14] = bits(weight if weight >= target else target)
    remaining = delta
    attempts = 0
    while words[0] and value(words[6]) != 0 and remaining > 0 and attempts < 4:
        attempts += 1
        old = value(words[7])
        amplitude = value(metadata['word_18'])
        if amplitude > 0:
            events.append('random 16384')
            j = f(f(f(f(f(f(16384.0 * value(0x38800100)) - 1.0) * amplitude) * remaining) * 10.0) + value(words[8]))
            lower = f(0.0 - amplitude)
            if lower >= j: j = lower
            if j >= amplitude: j = amplitude
            words[8] = bits(j)
            stats['jitter_updates'] += 1
        frame = f(f(f(f(div(f(value(words[8]) + 1.0), f(metadata['frames'])) * value(metadata['rate_bits'])) * remaining) * value(words[6])) + old)
        words[7] = bits(frame)
        selected = None
        distance = 0.0
        if metadata['notifies']:
            events.append('actor mesh')
            if not words[1] & 0xff00:
                for notify in metadata['notifies']:
                    time = value(notify['time_bits'])
                    if time > old and frame >= time:
                        candidate = f(time - old)
                        if selected is None or distance > candidate:
                            selected, distance = notify, candidate
        if selected is not None:
            time = value(selected['time_bits'])
            remaining = div(f(f(frame - time) * remaining), f(frame - old))
            words[7] = selected['time_bits']
            stats['notify_transitions'] += 1
            if selected['object_index']:
                events.append(f"notify {objects[selected['object_index']]}")
                stats['notify_callbacks'] += 1
            else: stats['null_notify_transitions'] += 1
            continue
        end = value(words[9])
        if end > frame: break
        # This diagnostic uses a looping channel, so no clear/play callback.
        if frame < 1.0: remaining = 0.0
        else:
            remaining = div(f(f(frame - 1.0) * remaining), f(frame - old))
            words[7] = 0
            stats['loop_wraps'] += 1
        if end > old and not words[1] & 0xff00:
            events.append(f'end {words[3]}')
            stats['anim_end_callbacks'] += 1
    events += ['replicate 0', 'flags', 'lock false']
    return words, events, attempts

for obj in report['objects']:
    original = links['objects'][obj['source_index']]
    bones = original['prefix']['bones']
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
        words[9] = bits(f(1.0 - div(1.0, f(metadata['frames']))))
        words[16] = len(bones)
        assert len(run['cases']) == 4
        for step, case in enumerate(run['cases']):
            delta = [0.125, 0.25, 0.5, 1.25][step]
            assert case['step'] == step and case['delta_bits'] == bits(delta) and case['editor'] == bool(step % 2)
            assert case['cache_before'] == (7 if step == 0 else 0)
            assert case['before'] == [{'words': words}]
            words, events, attempts = tick(words, metadata, delta, objects)
            assert case['after_tick'] == case['after_pose'] == [{'words': words}]
            assert case['tick_events'] == events
            assert case['tick'] == {'exit': 'Unlocked', 'attempts': attempts, 'replicated': 1}
            track = mapping['mapping'][0]
            sampled = bool(ts) and track >= 0
            expected_prep = {'buffers': {'local_resized': step == 0, 'matrices_resized': step == 0}, 'inverse_built': not built, 'linkups': 1}
            assert case['pose_result'] == {'preparation': expected_prep, 'sampled': int(sampled)}
            built = True
            events = ['Transform', {'Refresh': {'index': 0, 'result': {'Rebuilt': {'matched': sum(i >= 0 for i in mapping['mapping']), 'warning': not any(i >= 0 for i in mapping['mapping'])}} if step == 0 else 'SameLength'}},
                      {'Sequence': {'channel': 0, 'identity': 1}}]
            if ts:
                events += [{'Linkup': {'key': 7, 'index': 0}}]
            if sampled:
                frame = value(words[7])
                normalized = 0.0 if math.isnan(frame) or frame < 0 else min(frame, 1.0)
                time = f(metadata['frames'] * normalized)
                events += [{'RootSample': {'track': track, 'time_bits': bits(time)}}]
                q, p = sample(ts[track], time)
                actual_q = list(map(value, case['root']['rotation']))
                actual_p = list(map(value, case['root']['position']))
                qe = max(abs(a - b) for a, b in zip(q, actual_q))
                pe = max(abs(a - b) / max(1.0, abs(a)) for a, b in zip(p, actual_p))
                assert qe <= 2e-6 and pe <= 2e-5
                max_rotation, max_position = max(max_rotation, qe), max(max_position, pe)
                stats['sampled_roots'] += 1
            else:
                assert case['root'] == {'rotation': bones[0]['rotation'], 'position': bones[0]['position']}
                stats['reference_roots'] += 1
            assert case['pose_events'] == events
            assert case['matrix'] == matrix(case['root']['rotation'], case['root']['position'])
            assert case['bounds'] == {'minimum': [99] * 3, 'maximum': [100] * 3, 'sphere': [123] * 4, 'byte_60': 7, 'byte_61': 0, 'byte_179': 9}
            assert case['tails_zero'] is True
            stats['cases'] += 1
            stats['sequence_attempts'] += attempts
            stats['editor_ticks'] += bool(step % 2)
        stats['linkup_runs'] += 1
assert stats['cases'] == 900 and stats['linkup_runs'] == 225
log = (root / 'analysis/reports/runtime-tick-tests.log').read_text()
assert sum(map(int, re.findall(r'test result: ok\. (\d+) passed', log))) == 432 and 'test result: FAILED' not in log
sources = ['crates/rc-package/src/skeletal_runtime_tick.rs', 'crates/rc-package/src/skeletal_runtime_tick_tests.rs',
           'crates/rc-package/src/skeletal_runtime_hosts.rs', 'crates/rc-package/src/skeletal_lod_tick.rs',
           'crates/rc-inspect/src/bin/rc-loaded-tick-check.rs', 'scripts/Record-LoadedAnimationTicks.py',
           'docs/SKELETAL_RUNTIME_TICK.md']
result = {'date': '2026-10-08', 'rust_tests': 432, 'counts': dict(stats), 'scope': report['scope'],
          'max_rotation_error': max_rotation, 'max_position_relative_error': max_position,
          'report_sha256': sha(path), 'source_sha256': {s: sha(root / s) for s in sources},
          'checks': ['cargo test --workspace: 432 passed', 'cargo clippy --workspace --all-targets -- -D warnings',
                     'cargo fmt --all -- --check', '900 continuous loaded-sequence ticks independently recomputed with rounded f32 operations',
                     'notify resolutions and every external tick event independently checked against original metadata',
                     'root samples independently computed from original tracks; matrices bit-exact against ASM constructor oracle',
                     'six adapter tests cover metadata, null/missing objects, scoped first-match resolution and lookup'],
          'remaining': ['actual Actor/Script/notify execution and object registration', 'real RNG, lifecycle and replication',
                        'end callback storage relocation', 'full pose tick integration and game frame loop', 'final Android verification']}
(root / 'analysis/reports/loaded-animation-ticks-validation.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
path = root / 'analysis/evidence.json'
evidence = json.loads(path.read_text(encoding='utf-8-sig'))
evidence['rust_tests'] = 432
evidence['loaded_animation_ticks_validation'] = result
path.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
note = ('2026-10-08: Geladene Originalsequenzen mit vorbereitetem Animationstick und Root-Auswertung verbunden. '
        'Frames/Rate/Jitter/Notifyzeiten aus Archivdaten, explizite sequenzgebundene Notify-Objektaufloesung, '
        'Fehler statt stiller Null bei fehlenden Objekten. 900 fortlaufende Tick-/Root-Faelle in 225 Linkuplaeufen '
        'unabhaengig aus Originalmetadaten und Tracks geprueft. 432 Tests, Clippy und Format bestanden. '
        'Identitaeten/Loopkanal/Zufall/Rueckrufe diagnostisch; reale Actor-/Script-/Spielschleife und Vollpose-Anbindung offen. '
        'Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_RUNTIME_TICK.md.')
wiki = Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for target in [root / 'README.md', wiki / 'index.md', wiki / 'architecture/porting.md', wiki / 'log.md']:
    if 'Geladene Originalsequenzen mit vorbereitetem Animationstick und Root-Auswertung verbunden' not in target.read_text(encoding='utf-8-sig'):
        with target.open('a', encoding='utf-8') as stream: stream.write('\n\n' + note + '\n')
print(json.dumps({'counts': dict(stats), 'max_rotation_error': max_rotation, 'max_position_relative_error': max_position}))
