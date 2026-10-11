"""Bind the prepared Actor falling arithmetic and original DC15s release checks."""
import hashlib
import json
import re
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT/'analysis/mission4-flight'
GAME = Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData')

def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream,'sha256').hexdigest()

def read(name):
    return json.loads((OUT/name).read_text(encoding='utf-8-sig'))

oracle, falling, release = map(read,['falling-oracle.json','falling-validation.json','release-validation.json'])
assert falling['passed'] and falling['bit_exact'] and falling['cases']==len(oracle['cases'])==4096
assert oracle['binary_sha256']==sha(GAME/'System/engine.dll')
assert oracle['assembly_sha256']==sha(OUT/'falling-instructions.txt')
for field in ['passed','five_notify_consumptions','empty_sound','reentrant_throw_ignored','weapon_gate_until_animation_end']:
    assert release[field],field
assert [(r['release_tick'],r['end_tick'],r['remaining']) for r in release['traces']]==[(35,57,i) for i in range(4,-1,-1)]
original=read('throw-original.json')
assert original['frames']==29 and original['rate']==30 and original['evaluated_geometry_frames']==3
assert len([n for n in original['notifies'] if n['class']=='Engine.AnimNotify_ReleaseGrenade'])==1
tests_log=(OUT/'workspace-tests.log').read_text(encoding='utf-8-sig')
assert 'FAILED' not in tests_log
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed',tests_log)))
assert tests==1022,tests
assert 'Finished' in (OUT/'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log','diff-check.log']:
    assert not (OUT/name).read_text(encoding='utf-8-sig').strip(),name
prior_path=ROOT/'analysis/mission4-detonator/validation-manifest.json'
prior=json.loads(prior_path.read_text())
for name,row in prior['evidence'].items():
    p=ROOT/name
    assert p.stat().st_size==row['bytes'] and sha(p)==row['sha256'],name
originals=dict(prior['originals'])
for name in ['Animations/hudarms.ukx','System/engine.dll']:
    originals[str(GAME/name)]=sha(GAME/name)
for name,digest in originals.items():
    assert sha(Path(name))==digest,name
names=set(prior['sources'])
for folder in ['crates','docs','scripts','android/app/src/main/java']:
    names.update(p.relative_to(ROOT).as_posix() for p in (ROOT/folder).rglob('*')
                 if p.is_file() and p.suffix in ['.rs','.md','.py','.ps1','.java','.toml'])
sources={name:sha(ROOT/name) for name in sorted(names)}
with ZipFile(OUT/'source-snapshot.zip','w',ZIP_DEFLATED) as archive:
    for name in sources:
        archive.write(ROOT/name,name)
with ZipFile(OUT/'source-snapshot.zip') as archive:
    assert archive.testzip() is None
    for name,digest in sources.items():
        assert hashlib.sha256(archive.read(name)).hexdigest()==digest
evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p))
          for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json'}
manifest=dict(stage='original-detonator-falling-and-release-foundation',
    scope='Prepared Actor falling SSE velocity/displacement and native iteration/refund budget; original DC15s clip/notify and selected weapon gate with real channel frames. No MoveActor/GetNetBuoyancy/landing/world-flight, Game/JNI/touch integration or playable objective3 acceptance.',
    checks=dict(workspace_library_tests=tests,new_rule_tests=9,native_actor_falling_cases=4096,
                bit_exact_prepared_falling=True,original_release_notify=True,five_throw_cycles=True,
                empty_inventory=True,clippy=True,format=True,source_snapshot_verified=True,
                prior_foundation_evidence_unchanged=len(prior['evidence']),playable_objective3=False,android_tested=False),
    previous_foundation_manifest_sha256=sha(prior_path),originals=originals,sources=sources,evidence=evidence)
(OUT/'validation-manifest.json').write_bytes((json.dumps(manifest,indent=2)+'\n').encode())
print(f'Bound {len(evidence)} evidence files, {len(sources)} sources and {len(originals)} original files; {tests} tests')
