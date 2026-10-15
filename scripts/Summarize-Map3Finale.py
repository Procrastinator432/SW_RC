"""Freeze Sun Fac's original fighter success/failure increment after checks."""
import base64,hashlib,json,re,shutil,struct,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];WORK=ROOT/'analysis/mission26-map3-finale';OUT=WORK/'delivery'
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
route=read(OUT/'playthrough/validation.json');win=read(OUT/'native/validation.json')
android=read(OUT/'android/native-validation.json');ui=read(OUT/'android/validation.json')
assert all(v['passed'] for v in [route,win,android,ui])
assert route['inputs']==win['map3_inputs']==android['map3_inputs']==10841
assert route['failure_inputs']==win['failure_inputs']==android['failure_inputs']==10526
assert win['ordinary_failure_without_sev'] and android['ordinary_failure_without_sev'] and route['failure_without_sev_exact_replay']
assert win['real_map1_inputs']==android['real_map1_inputs']==18730
assert win['full_map2_inputs']==android['full_map2_inputs']==16414
assert route['ordinary_input_replay_exact'] and route['exact_restart_state_and_frame'] and route['frame_rates_exact']==[20,60,144]
s=route['final'];o=s['map']['opening'];b=o['planning']['guard']['breach']['balcony']['finale']
assert o['objectives']==[True,True,True,True,True,True,False] and o['active']==6 and s['phase']=='Playing'
assert [s[k] for k in ['health','ammo','reserve']]==[80,9,84]
assert s['statistics']['kills']==20 and s['statistics']['shots']==68 and s['statistics']['enemy_shots']==12
assert b['sun_stage']==6 and b['ship_stage']==5 and b['success'] and b['shots']==7 and b['blocked']==0
assert not b['ship_visible'] and b['wreck_visible'] and b['failed_tick'] is None
assert len(b['ship_animation']['notifies'])==9 and b['sun_animation']['random'] is not None
assert b['terminated_tick']==o['event_ticks']['KillMe']==o['event_ticks']['SunFacTerminated']
assert b['start_tick']<b['takeoff_tick']<b['terminated_tick']
assert all(e in o['events'] for e in ['StartSunFacRunning','SunFacRunToShip','SpawnClimbOverWall','TakeOff','SnipeSunFac','ConditionSunFacDead','DisableBalconySnipe','TrackElevatorDoor'])
assert len(s['enemies'])==23 and sum(e['health']>0 for e in s['enemies'])==3 and b['climber']==22
assert len(s['map']['events'])==win['audio_events']==android['audio_events']==166
failure=route['failure'];f=failure['map']['opening']['planning']['guard']['breach']['balcony']['finale']
assert failure['phase']=='Lost' and failure['health']==0 and not f['success'] and f['failed_tick'] is not None
assert f['terminated_tick'] is None and f['shots']==0 and f['reminder_index']==4 and len(f['ship_animation']['notifies'])==5
assert not failure['map']['opening']['objectives'][0]
scene=OUT/'geo_01c.rcplay';raw=scene.read_bytes();assert raw[:8]==b'RCPLAY14'
n=struct.unpack_from('<I',raw,8)[0];a=json.loads(raw[12:12+n]);assert n<32*1024*1024 and len(a['sounds'])==47
old=(ROOT/'analysis/mission25-map3-balcony/delivery/geo_01c.rcplay').read_bytes();old_n=struct.unpack_from('<I',old,8)[0];old_assets=json.loads(old[12:12+old_n])
for i,sound in enumerate(a['sounds']):
 assert sound['wave'].startswith('base64:');wave=base64.b64decode(sound['wave'][7:],validate=True);assert wave[:4]==b'RIFF'
 if i<28:assert sound['source']==old_assets['sounds'][i]['source'] and wave==bytes(old_assets['sounds'][i]['wave'])
for mode in ['known','unknown']:
 r=read(WORK/f'scene-reader-{mode}.json');assert r['passed'] and r['heap_limit']==96*1024*1024 and r['sha256']==sha(scene)
tests=(WORK/'workspace-tests.log').read_text(encoding='utf-8-sig');count=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',tests)))
assert count==1119 and 'FAILED' not in tests and 'Finished `release`' in (WORK/'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log','whitespace.log']:assert not (WORK/name).read_text(encoding='utf-8-sig').strip()
unchanged={}
for milestone in ['mission15-complete','mission16-map2','mission17-map2-gameplay','mission18-map2-route','mission19-map3','mission20-map3-opening','mission21-map3-planning','mission22-map3-escape','mission23-map3-guard','mission24-map3-breach','mission25-map3-balcony']:
 prior=read(ROOT/f'analysis/{milestone}/delivery/validation-manifest.json')
 for name,record in prior['evidence'].items():
  file=ROOT/name;assert file.stat().st_size==record['bytes'] and sha(file)==record['sha256'],name
 unchanged[milestone]=len(prior['evidence'])
assert sum(unchanged.values())==516
with zipfile.ZipFile(OUT/'republic-commando-0.43.0.apk') as z:
 for abi in ['arm64-v8a','x86_64']:
  for lib in ['librc_native.so','librc_bridge.so']:assert z.read(f'lib/{abi}/{lib}')==(ROOT/'android/app/src/main/jniLibs'/abi/lib).read_bytes()
assert "versionCode='47'" in (WORK/'apk-info.txt').read_text(encoding='utf-8-sig')
for name in ['workspace-tests.log','clippy.log','format.log','whitespace.log','android-build.log','native-build.log','scene-reader-known.json','scene-reader-unknown.json','apk-info.txt','playthrough.log','native-test.log','android-test.log','android-native-test.log','build.log','original-assets.json','ship-roots.json','materials.json','assets.log']:
 shutil.copyfile(WORK/name,OUT/name)
shutil.copyfile(ROOT/'target/release/rc_native.dll',OUT/'native/rc_native.dll')
shutil.copyfile(ROOT/'docs/MAP3_FINALE.md',OUT/'README.md')
with zipfile.ZipFile(OUT/'original-bindings.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in [ROOT/'analysis/mission19-map3/mission-flow.json',ROOT/'analysis/mission19-map3/original-map-inventory.json',WORK/'original-assets.json',WORK/'ship-roots.json',WORK/'materials.json']:z.write(p,p.name)
sources=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'README.md']
for folder,extensions in [('crates',{'.rs','.toml'}),('android',{'.java','.c','.xml','.gradle'}),('scripts',{'.py','.ps1','.c','.java'}),('docs',{'.md'})]:
 for p in (ROOT/folder).rglob('*'):
  if p.is_file() and p.suffix in extensions and not {'build','.gradle','.cxx'}.intersection(p.relative_to(ROOT/folder).parts):sources.append(p)
with zipfile.ZipFile(OUT/'source-snapshot.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in sorted(set(sources)):z.write(p,p.relative_to(ROOT).as_posix())
files=[p for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json']
result=dict(passed=True,version='0.43.0',version_code=47,original_completed_objectives=[0,1,2,3,4,5],complete_three_map_mission=False,
 sunfac_terminated_by_original_killme_notify=True,original_ship_notifies=9,sev_sniper_shots=7,ordinary_failure_without_sev=True,
 ordinary_map3_inputs=10841,failure_inputs=10526,player_health=80,ammo=9,reserve=84,player_shots=68,kills=20,remaining_enemies=3,sounds=47,audio_events=166,
 actual_map1_inputs=18730,actual_map2_inputs=16414,library_tests=count,prior_28_original_waves_byte_exact=True,prior_delivery_evidence_unchanged=unchanged,
 limits=['Factory elevator objective6, physical interaction, exit39/GEO_03A and full first-mission completion pending',
 'Selected original scripts/clips/root/notify math; flight is decorative/invulnerable, full physics/Karma/music/particles and Grievous flyby pending',
 'Five-second ShootTarget window with 0.8s port cadence, seven shots; no native weapon ballistics/damage or full Squad/marker behavior parity',
 'One supported warrior from spawnpoint488; native climb, spawn selection/scheduling/patrol and targeting Sev pending',
 'Stunned uses native jitter update with local seeded CRT stream; global original RNG and full jitter-pose parity pending',
 'Goal0 completed from SunFacTerminated as explicit port mapping; original287 repeats objective1',
 'Failure ends player health/phase after original dialogue; full native squad HurtActor/incapacitation not executed',
 'Earlier healing, door damage, grenade, marker, shader and other bounded-host limitations remain',
 'No complete manual touch or physical ARM64 acceptance'],
 evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in sorted(files)})
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2)+'\n')
print(f'0.43.0 frozen: {len(files)} evidence files; {sum(unchanged.values())} prior files unchanged')
