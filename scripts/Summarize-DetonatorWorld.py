"""Bind world-flight feasibility and the compatible arena surface correction."""
import hashlib
import json
import re
import struct
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'analysis/mission4-world-flight'
GAME=Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData')
def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream,'sha256').hexdigest()
def read(name):
    return json.loads((OUT/name).read_text(encoding='utf-8-sig'))
def snapshot_offset(raw):
    at=0
    while raw[at:at+8]!=b'RCPLAY01':
        assert raw[at:at+8] in [b'RCPLAY04',b'RCPLAY02']
        at+=12+struct.unpack_from('<I',raw,at+8)[0]
    return at+12+struct.unpack_from('<I',raw,at+8)[0]
scene=OUT/'geo_01a-grenades-arena.rcplay'
old=(ROOT/'analysis/mission3-playable/geo_01a-grenades.rcplay').read_bytes()
new=scene.read_bytes()
old_at,new_at=map(snapshot_offset,[old,new])
assert old_at==new_at and old[:old_at]==new[:new_at]
a,b=old[old_at:],new[new_at:]
assert struct.unpack_from('<II',a,4)==struct.unpack_from('<II',b,4)
version,count=struct.unpack_from('<II',a,4)
assert version==3
assert all(a[20+i*68:60+i*68]==b[20+i*68:60+i*68] for i in range(count))
repair=read('arena-repair-validation.json')
assert repair['passed'] and repair['initial_game_state_unchanged'] and repair['bundle_metadata_byte_exact']
assert repair['rebound_faces']==13743 and repair['layered_materials']==14 and repair['untextured_faces']==0
for folder in ['', 'arena/']:
    v=read(folder+'world-validation.json')
    assert v['passed'] and v['actual_clipped_projectile_contact'] and v['route_from_pickup_physically_walked']
    assert v['original_trigger']==621 and v['attempts']==6
    r=read(folder+'render-validation.json')
    assert r['passed'] and r['world_depth'] and r['textured_faces']==r['projectile_faces']==352
    assert all(row['changed_pixels']>0 for row in r['frames'])
assert (OUT/'world-flight-trace.json').read_bytes()==(OUT/'arena/world-flight-trace.json').read_bytes()
regression=read('pickup-regression/validation.json')
for field in ['passed','input_driven_all_routes','objective2_completed','restart_state_and_frame_exact','static_gpu_packet_immutable']:
    assert regression[field],field
assert regression['kills']==regression['grenades']==5 and regression['won_tick']==5030
abi=read('abi/abi-validation.json')
assert abi['passed'] and abi['scene_sha256']==sha(scene) and abi['physical_walk_three_objectives']
tests_log=(OUT/'workspace-tests.log').read_text(encoding='utf-8-sig')
assert 'FAILED' not in tests_log
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed',tests_log)))
assert tests==1031,tests
assert 'Finished' in (OUT/'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log','source-diff-check.log']:
    assert not (OUT/name).read_text(encoding='utf-8-sig').strip(),name
prior_path=ROOT/'analysis/mission4-flight/validation-manifest.json'
prior=json.loads(prior_path.read_text())
for name,row in prior['evidence'].items():
    p=ROOT/name
    assert p.stat().st_size==row['bytes'] and sha(p)==row['sha256'],name
originals=dict(prior['originals'])
for name in ['Textures/Arena_Textures.utx','StaticMeshes/Arena_HR.usx','System/engine.dll']:
    originals[str(GAME/name)]=sha(GAME/name)
for name,digest in originals.items():
    assert sha(Path(name))==digest,name
names=set(prior['sources'])
for folder in ['crates','docs','scripts','android/app/src/main/java']:
    names.update(p.relative_to(ROOT).as_posix() for p in (ROOT/folder).rglob('*')
                 if p.is_file() and p.suffix in ['.rs','.md','.py','.ps1','.java','.toml'])
sources={name:sha(ROOT/name) for name in sorted(names)}
with ZipFile(OUT/'source-snapshot.zip','w',ZIP_DEFLATED) as archive:
    for name in sources:archive.write(ROOT/name,name)
with ZipFile(OUT/'source-snapshot.zip') as archive:
    assert archive.testzip() is None
    for name,digest in sources.items():assert hashlib.sha256(archive.read(name)).hexdigest()==digest
evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p))
          for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json'}
for name in ['detonator-buoyancy.c','detonator-landed.c','arena-texscaler.c']:
    p=ROOT/'analysis/decompiled'/name
    evidence[p.relative_to(ROOT).as_posix()]=dict(bytes=p.stat().st_size,sha256=sha(p))
manifest=dict(stage='original-detonator-world-feasibility-and-arena-surfaces',
    scope='Selected dry static-world grenade adapter, continuous pawn route from pickup and actual clipped projectile contact621. Compatible 0.21.0 scene surface correction:14 original base layers/13743 faces, no full lightmap/blend parity. No Game/JNI/touch throw, all native projectile filters, damage selection, live bunker transitions or Android objective3 acceptance.',
    scene=dict(file=scene.name,sha256=sha(scene),compatible_app='0.21.0',original_metadata_byte_exact=True,original_geometry_byte_exact=True),
    checks=dict(workspace_library_tests=tests,new_rule_tests=9,physical_pickup_to_throw_route=True,
                physical_clipped_trigger621_contact=True,trajectory_unchanged_by_texture_repair=True,
                original_layered_materials=14,corrected_static_faces=13743,original_textured_projectile_faces=352,
                selected_mission_regression=True,native_abi=True,clippy=True,format=True,source_diff_check=True,
                prior_foundation_evidence_unchanged=len(prior['evidence']),playable_objective3=False,android_tested=False),
    diagnostics=dict(workspace_diff='diff-check.log contains unrelated IntelliJ .idea CRLF whitespace; these user IDE files were preserved. Source-scoped diff-check is clean.',
                     initial_subset='flight-tests.log covers initial six flight cases; workspace-tests.log covers final nine new rules and all1031 library tests.'),
    previous_foundation_manifest_sha256=sha(prior_path),originals=originals,sources=sources,evidence=evidence)
(OUT/'validation-manifest.json').write_bytes((json.dumps(manifest,indent=2)+'\n').encode())
print(f'Bound {len(evidence)} evidence files, {len(sources)} sources, {len(originals)} originals; {tests} tests; scene {sha(scene)}')
