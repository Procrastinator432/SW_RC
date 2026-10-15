"""Freeze the original SBD intro and objective3 increment after platform checks."""
import hashlib,json,re,shutil,struct,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];WORK=ROOT/'analysis/mission23-map3-guard';OUT=WORK/'delivery'
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
route=read(OUT/'playthrough/validation.json');win=read(OUT/'native/validation.json')
android=read(OUT/'android/native-validation.json');ui=read(OUT/'android/validation.json')
assert all(v['passed'] for v in [route,win,android,ui])
assert route['inputs']==win['map3_inputs']==android['map3_inputs']==4653
assert win['real_map1_inputs']==android['real_map1_inputs']==18730
assert win['full_map2_inputs']==android['full_map2_inputs']==16414
assert route['ordinary_input_replay_exact'] and route['exact_restart_state_and_frame']
assert route['frame_rates_exact']==[20,60,144] and route['native_notify_counts']==[3,2,10] and route['no_early_intro'] and route['closed_door_blocks_focus']
s=route['final'];o=s['map']['opening'];p=o['planning'];g=p['guard']
assert o['objectives']==[False,True,True,True,False,False,False] and o['joined'] and not o['locked']
assert s['health']==5 and s['ammo']==8 and s['reserve']==122
assert s['statistics']['kills']==6 and s['statistics']['shots']==31 and s['statistics']['enemy_shots']==1 and p['assists']==10
assert g['focus_commands']==1 and g['focus'] is None and g['spawned'] and g['sbd']==13
assert len(s['enemies'])==14 and sum(e['health']>0 for e in s['enemies'])==8 and s['enemies'][13]['health']==0
assert [len(a['notifies']) for a in g['animations']]==[3,2,10] and all(a['finished'] for a in g['animations'])
assert len(g['bindings'])==9 and len({i for i,_ in g['bindings']})==9
assert not p['escape']['locked'] and g['intro_tick']==o['event_ticks']['SBDIntro'] and g['destroyed_tick']==o['event_ticks']['SBDDestroyed']
assert all(event in o['events'] for event in ['SpawnGuardRoom','GuardRoomDoorUnlocked','GuardRoomDroids','SBDCombatReady','SBDDestroyed','TrackBreachDoorSunFac'])
assert len(s['map']['events'])==win['audio_events']==android['audio_events']==88
scene=OUT/'geo_01c.rcplay';raw=scene.read_bytes();assert raw[:8]==b'RCPLAY14'
n=struct.unpack_from('<I',raw,8)[0];a=json.loads(raw[12:12+n]);assert n<32*1024*1024
assert len(a['sounds'])==23 and len(a['opening']['planning']['guard']['factories'])==8
for mode in ['known','unknown']:
 r=read(WORK/f'scene-reader-{mode}.json');assert r['passed'] and r['heap_limit']==96*1024*1024 and r['sha256']==sha(scene)
tests=(WORK/'workspace-tests.log').read_text(encoding='utf-8-sig')
count=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',tests)));assert count==1116 and 'FAILED' not in tests
assert 'Finished `release`' in (WORK/'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log','whitespace.log']:assert not (WORK/name).read_text(encoding='utf-8-sig').strip()
unchanged={}
for milestone in ['mission15-complete','mission16-map2','mission17-map2-gameplay','mission18-map2-route','mission19-map3','mission20-map3-opening','mission21-map3-planning','mission22-map3-escape']:
 prior=read(ROOT/f'analysis/{milestone}/delivery/validation-manifest.json')
 for name,record in prior['evidence'].items():
  file=ROOT/name;assert file.stat().st_size==record['bytes'] and sha(file)==record['sha256'],name
 unchanged[milestone]=len(prior['evidence'])
assert sum(unchanged.values())==386
with zipfile.ZipFile(OUT/'republic-commando-0.40.0.apk') as z:
 for abi in ['arm64-v8a','x86_64']:
  for lib in ['librc_native.so','librc_bridge.so']:assert z.read(f'lib/{abi}/{lib}')==(ROOT/'android/app/src/main/jniLibs'/abi/lib).read_bytes()
assert "versionCode='44'" in (WORK/'apk-info.txt').read_text(encoding='utf-8-sig')
for name in ['workspace-tests.log','clippy.log','format.log','whitespace.log','android-build.log','native-build.log','scene-reader-known.json','scene-reader-unknown.json','apk-info.txt','playthrough.log','native-test.log','android-test.log','android-native-test.log','build.log','original-assets.json']:
 shutil.copyfile(WORK/name,OUT/name)
shutil.copyfile(ROOT/'target/release/rc_native.dll',OUT/'native/rc_native.dll')
shutil.copyfile(ROOT/'docs/MAP3_GUARD.md',OUT/'README.md')
with zipfile.ZipFile(OUT/'original-bindings.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in [ROOT/'analysis/mission19-map3/mission-flow.json',ROOT/'analysis/mission19-map3/original-map-inventory.json',ROOT/'analysis/mission22-map3-escape/next-sbd-bindings.json',WORK/'original-assets.json']:z.write(p,p.name)
sources=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'README.md']
for folder,extensions in [('crates',{'.rs','.toml'}),('android',{'.java','.c','.xml','.gradle'}),('scripts',{'.py','.ps1','.c','.java'}),('docs',{'.md'})]:
 for p in (ROOT/folder).rglob('*'):
  if p.is_file() and p.suffix in extensions and not {'build','.gradle','.cxx'}.intersection(p.relative_to(ROOT/folder).parts):sources.append(p)
with zipfile.ZipFile(OUT/'source-snapshot.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in sorted(set(sources)):z.write(p,p.relative_to(ROOT).as_posix())
files=[p for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json']
result=dict(passed=True,version='0.40.0',version_code=44,original_completed_objectives=[1,2,3],complete_three_map_mission=False,
 sbd_intro_executed=True,sbd_intro_actors=[321,322,323],native_notify_counts=[3,2,10],intro_sbd_destroyed=True,
 guard_factories_spawned=[196,197,228,229,230,231,232,233],remaining_optional_guard_enemies=8,focus_commands=1,
 ordinary_map3_inputs=4653,player_health=5,ammo=8,reserve=122,player_shots=31,kills=6,squad_assists=10,sounds=23,audio_events=88,
 actual_map1_inputs=18730,actual_map2_inputs=16414,library_tests=count,prior_delivery_evidence_unchanged=unchanged,
 limits=['Guard-room traversal/remaining enemies, Bacta stations310/261, breach objective4, sniper/Sun Fac ending/elevator objectives0/5/6 pending',
 'Native intro animation math/timing; decorative invulnerable actors until AnimEnd, bounded combat handoff/placement and body retirement',
 'Original health/extents/meshes; bounded hitscan/AI/focus/assistance, no native armor/ballistic/CTBot/Squad/Karma parity',
 'Eleven fire notifies play a shared original BattleDroidBlasterDefaults cue; native SBD weapon/audio binding and other notify effects pending',
 'Earlier bounded entry, Prop, mover, collision and effects limitations remain',
 'No complete manual touch or physical ARM64 acceptance'],
 evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in sorted(files)})
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2)+'\n')
print(f'0.40.0 frozen: {len(files)} evidence files; {count} tests; 386 prior files unchanged')
