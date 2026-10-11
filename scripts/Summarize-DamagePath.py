"""Bind the bounded M3 damage reconstruction to reports and a source snapshot."""
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED
import hashlib, json, re

root = Path(__file__).resolve().parent.parent
out = root/'analysis/milestone3c'
load=lambda name: json.loads((out/name).read_text(encoding='utf-8'))
sha=lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
assets=load('damage-assets.json'); oracle=load('variance-oracle.json'); validation=load('variance-validation.json')
assert validation['passed'] and validation['cases']==len(oracle['cases'])==4038
assert validation['provenance']==oracle['provenance']
assert sha(out/'damage-assets.json')==oracle['provenance']['assets_sha256']
assert sha(root/'analysis/decompiled/damage-variance.asm')==oracle['provenance']['assembly_sha256']
scalar=lambda cls,p: assets['scalar'][cls][p]['Ok']['value']['value']
assert scalar('CTCharacters.CloneCommando','Health')==100
assert scalar('CTCharacters.CloneCommando','Shields')==100
assert scalar('CTInventory.DC17BlasterProj','MyDamageType')['path'].lower()=='properties.ctdamageenergydc17'
examples=load('selected-variance-examples.json')['examples']
assert [x['result'] for x in examples]==[93.75,40.,10.,62.5,6.,6.]
log=(out/'workspace-tests.log').read_text(encoding='utf-8')
assert sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed',log)))==958
assert 'Finished' in (out/'clippy.log').read_text(encoding='utf-8')

# Extend the last delivered complete source inventory, preserving old artifacts.
with ZipFile(root/'analysis/milestone3/source-snapshot.zip') as previous:
    names=set(previous.namelist())
for folder in ['crates','docs','scripts']:
    for p in (root/folder).rglob('*'):
        if p.is_file() and p.suffix in ['.rs','.md','.py','.ps1','.java','.toml']:
            names.add(p.relative_to(root).as_posix())
sources={name:sha(root/name) for name in sorted(names)}
with ZipFile(out/'source-snapshot.zip','w',ZIP_DEFLATED) as archive:
    for name in sources: archive.write(root/name,name)
with ZipFile(out/'source-snapshot.zip') as archive:
    for name,expected in sources.items(): assert hashlib.sha256(archive.read(name)).hexdigest()==expected
game=Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData')
originals=[game/name for name in ['System/engine.dll','System/engine.u','System/ctgame.u','System/ctcharacters.u','System/ctinventory.u','Properties/properties.u']]
assert sha(originals[0])==oracle['provenance']['binary_sha256']
old=root/'analysis/milestone3'
assert sha(old/'republic-commando-0.11.0.apk')=='13ac4d57f7ee37d364c8a7f5e62e2b2c74ba15f6a92d4ce38fa056f90f2549b7'
assert sha(old/'geo_01a-combat.rcplay')=='7e01e75d61fcb5609e1894dc064aabc4b56c33672c79fb2ddd5e3977c9cca161'
evidence=[p for p in out.iterdir() if p.is_file() and p.name!='validation-manifest.json']+[root/'analysis/decompiled/damage-variance.asm']
manifest=dict(scope='Prepared native variance and source-based Pawn damage arithmetic; no gameplay/Android integration',
              checks=dict(native_variance_cases=4038,workspace_library_tests=958,clippy=True,source_snapshot_verified=True),
              sources=sources,evidence={p.relative_to(root).as_posix():sha(p) for p in evidence},
              originals={str(p):sha(p) for p in originals},previous_playable='0.11.0 pinned APK/scene unchanged',
              next_step='Fractional combat state and shields, Game.ReduceDamage boundary, melee goal/attack clips/hit notifies; Android at integration end')
(out/'validation-manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
print(f'Verified 4038 native cases, 958 tests and {len(sources)} archived source files')
