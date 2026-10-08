"""Record prepared notify/end branches without claiming a complete LOD tick."""
from pathlib import Path
import hashlib
import json
import re

root = Path(__file__).resolve().parents[1]
log = (root / 'analysis/reports/animation-tick-transitions-tests.log').read_text()
count = sum(map(int, re.findall(r'test result: ok\. (\d+) passed', log)))
assert count == 407 and 'test result: FAILED' not in log
assert len(re.findall(r'^test skeletal_animation_tick::transition_tests::.* \.\.\. ok$', log, re.M)) == 13
asm = (root / 'analysis/decompiled/animation-update-entry.asm').read_text()
for instruction in ['10450540 JBE 0x1045055f', '1045054a JC 0x1045055f',
                    '10450558 JBE 0x1045055f', '10450595 MOV dword ptr [EBX + 0x1c],EDX',
                    '104505c0 CALL dword ptr [EDX + 0x78]', '104505d1 JL 0x10450634',
                    '1045064e JA 0x1045076e', '10450667 JBE 0x10450673',
                    '10450737 CALL dword ptr [EAX + 0xb0]',
                    '10450745 MOVSS dword ptr [EBX + 0x18],XMM0',
                    '10450763 CALL dword ptr [EDX + 0x100]']:
    assert instruction in asm, instruction
sources = ['crates/rc-package/src/skeletal_animation_tick.rs',
           'crates/rc-package/src/skeletal_animation_tick_transition_tests.rs',
           'analysis/decompiled/animation-update-entry.asm',
           'analysis/decompiled/animation-update-entry.c',
           'docs/SKELETAL_TICK_TRANSITIONS.md', 'scripts/Record-TickTransitions.py']
result = {
    'date': '2026-10-08', 'rust_tests': count, 'additional_focused_tests': 13,
    'scope': 'prepared notify dispatch with mutable channel list and fixed-channel end/loop host transitions; full LOD tick remains external',
    'addresses': ['104504dc..10450640', '10450645..10450769'],
    'source_sha256': {s: hashlib.sha256((root / s).read_bytes()).hexdigest() for s in sources},
    'checks': ['cargo test --workspace: 407 passed',
               'cargo clippy --workspace --all-targets -- -D warnings',
               'cargo fmt --all -- --check',
               '13 transition tests include one composed multi-notify and loop frame progression',
               'exported ASM selection, dispatch, early-return and callback-order anchors checked'],
    'remaining': ['actual notify/Actor/Script object binding', 'sequence resolution and random jitter',
                  'production iteration loop and replication', 'LOD lifecycle and cache invalidation',
                  'end callback channel storage/lifetime effects', 'actual runtime tick connection',
                  'final Android verification'],
}
(root / 'analysis/reports/animation-tick-transitions-validation.json').write_text(
    json.dumps(result, indent=2) + '\n', encoding='utf-8')
path = root / 'analysis/evidence.json'
evidence = json.loads(path.read_text(encoding='utf-8-sig'))
evidence['rust_tests'] = count
evidence['animation_tick_transitions_validation'] = result
path.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
note = ('2026-10-08: Notify- und Enduebergaenge des Animationsticks rekonstruiert: '
        'zeitlich naechster Notify mit stabilen Gleichstaenden, Restzeit, Nullobjekten, '
        'Kanallistenersetzung/-entfernung sowie End-/Schleifenzweige mit Clear/Stop/AnimEnd-Reihenfolge. '
        '13 neue Tests einschliesslich zusammengesetzter Frame-/Notify-/Schleifenschritte; '
        '407 Workspace-Tests, Clippy und Format bestanden. Vorbereitete Host-Grenzen, '
        'noch kein vollstaendiger LOD-Tick oder reale Actor-/Script-Anbindung. Android zum Schluss. '
        'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_TICK_TRANSITIONS.md.')
wiki = Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for target in [root / 'README.md', wiki / 'index.md', wiki / 'architecture/porting.md', wiki / 'log.md']:
    if 'Notify- und Enduebergaenge des Animationsticks rekonstruiert' not in target.read_text(encoding='utf-8-sig'):
        with target.open('a', encoding='utf-8') as stream:
            stream.write('\n\n' + note + '\n')
print(json.dumps({'rust_tests': count, 'additional_focused_tests': 13}))
