"""Validate and hash the bounded M3 Geonosian decision evidence."""
from pathlib import Path
import hashlib
import json
import re

root = Path(__file__).resolve().parent.parent
out = root / 'analysis/milestone3b'
load = lambda name: json.loads((out / name).read_text(encoding='utf-8'))
assets = load('geonosian-assets.json')
oracle = load('attack-oracle.json')
validation = load('attack-validation.json')
assert validation['passed'] and validation['cases'] == len(oracle['cases']) == 9408
assert validation['provenance'] == oracle['provenance']
value = lambda cls, prop: assets['classes'][cls][prop]['Ok']['value']['value']
assert value('ctcharacters.GeonosianWarrior', 'RequiredEquipment[0]') == 'CTInventory.ForcePike'
assert value('ctinventory.ForcePike', 'MeleeRange') == 350
assert value('ctinventory.ForcePike', 'MeleeDamage') == 125
assert len(assets['map_actors']) == 2
inventory = {a['actor']: a['inventory']['RequiredEquipment[0]']['Ok']['value']['value'] for a in assets['map_actors']}
assert inventory == {'GeonosianWarrior1': 'CTInventory.ForcePike', 'GeonosianWarriorNoWeap1': ''}
notifies = {n['index']: n for n in assets['notifies']}
melee = {}
for s in assets['sequences']:
    hits = [n for n in s['notifies'] if notifies[n['object_index']]['class'] == 'Engine.AnimNotify_MeleeHit']
    if hits:
        assert len(hits) == 1
        melee[s['name']['name']] = hits[0]['time_bits']
assert melee == {'MeleeAttack': 1050673152, 'MeleeAttack2': 1053619989, 'Lunge': 1056534423, 'Lunge2': 1059532549}
test_log = (out / 'workspace-tests.log').read_text(encoding='utf-8')
counts = re.findall(r'test result: ok\. (\d+) passed; 0 failed', test_log)
assert sum(map(int, counts)) == 947, counts
assert 'Finished' in (out / 'clippy.log').read_text(encoding='utf-8')

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

source_paths = [
    'crates/rc-package/src/geonosian_attack.rs', 'crates/rc-package/src/lib.rs',
    'crates/rc-inspect/src/bin/rc-geonosian-assets.rs', 'crates/rc-inspect/src/bin/rc-geonosian-check.rs',
    'scripts/Record-GeonosianAttack.py', 'scripts/Summarize-GeonosianAttack.py',
    'docs/GEONOSIAN_ATTACK.md', 'docs/COMBAT_SCENE.md', 'docs/CONTINUATION_PLAN.md',
]
evidence_paths = [p for p in out.iterdir() if p.is_file() and p.name != 'validation-manifest.json']
evidence_paths += [root / f'analysis/decompiled/{name}' for name in [
    'geonosian-attack.c', 'geonosian-attack.asm', 'geonosian-melee-goal.c',
    'geonosian-essentials.c', 'geonosian-melee-notify.c',
]]
game = Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData')
originals = [game / name for name in ['System/ctgame.dll', 'System/engine.dll', 'System/ctgame.u',
             'System/engine.u', 'System/ctcharacters.u', 'System/ctinventory.u',
             'Properties/properties.u', 'Animations/geonosian.ukx', 'Maps/geo_01a.ctm']]
assert digest(game / 'System/ctgame.dll') == oracle['provenance']['binary_sha256']
assert digest(root / 'analysis/decompiled/geonosian-attack.asm') == oracle['provenance']['assembly_sha256']
previous = root / 'analysis/milestone3'
assert digest(previous / 'republic-commando-0.11.0.apk') == '13ac4d57f7ee37d364c8a7f5e62e2b2c74ba15f6a92d4ce38fa056f90f2549b7'
assert digest(previous / 'geo_01a-combat.rcplay') == '7e01e75d61fcb5609e1894dc064aabc4b56c33672c79fb2ddd5e3977c9cca161'
manifest = dict(
    scope='Prepared native decision port; no playable scene integration or Android change',
    checks=dict(native_branch_cases=9408, workspace_library_tests=947,
                clippy=True, attack_clip_hit_mappings=4, original_map_inventory_overrides=True),
    sources={name: digest(root / name) for name in source_paths},
    evidence={str(p.relative_to(root)): digest(p) for p in evidence_paths},
    originals={str(p): digest(p) for p in originals},
    previous_playable='0.11.0; pinned APK and scene hashes unchanged',
    next_step='Melee goal, attack animation/notify execution and damage rules before scene integration',
)
(out / 'validation-manifest.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
print('Verified 9408 native branch cases, 947 library tests, inventory and four melee-hit mappings')
