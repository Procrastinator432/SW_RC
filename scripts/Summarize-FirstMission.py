"""Freeze the playable first mission including its actual factory scene transition."""
import hashlib,json,re,shutil,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];WORK=ROOT/'analysis/mission28-first-mission-complete';OUT=WORK/'delivery'
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
r=read(OUT/'playthrough/validation.json');w=read(OUT/'native/validation.json');a=read(OUT/'android/native-validation.json');u=read(OUT/'android/validation.json')
assert all(v['passed'] for v in [r,w,a,u])
assert [r[k] for k in ['actual_map1_inputs','actual_map2_inputs','actual_map3_inputs','factory_inputs']]==[18730,16414,11800,360]
for v in [w,a]:
 assert [v[k] for k in ['real_map1_inputs','full_map2_inputs','map3_inputs','factory_inputs']]==[18730,16414,11800,360]
 assert v['actual_factory_travel'] and v['original_playerstart'] and v['three_members_carried'] and v['wrong_travel_atomic'] and v['factory_audio']==2
assert r['bad_saved_carries_atomic']==12 and r['saved_restore_exact_state_and_frame'] and r['restart_and_replay_exact_state_and_frame']
entry=r['entry'];final=r['final'];assert [entry[k] for k in ['health','ammo','reserve']]==[65,14,65] and [final[k] for k in ['health','ammo','reserve']]==[65,20,57]
assert entry['pawn']['body']['position']==[-2555.739501953125,2654.17138671875,6920.81640625] and entry['map']['restored'] and entry['map']['grenades']==4
for key in ['companion','fixer','sev']:
 m=entry['map'][key];assert m['joined'] and m['health']==100 and m['shields']==100 and not m['down']
 assert final['map'][key]['position']!=m['position']
assert final['statistics']['shots']==2 and final['statistics']['jumps']==1 and final['pawn']['body']['grounded']
for folder in ['playthrough','native','android']:
 h=read(OUT/folder/'map3-handoff.json');assert h['schema']==3 and h['trigger']==39 and h['destination']=='GEO_03A.ctm#playerstart' and h['map3']['objectives']==[True]*7
surfaces=read(WORK/'surfaces/surface-receipt.json');assert surfaces['errors']==[] and surfaces['untextured']==0 and surfaces['fx_omitted']==4715
build=read(OUT/'build-receipt.json');assert build['colliders']==184 and build['triangles']==118736 and build['bytes']==24985911
scope=read(WORK/'import-scope.json');assert [v['original'] for v in scope['omitted_non_static_bodies']]==[740]
scene=OUT/'geo_03a.rcplay'
for mode in ['known','unknown']:
 v=read(WORK/f'scene-reader-{mode}.json');assert v['passed'] and v['sha256']==sha(scene) and v['heap_limit']==96*1024*1024
assert read(WORK/'campaign-routes.json')['all_three_links']
assert sha(OUT/'geo_01c.rcplay')==sha(ROOT/'analysis/mission27-map3-elevator/delivery/geo_01c.rcplay')
count=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(WORK/'workspace-tests.log').read_text(encoding='utf-8-sig'))));assert count==1120
assert 'Finished `release`' in (WORK/'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log','whitespace.log']:assert not (WORK/name).read_text(encoding='utf-8-sig').strip()
prior={}
for name in ['mission15-complete','mission16-map2','mission17-map2-gameplay','mission18-map2-route','mission19-map3','mission20-map3-opening','mission21-map3-planning','mission22-map3-escape','mission23-map3-guard','mission24-map3-breach','mission25-map3-balcony','mission26-map3-finale','mission27-map3-elevator']:
 m=read(ROOT/f'analysis/{name}/delivery/validation-manifest.json')
 for path,record in m['evidence'].items():
  p=ROOT/path;assert p.stat().st_size==record['bytes'] and sha(p)==record['sha256'],path
 prior[name]=len(m['evidence'])
assert sum(prior.values())==604
with zipfile.ZipFile(OUT/'republic-commando-0.45.0.apk') as z:
 for abi in ['arm64-v8a','x86_64']:
  for lib in ['librc_native.so','librc_bridge.so']:assert z.read(f'lib/{abi}/{lib}')==(ROOT/'android/app/src/main/jniLibs'/abi/lib).read_bytes()
assert "versionCode='49'" in (WORK/'apk-info.txt').read_text(encoding='utf-8-sig')
for name in ['world.log','surfaces.log','build.log','audit.log','import-scope.json','playthrough.log','native-build.log','native-test.log','android-build.log','android-test.log','workspace-tests.log','clippy.log','format.log','whitespace.log','campaign-routes.json','scene-reader-known.json','scene-reader-unknown.json','apk-info.txt']:
 shutil.copyfile(WORK/name,OUT/name)
shutil.copyfile(WORK/'surfaces/surface-receipt.json',OUT/'surface-receipt.json')
shutil.copyfile(ROOT/'target/release/rc_native.dll',OUT/'native/rc_native.dll')
shutil.copyfile(ROOT/'docs/FIRST_MISSION_COMPLETE.md',OUT/'README.md')
with zipfile.ZipFile(OUT/'original-bindings.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in [WORK/'original/original-map-inventory.json',WORK/'import-scope.json',OUT/'build-receipt.json',OUT/'surface-receipt.json']:z.write(p,p.name)
package=[ROOT/'analysis/mission15-complete/delivery/geo_01a-first-mission.rcplay',ROOT/'analysis/mission18-map2-route/delivery/geo_01b.rcplay',OUT/'geo_01c.rcplay',scene,OUT/'republic-commando-0.45.0.apk',OUT/'README.md']
with zipfile.ZipFile(OUT/'mission1-playable-0.45.0.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in package:z.write(p,p.name)
 z.writestr('SHA256.json',json.dumps({p.name:dict(bytes=p.stat().st_size,sha256=sha(p)) for p in package},indent=2))
sources=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'README.md']
for folder,extensions in [('crates',{'.rs','.toml'}),('android',{'.java','.c','.xml','.gradle'}),('scripts',{'.py','.ps1','.c','.java'}),('docs',{'.md'})]:
 for p in (ROOT/folder).rglob('*'):
  if p.is_file() and p.suffix in extensions and not {'build','.gradle','.cxx'}.intersection(p.relative_to(ROOT/folder).parts):sources.append(p)
with zipfile.ZipFile(OUT/'source-snapshot.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in sorted(set(sources)):z.write(p,p.relative_to(ROOT).as_posix())
files=[p for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json']
manifest=dict(passed=True,version='0.45.0',version_code=49,first_mission_playable_through_actual_factory_transition=True,full_original_engine_parity=False,
 actual_map1_inputs=18730,actual_map2_inputs=16414,actual_map3_inputs=11800,factory_control_inputs=360,all_seven_c_objectives=True,three_squad_members_carried=True,
 entry_health=65,entry_ammo=14,entry_reserve=65,grenades=4,original_factory_playerstart=8,original_factory_exit_trigger=39,library_tests=count,
 prior_delivery_evidence_unchanged=prior,
 limits=['Scene selection remains an Android document picker after each Next button','Factory destination contains static world, entry controls and carried squad; next mission scripts/encounters/mover sequence pending',
 '4715 transparent effect surfaces and hidden DefaultProp light carrier740 omitted explicitly; full original dynamic effects/mover collision pending',
 'Earlier AI/Squad/marker, ballistics/grenade/door damage, Bacta, shader/music/particles and staging parity limits remain','Automated Android x86_64; ARM64 built, full physical/manual-touch acceptance pending'],
 evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in sorted(files)})
(OUT/'validation-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(f'0.45.0 frozen: {len(files)} evidence files; {sum(prior.values())} prior files unchanged')
