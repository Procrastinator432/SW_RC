"""Freeze the GEO_01C Bacta/guard-room/door-breach increment after checks."""
import hashlib,json,re,shutil,struct,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
WORK=ROOT/'analysis/mission24-map3-breach';OUT=WORK/'delivery'
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
route=read(OUT/'playthrough/validation.json');win=read(OUT/'native/validation.json')
android=read(OUT/'android/native-validation.json');ui=read(OUT/'android/validation.json')
assert all(v['passed'] for v in [route,win,android,ui])
assert route['inputs']==win['map3_inputs']==android['map3_inputs']==7081
assert win['real_map1_inputs']==android['real_map1_inputs']==18730
assert win['full_map2_inputs']==android['full_map2_inputs']==16414
assert route['ordinary_input_replay_exact'] and route['exact_restart_state_and_frame']
assert route['frame_rates_exact']==[20,60,144] and route['original_breach_notify_counts']==[5,3,0]
assert route['both_stations_used'] and route['healing_requires_use'] and route['healing_locks_walking_and_fire'] and route['healing_clamped_100']
s=route['final'];o=s['map']['opening'];b=o['planning']['guard']['breach']
assert o['objectives']==[False,True,True,True,True,False,False]
assert [s[k] for k in ['health','ammo','reserve']]==[95,12,103]
assert s['statistics']['kills']==12 and s['statistics']['shots']==46 and s['statistics']['enemy_shots']==9
assert b['stage']==3 and b['door_fraction']==1 and 'settling' not in b and b['healed']==[95,35]
assert all(a['finished'] for a in b['animations']) and [len(a['notifies']) for a in b['animations']]==[5,3,0]
assert o['event_ticks']['SunFacRoomDoorOpening']>=b['animation_tick']>b['command_tick']
assert o['event_ticks']['SunFacRoomDoorOpened']==b['opened_tick']
assert all(e in o['events'] for e in ['MDBDestroySwitch','MDBtossGrenade','LandingPadDoorBreached','ContinueToAARoom','SpawnAARoom','TrackSunFacInsideShield'])
assert len(s['enemies'])==14 and sum(e['health']>0 for e in s['enemies'])==2
assert len(s['map']['events'])==win['audio_events']==android['audio_events']==120
scene=OUT/'geo_01c.rcplay';raw=scene.read_bytes();assert raw[:8]==b'RCPLAY14'
n=struct.unpack_from('<I',raw,8)[0];a=json.loads(raw[12:12+n]);assert n<32*1024*1024
assert len(a['sounds'])==26
assets=a['opening']['planning']['guard']['breach']
assert [x['original'] for x in assets['stations']]==[310,261]
assert [x['original'] for x in assets['doors']]==[120,121]
assert len(assets['navigation']['nodes'])==120 and len(assets['navigation']['edges'])==1127
for mode in ['known','unknown']:
 r=read(WORK/f'scene-reader-{mode}.json');assert r['passed'] and r['heap_limit']==96*1024*1024 and r['sha256']==sha(scene)
tests=(WORK/'workspace-tests.log').read_text(encoding='utf-8-sig')
count=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',tests)));assert count==1116 and 'FAILED' not in tests
assert 'Finished `release`' in (WORK/'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log','whitespace.log']:assert not (WORK/name).read_text(encoding='utf-8-sig').strip()
unchanged={}
for milestone in ['mission15-complete','mission16-map2','mission17-map2-gameplay','mission18-map2-route','mission19-map3','mission20-map3-opening','mission21-map3-planning','mission22-map3-escape','mission23-map3-guard']:
 prior=read(ROOT/f'analysis/{milestone}/delivery/validation-manifest.json')
 for name,record in prior['evidence'].items():
  file=ROOT/name;assert file.stat().st_size==record['bytes'] and sha(file)==record['sha256'],name
 unchanged[milestone]=len(prior['evidence'])
assert sum(unchanged.values())==433
with zipfile.ZipFile(OUT/'republic-commando-0.41.0.apk') as z:
 for abi in ['arm64-v8a','x86_64']:
  for lib in ['librc_native.so','librc_bridge.so']:assert z.read(f'lib/{abi}/{lib}')==(ROOT/'android/app/src/main/jniLibs'/abi/lib).read_bytes()
assert "versionCode='45'" in (WORK/'apk-info.txt').read_text(encoding='utf-8-sig')
for name in ['workspace-tests.log','clippy.log','format.log','whitespace.log','android-build.log','native-build.log','scene-reader-known.json','scene-reader-unknown.json','apk-info.txt','playthrough.log','native-test.log','android-test.log','build.log','original-assets.json','door-damaged-state.json','bacta-animations.json','root-anchors.json']:
 shutil.copyfile(WORK/name,OUT/name)
shutil.copyfile(ROOT/'target/release/rc_native.dll',OUT/'native/rc_native.dll')
shutil.copyfile(ROOT/'docs/MAP3_BREACH.md',OUT/'README.md')
with zipfile.ZipFile(OUT/'original-bindings.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in [ROOT/'analysis/mission19-map3/mission-flow.json',ROOT/'analysis/mission19-map3/original-map-inventory.json',WORK/'original-assets.json',WORK/'breach-clips.json',WORK/'door-damaged-state.json']:z.write(p,p.name)
sources=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'README.md']
for folder,extensions in [('crates',{'.rs','.toml'}),('android',{'.java','.c','.xml','.gradle'}),('scripts',{'.py','.ps1','.c','.java'}),('docs',{'.md'})]:
 for p in (ROOT/folder).rglob('*'):
  if p.is_file() and p.suffix in extensions and not {'build','.gradle','.cxx'}.intersection(p.relative_to(ROOT/folder).parts):sources.append(p)
with zipfile.ZipFile(OUT/'source-snapshot.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in sorted(set(sources)):z.write(p,p.relative_to(ROOT).as_posix())
files=[p for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json']
result=dict(passed=True,version='0.41.0',version_code=45,original_completed_objectives=[1,2,3,4],complete_three_map_mission=False,
 bacta_stations=[310,261],healed=[95,35],breach_roles=['Scorch/doorman','Fixer/grenadier','Sev/coverman'],native_notify_counts=[5,3,0],
 ordinary_map3_inputs=7081,player_health=95,ammo=12,reserve=103,player_shots=46,kills=12,remaining_enemies=2,sounds=26,audio_events=120,
 actual_map1_inputs=18730,actual_map2_inputs=16414,library_tests=count,prior_delivery_evidence_unchanged=unchanged,
 limits=['Sun Fac continuation, AA-room factories, sniper/ending/elevator and objectives0/5/6 pending',
 'BactaRate4 interpreted as four-second full heal; static Holster/Load pose, original activation/FX/audio pending',
 'Bounded grounded navigation and animation root-body movement; no native ReachSpec/CTBot/Squad/Karma parity',
 'Original grenade notify recorded; grenade object/damage, charge prop, door damage area/FX/replacement mesh and speech cues pending',
 'Emitted SpawnAARoom/ContinueToAARoom events do not yet execute the next section',
 'Earlier bounded combat, mover, collision, marker and effects limitations remain',
 'No complete manual touch or physical ARM64 acceptance'],
 evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in sorted(files)})
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2)+'\n')
print(f'0.41.0 frozen: {len(files)} evidence files; {sum(unchanged.values())} prior files unchanged')
