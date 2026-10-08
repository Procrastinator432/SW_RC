"""Record bounded UpdateAnimation arithmetic evidence; no full-tick claim."""
from pathlib import Path
import hashlib
import json
import re

root = Path(__file__).resolve().parents[1]
log = (root / 'analysis/reports/animation-tick-tests.log').read_text()
count = sum(map(int, re.findall(r'test result: ok\. (\d+) passed', log)))
assert count == 394 and 'test result: FAILED' not in log
sources = [
    'crates/rc-package/src/skeletal_animation_tick.rs',
    'analysis/decompiled/animation-update-entry.c',
    'analysis/decompiled/animation-update-entry.asm',
    'docs/SKELETAL_ANIMATION_TICK.md',
    'scripts/Record-AnimationTick.py',
]
result = {
    'date': '2026-10-08',
    'rust_tests': count,
    'focused_tests': 8,
    'scope': 'UpdateAnimation arithmetic blocks and host-supplied LOD wrapper; full tick remains external',
    'addresses': ['10450345..104503bd', '104504ad..104504d7', '105001f0..10500242'],
    'source_sha256': {s: hashlib.sha256((root / s).read_bytes()).hexdigest() for s in sources},
    'checks': ['cargo test --workspace: 394 passed',
               'cargo clippy --workspace --all-targets -- -D warnings',
               'cargo fmt --all -- --check'],
    'remaining': ['LOD lifecycle and cache invalidation', 'random jitter',
                  'notify and AnimEnd callbacks', 'loop/end and replication',
                  'connection to actual runtime tick', 'final Android verification'],
}
(root / 'analysis/reports/animation-tick-validation.json').write_text(
    json.dumps(result, indent=2) + '\n', encoding='utf-8')
path = root / 'analysis/evidence.json'
evidence = json.loads(path.read_text(encoding='utf-8-sig'))
evidence['rust_tests'] = count
evidence['animation_tick_arithmetic_validation'] = result
path.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
note = ('2026-10-08: UpdateAnimation-Rechenbloecke rekonstruiert: Kanalblend/gewicht, '
        'Framefortschreibung mit bestehendem Jitter und Director-Zeitbudget nach externem LOD-Host. '
        'SSE-NaN-Zweige, negative Zeit und Host-Mutationsreihenfolge geprueft; 394 Tests, Clippy und Format bestanden. '
        'Vollstaendiger Tick mit Notifies, Schleifen, Actor-Rueckrufen und Cacheinvalidierung noch offen; '
        'Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_ANIMATION_TICK.md.')
wiki = Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for target in [root / 'README.md', wiki / 'index.md', wiki / 'architecture/porting.md', wiki / 'log.md']:
    if 'UpdateAnimation-Rechenbloecke rekonstruiert' not in target.read_text(encoding='utf-8-sig'):
        with target.open('a', encoding='utf-8') as stream:
            stream.write('\n\n' + note + '\n')
print(json.dumps({'rust_tests': count, 'focused_tests': 8}))
