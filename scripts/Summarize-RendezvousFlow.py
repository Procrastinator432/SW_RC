"""Bind prepared objective4 evidence; explicitly retain the failed route acceptance."""
import hashlib
import json
import wave
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission5-transition'
GAME = Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData')

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def read(name):
    return json.loads((OUT / name).read_text(encoding='utf-8-sig'))

prior = json.loads((ROOT / 'analysis/mission4-complete/validation-manifest.json').read_text())
for name, row in prior['evidence'].items():
    path = ROOT / name
    assert path.stat().st_size == row['bytes'] and sha(path) == row['sha256'], name
originals = prior['originals'].copy()
for name in ['Sounds/geo_sound.uax', 'Sounds/GEO_Voice.uax', 'Sounds/Character_Voice.uax']:
    path = GAME / name
    originals[str(path)] = sha(path)
for name, digest in originals.items():
    assert sha(Path(name)) == digest, name
bindings = read('rendezvous-bindings.json')
flow = read('flow-validation.json')
route = read('route/route-validation.json')
assert len(read('original-map-inventory.json')) == 1920
assert bindings['passed'] and len(bindings['actions']) == 40
assert len(bindings['sound_bank']) == 8
assert flow['passed'] and [r['hz'] for r in flow['runs']] == [20, 60, 144]
for run in flow['runs']:
    assert run['sounds'] == 8
    assert all(run['state'][flag] for flag in ['squad_joined', 'objective_completed', 'charge_enabled'])
for source, row in bindings['sound_bank'].items():
    path = OUT / 'sounds' / (source.rsplit('.', 1)[1] + '.wav')
    assert path.stat().st_size == row['bytes']
    with wave.open(str(path)) as pcm:
        assert abs(pcm.getnframes() / pcm.getframerate() - row['seconds']) < 1e-8
assert not route['passed'] and route['anchors'] == 99 and route['verified_segments'][0]['target'] == 802
assert route['reason'].endswith('957')
sources = [ROOT / 'crates/rc-game/src/lib.rs', ROOT / 'crates/rc-game/src/rendezvous.rs', ROOT / 'docs/RENDEZVOUS_FLOW.md', Path(__file__)]
sources += list((ROOT / 'crates/rc-inspect/src/bin').glob('rc-rendezvous-*.rs'))
sources += [ROOT / 'crates/rc-inspect/src/bin/rc-next-mission-audit.rs']
record = lambda p: {'bytes': p.stat().st_size, 'sha256': sha(p)}
evidence = {p.relative_to(ROOT).as_posix(): record(p) for p in sorted(OUT.rglob('*')) if p.is_file() and p.name != 'validation-manifest.json'}
manifest = {
    'script_host_passed': True,
    'playable_milestone_complete': False,
    'route_accepted': False,
    'current_playable_version': '0.23.0',
    'scope': 'Prepared selected objective4 event host and original audio. No new Game/companion/Android integration. Walking-route diagnostic fails at957.',
    'previous_delivery_evidence_unchanged': True,
    'evidence': evidence,
    'sources': {p.relative_to(ROOT).as_posix(): record(p) for p in sources},
    'originals': originals,
}
(OUT / 'validation-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8', newline='\n')
print(f'Prepared host verified: {len(evidence)} evidence files, {len(originals)} originals. Playable route remains unaccepted.')
