"""Bind prepared M3k trajectory arithmetic to native instructions and sources."""
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED
import hashlib
import json
import re

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/milestone3k'
def sha(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def read(name):return json.loads((OUT/name).read_text(encoding='utf-8-sig'))
oracle=read('trajectory-oracle.json'); check=read('trajectory-validation.json')
assert check['passed'] and check['provenance']==oracle['provenance']
assert check['cases']==len(oracle['cases'])==4240
assert all(check['extensions']) and sum(check['extensions'])==4240
assert check['seed_calls']==sum(len(c['expected']['seeds']) for c in oracle['cases'])
defensive=read('defensive-validation.json')
assert defensive['passed'] and defensive['searches']==1798 and defensive['plans']==392 and defensive['post_melee']==224
assert read('legacy-replay.json')['exact_replay']
scene=read('scene-regression/game-rules-validation.json')
assert scene['passed'] and [r['defeat_tick'] for r in scene['levels']]==[360,159,38]
for row in scene['levels']:
    for k in ['three_kills_9_hits','restart_exact','cadence_exact','neutral_player_and_enemy','damage_error_rolls_back']:assert row[k],k

tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed',(OUT/'workspace-tests.log').read_text(encoding='utf-8-sig'))))
assert tests==983
assert 'Finished' in (OUT/'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log','diff-check.log']:assert not (OUT/name).read_text(encoding='utf-8-sig').strip()
previous=json.loads((ROOT/'analysis/milestone3j/validation-manifest.json').read_text())
for name,r in previous['evidence'].items():
    p=ROOT/name;assert p.stat().st_size==r['bytes'] and sha(p)==r['sha256'],name
originals=dict(previous['originals'])
for name,expected in originals.items():assert sha(Path(name))==expected,name
game=Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData')
assert sha(game/'System/engine.dll')==oracle['provenance']['binary_sha256']
originals[str(game/'System/engine.dll')]=oracle['provenance']['binary_sha256']
for name,expected in oracle['provenance']['assembly_sha256'].items():assert sha(ROOT/name)==expected,name
for folder,names in [('System__ctgame.u',['StateGeonosianWarriorAttack.uc','WarriorPerch.uc','GOAL_JumpTo.uc','GOAL_ChangePhysics.uc']),('System__engine.u',['NavigationPoint.uc','Perch.uc','JumpDest.uc','LevelInfo.uc','Actor.uc'])]:
    for name in names:
        p=ROOT/'analysis/reports/sources'/folder/name;originals[str(p)]=sha(p)
with ZipFile(ROOT/'analysis/milestone3j/source-snapshot.zip') as z:names=set(z.namelist())
for folder in ['crates','scripts','docs']:
    for p in (ROOT/folder).rglob('*'):
        if p.is_file() and p.suffix in ['.rs','.md','.py','.ps1','.java','.toml']:names.add(p.relative_to(ROOT).as_posix())
sources={name:sha(ROOT/name) for name in sorted(names)}
with ZipFile(OUT/'source-snapshot.zip','w',ZIP_DEFLATED) as z:
    for name in sources:z.write(ROOT/name,name)
with ZipFile(OUT/'source-snapshot.zip') as z:
    for name,digest in sources.items():assert hashlib.sha256(z.read(name)).hexdigest()==digest
evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json'}
manifest=dict(stage='M3k-prepared-jump-trajectory',scope='Prepared original CalcTrajectoryVelocity arithmetic with explicit supplied RSQRTSS seeds. Root translation lookup investigated; serialized origin unresolved. No scene/APK change, goal execution, flight or new Android acceptance. M3 remains open.',checks=dict(native_trajectory_cases=4240,extension_cases=check['extensions'],seed_calls=check['seed_calls'],workspace_library_tests=tests,clippy=True,format=True,legacy_replay_commands=1002,m3i_three_scene_regressions=True,m3j_planning_cases=2414,previous_delivery_all_artifacts_verified=True,source_snapshot_verified=True),playable_delivery='analysis/milestone3i',playable_version='0.17.0-bot-goals-scene',sources=sources,evidence=evidence,originals=originals,next_step='Stored sequence translation origin/animation names/speed field, ChangePhysics and executable JumpTo start/flight/landing host, then scene and Android integration')
(OUT/'validation-manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
print(f'Verified {tests} tests, 4240 native trajectory cases, previous delivery artifacts and {len(sources)} archived source files; playable delivery remains 0.17.0')
