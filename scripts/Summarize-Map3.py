"""Freeze third-map import/transition, without claiming Sun Fac gameplay completion."""
import hashlib,json,re,shutil,zipfile,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];WORK=ROOT/'analysis/mission19-map3';OUT=WORK/'delivery'
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
route=read(OUT/'playthrough/validation.json');win=read(OUT/'native/validation.json');android=read(OUT/'android/native-validation.json');ui=read(OUT/'android/validation.json')
assert all(v['passed'] for v in [route,win,android,ui])
assert route['real_map1_inputs']==win['real_map1_inputs']==android['real_map1_inputs']==18730
assert route['real_map2_inputs']==win['full_map2_inputs']==android['full_map2_inputs']==16414
assert route['map3_inputs']==win['map3_inputs']==android['map3_inputs']==360
assert route['invalid_saved_receipts_atomic']==9 and route['restart_exact_state_frame'] and route['saved_restore_exact_state_frame']
assert route['entry']['health']==10 and route['entry']['ammo']==20 and route['entry']['reserve']==141
assert route['entry']['map']['grenades']==4
for name in ['companion','fixer']:assert route['entry']['map'][name]['health']==100 and route['entry']['map'][name]['shields']==100
assert win['two_members_carried'] and android['two_members_carried'] and ui['apk_saved_handoff_resume']
scene=OUT/'geo_01c.rcplay';data=scene.read_bytes();assert data[:8]==b'RCPLAY14'
a=json.loads(data[12:12+struct.unpack_from('<I',data,8)[0]])
assert a['destination']=='GEO_01C.ctm#playerstart' and a.get('gameplay') is None and a['fixer_mesh'] and len(a['sounds'])==1
assert all(p>>24==255 for m in a['fixer_mesh']['materials'] for p in m['pixels'])
surfaces=read(WORK/'surfaces/surface-receipt.json');world=read(WORK/'world/scene.json');assert not surfaces['errors']
assert surfaces['bsp_textured']==195 and surfaces['mesh_textured']==81553
assert len(world['instances'])==134 and len(world['omitted'])==5
assert [v['actor'] for v in world['omitted'] if v.get('draw_type',{}).get('value')==2]==['Rope6','Rope5','EndLevelIconProp2']
collision=read(OUT/'collision-completion-receipt.json');assert collision['completed_hulls']==142 and len(collision['reconstructed_cell']['planes'])==13
for mode in ['known','unknown']:
 r=read(WORK/f'scene-reader-{mode}.json');assert r['passed'] and r['heap_limit']==96*1024*1024 and r['sha256']==sha(scene)
tests=(WORK/'workspace-tests.log').read_text(encoding='utf-8-sig');count=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',tests)));assert count==1114 and 'FAILED' not in tests
assert 'Finished `release`' in (WORK/'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log','whitespace.log']:assert not (WORK/name).read_text(encoding='utf-8-sig').strip()
unchanged={}
for milestone in ['mission15-complete','mission16-map2','mission17-map2-gameplay','mission18-map2-route']:
 prior=read(ROOT/f'analysis/{milestone}/delivery/validation-manifest.json')
 for name,record in prior['evidence'].items():
  p=ROOT/name;assert p.stat().st_size==record['bytes'] and sha(p)==record['sha256'],name
 unchanged[milestone]=len(prior['evidence'])
with zipfile.ZipFile(OUT/'republic-commando-0.36.0.apk') as z:
 for abi in ['arm64-v8a','x86_64']:
  for lib in ['librc_native.so','librc_bridge.so']:assert z.read(f'lib/{abi}/{lib}')==(ROOT/'android/app/src/main/jniLibs'/abi/lib).read_bytes()
assert "versionCode='40'" in (WORK/'apk-info.txt').read_text(encoding='utf-8-sig')
for name in ['workspace-tests.log','clippy.log','format.log','whitespace.log','android-build.log','native-build.log','scene-reader-known.json','scene-reader-unknown.json','apk-info.txt','playthrough.log','native-test.log','android-test.log','world-build.log','surfaces.log','collision-audit.log']:
 shutil.copyfile(WORK/name,OUT/name)
shutil.copyfile(WORK/'surfaces/surface-receipt.json',OUT/'surface-receipt.json');shutil.copyfile(WORK/'world/scene.json',OUT/'world-receipt.json')
shutil.copyfile(ROOT/'target/release/rc_native.dll',OUT/'native/rc_native.dll');shutil.copyfile(ROOT/'docs/MAP3_INTEGRATION.md',OUT/'README.md')
with zipfile.ZipFile(OUT/'original-bindings.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in [WORK/'original-map-inventory.json',WORK/'mission-flow.json',WORK/'missing-hull.json',WORK/'missing-hull-feasibility.json']:z.write(p,p.name)
sources=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'README.md']
for folder,extensions in [('crates',{'.rs','.toml'}),('android',{'.java','.c','.xml','.gradle'}),('scripts',{'.py','.ps1','.c','.java'}),('docs',{'.md'})]:
 for p in (ROOT/folder).rglob('*'):
  if p.is_file() and p.suffix in extensions and not {'build','.gradle','.cxx'}.intersection(p.relative_to(ROOT/folder).parts):sources.append(p)
with zipfile.ZipFile(OUT/'source-snapshot.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in sorted(set(sources)):z.write(p,p.relative_to(ROOT).as_posix())
files=[p for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json']
result=dict(passed=True,version='0.36.0',version_code=40,map3_import_and_travel_verified=True,map3_gameplay_integrated=False,complete_three_map_mission=False,
 original_start_actor=92,triangles=81748,colliders=74,static_instances=134,original_exit_actor=39,original_next_map='GEO_03A.ctm#playerstart',
 original_objectives_inventoried=7,original_action_chains_inventoried=47,original_factories_inventoried=22,original_actors_actions_inventoried=997,
 real_map1_inputs=18730,real_map2_inputs=16414,map3_control_inputs=360,handoff_schema=2,second_map_exit_actor=567,two_members_carried=True,
 library_tests=count,prior_delivery_evidence_unchanged=unchanged,
 original_map_sha256=sha(Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData/Maps/geo_01c.ctm')),
 limits=['Third map is exploration: original Sun Fac, Sev, glass, super battle droid, breach, snipe and elevator gameplay pending',
 'C squad local physical following only; full navigation/combat/orders/revival and native parity incomplete',
 'One missing BSP cell reconstructed mathematically; no native LineCheck equivalence',
 'Nonstatic default meshes excluded; original rope/intro/end-symbol and 1108 FX triangles not rendered',
 'APK smoke uses actual persisted B→C receipt; no complete manual touch or physical ARM64 acceptance'],
 evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in sorted(files)})
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2)+'\n')
print(f"0.36.0 frozen: {len(files)} evidence files, {count} tests; prior unchanged {unchanged}")
