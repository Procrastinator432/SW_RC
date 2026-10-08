"""Record prepared LOD control flow and constants from the original PE image."""
from pathlib import Path
import hashlib
import json
import re
import struct

root = Path(__file__).resolve().parents[1]
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
log = (root / 'analysis/reports/lod-animation-tick-tests.log').read_text()
count = sum(map(int, re.findall(r'test result: ok\. (\d+) passed', log)))
assert count == 426 and 'test result: FAILED' not in log
assert len(re.findall(r'^test skeletal_lod_tick::tests::.* \.\.\. ok$', log, re.M)) == 19
binary = Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\engine.dll')
data = binary.read_bytes()
pe = struct.unpack_from('<I', data, 60)[0]
sections = struct.unpack_from('<H', data, pe + 6)[0]
optional = struct.unpack_from('<H', data, pe + 20)[0]
base = struct.unpack_from('<I', data, pe + 52)[0]
def word(address):
    rva = address - base
    for i in range(sections):
        _, va, raw, pointer = struct.unpack_from('<IIII', data, pe + 24 + optional + i * 40 + 8)
        if va <= rva and rva + 4 <= va + raw:
            return struct.unpack_from('<I', data, pointer + rva - va)[0]
    raise AssertionError(hex(address))
constants = {hex(a): word(a) for a in [0x10672164, 0x10668e18, 0x10653578, 0x1065efcc]}
assert constants == {'0x10672164': 0x38800100, '0x10668e18': 0x41200000,
                     '0x10653578': 0x3f800000, '0x1065efcc': 0}
asm = (root / 'analysis/decompiled/animation-update-entry.asm').read_text()
for anchor in ['10450317 JNP 0x10450796', '10450329 JNZ 0x10450796',
               '104503e9 JNP 0x1045076e', '104503f7 JBE 0x1045076e',
               '10450401 CMP EAX,0x4', '10450445 JZ 0x104503d1',
               '10450462 MULSS XMM0,dword ptr [0x10672164]',
               '1045047c MULSS XMM0,dword ptr [0x10668e18]',
               '1045079a MOV byte ptr [ESI + 0x61],0x0',
               '1045079e CALL dword ptr [EAX + 0xac]',
               '104507ae CALL dword ptr [EDX]',
               '104507b6 CALL dword ptr [EAX + 0xa8]']:
    assert anchor in asm, anchor
sources = ['crates/rc-package/src/skeletal_lod_tick.rs',
           'crates/rc-package/src/skeletal_lod_tick_tests.rs',
           'crates/rc-package/src/skeletal_animation_tick.rs',
           'crates/rc-package/src/skeletal_sequence.rs',
           'analysis/decompiled/animation-update-entry.asm',
           'docs/SKELETAL_LOD_TICK.md', 'scripts/Record-LodAnimationTick.py']
result = {
    'date': '2026-10-08', 'rust_tests': count, 'additional_focused_tests': 19,
    'scope': 'prepared LOD tick with four-attempt channel loop, supplied sequence/callback/lifecycle hosts, jitter and common cache invalidation',
    'dll_constants': constants, 'binary_sha256': {str(binary): sha(binary)},
    'source_sha256': {s: sha(root / s) for s in sources},
    'checks': ['cargo test --workspace: 426 passed',
               'cargo clippy --workspace --all-targets -- -D warnings',
               'cargo fmt --all -- --check',
               '19 prepared tick tests cover sequence lookup, jitter, notify mutation, loop cap, lifecycle, failure writes and Director wrapper composition',
               'four constants read from original engine.dll PE sections',
               'exported loop, jitter and completion instruction anchors checked'],
    'remaining': ['concrete loaded sequence and notify binding', 'actual Actor/Script callbacks',
                  'native random/lifecycle implementation and replication transport',
                  'end-callback channel storage lifetime effects', 'actual game tick/frame connection',
                  'final Android verification'],
}
(root / 'analysis/reports/lod-animation-tick-validation.json').write_text(
    json.dumps(result, indent=2) + '\n', encoding='utf-8')
path = root / 'analysis/evidence.json'
evidence = json.loads(path.read_text(encoding='utf-8-sig'))
evidence['rust_tests'] = count
evidence['lod_animation_tick_validation'] = result
path.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
note = ('2026-10-08: Vorbereiteter LOD-Animationstick verbunden: vier Sequenzversuche pro Kanal, '
        'Editor-/Cachelookup, Jitter mit Original-DLL-Konstanten, Notify-/End-/Schleifenuebergaenge, '
        'Replikationsgrenze und Cacheinvalidierung vor abschliessender Lifecycle-Abfrage. '
        'Notify-Kanalentfernung behaelt fruehen Ruecksprung; Director-Budget laeuft danach weiter. '
        '19 neue Tests, insgesamt 426; Clippy und Format bestanden. Konkrete Actor-/Script-/Objektbindung '
        'und Spieltick-Anbindung noch offen; Endcallbacks duerfen Kanalstorage im sicheren Hostmodell '
        'nicht verschieben. Android zum Schluss. Details '
        'D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_LOD_TICK.md.')
wiki = Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for target in [root / 'README.md', wiki / 'index.md', wiki / 'architecture/porting.md', wiki / 'log.md']:
    if 'Vorbereiteter LOD-Animationstick verbunden' not in target.read_text(encoding='utf-8-sig'):
        with target.open('a', encoding='utf-8') as stream:
            stream.write('\n\n' + note + '\n')
print(json.dumps({'rust_tests': count, 'additional_focused_tests': 19, 'dll_constants': constants}))
