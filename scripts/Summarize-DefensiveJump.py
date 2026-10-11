"""Bind prepared M3j defensive planning to instructions, original map and sources."""
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED
import hashlib
import json
import re

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/milestone3j'
def sha(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def read(name):return json.loads((OUT/name).read_text(encoding='utf-8-sig'))
oracle=read('defensive-oracle.json'); check=read('defensive-validation.json'); assets=read('defensive-receipt.json')
assert check['passed'] and check['provenance']==oracle['provenance']
assert check['searches']==len(oracle['searches'])==1798
assert check['plans']==len(oracle['plans'])==392
assert check['post_melee']==len(oracle['post_melee'])==224
assert check['searches_with_destination']>0
assert len(assets['original_linked_order'])==361 and len(assets['nodes'])==16
assert not assets['unlinked_special_nodes']
assert sum(n['class']=='Engine.JumpDest' for n in assets['nodes'])==15
assert sum(n['class']=='CTGame.WarriorPerch' for n in assets['nodes'])==1
assert assets['probability']['value']['value']==0.20000000298023224
assert assets['player_probability']['value']['value']==0.800000011920929
assert read('legacy-replay.json')['exact_replay']
scene=read('scene-regression/game-rules-validation.json')
assert scene['passed'] and [r['defeat_tick'] for r in scene['levels']]==[360,159,38]
for row in scene['levels']:
    for k in ['three_kills_9_hits','restart_exact','cadence_exact','neutral_player_and_enemy','damage_error_rolls_back']:assert row[k],k
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed',(OUT/'workspace-tests.log').read_text(encoding='utf-8-sig'))))
assert tests==980
assert 'Finished' in (OUT/'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log','diff-check.log']:assert not (OUT/name).read_text(encoding='utf-8-sig').strip()
previous=json.loads((ROOT/'analysis/milestone3i/validation-manifest.json').read_text())
for name,r in previous['evidence'].items():
    p=ROOT/name;assert p.stat().st_size==r['bytes'] and sha(p)==r['sha256'],name
originals=dict(previous['originals'])
for name,expected in originals.items():assert sha(Path(name))==expected,name
game=Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData')
assert sha(game/'System/ctgame.dll')==oracle['provenance']['binary_sha256']
for name,expected in oracle['provenance']['assembly_sha256'].items():assert sha(ROOT/name)==expected,name
for folder,names in [('System__ctgame.u',['StateGeonosianWarriorAttack.uc','WarriorPerch.uc','GOAL_JumpTo.uc','GOAL_ChangePhysics.uc']),('System__engine.u',['NavigationPoint.uc','Perch.uc','JumpDest.uc','LevelInfo.uc','Actor.uc'])]:
    for name in names:
        p=ROOT/'analysis/reports/sources'/folder/name;originals[str(p)]=sha(p)
with ZipFile(ROOT/'analysis/milestone3i/source-snapshot.zip') as z:names=set(z.namelist())
for folder in ['crates','scripts','docs']:
    for p in (ROOT/folder).rglob('*'):
        if p.is_file() and p.suffix in ['.rs','.md','.py','.ps1','.java','.toml']:names.add(p.relative_to(ROOT).as_posix())
sources={name:sha(ROOT/name) for name in sorted(names)}
with ZipFile(OUT/'source-snapshot.zip','w',ZIP_DEFLATED) as z:
    for name in sources:z.write(ROOT/name,name)
with ZipFile(OUT/'source-snapshot.zip') as z:
    for name,digest in sources.items():assert hashlib.sha256(z.read(name)).hexdigest()==digest
evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json'}
manifest=dict(stage='M3j-defensive-planning',scope='Prepared original nearest-node, defensive goal planning and post-melee random branch. No scene/APK change, goal execution, flight or new Android acceptance. M3 remains open.',checks=dict(native_cases=2414,workspace_library_tests=tests,clippy=True,format=True,legacy_replay_commands=1002,m3i_three_scene_regressions=True,original_navigation_chain=361,special_nodes=16,previous_delivery_all_artifacts_verified=True,source_snapshot_verified=True),playable_delivery='analysis/milestone3i',playable_version='0.17.0-bot-goals-scene',sources=sources,evidence=evidence,originals=originals,next_step='CalcTrajectoryVelocity/root translation, JumpTo/ChangePhysics/landing, bounded executable defensive goal host, then scene and Android integration')
(OUT/'validation-manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
print(f'Verified {tests} tests, 2414 native planning cases, 361 original navigation nodes and {len(sources)} archived source files; playable delivery remains 0.17.0')
