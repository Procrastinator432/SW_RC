"""Freeze the original third-map entry lanes; remaining Sun Fac gameplay stays explicit."""
import hashlib,json,re,shutil,zipfile,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];WORK=ROOT/'analysis/mission20-map3-opening';OUT=WORK/'delivery'
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
route=read(OUT/'playthrough/validation.json');win=read(OUT/'native/validation.json');android=read(OUT/'android/native-validation.json');ui=read(OUT/'android/validation.json')
assert all(v['passed'] for v in [route,win,android,ui])
assert route['inputs']==win['map3_inputs']==android['map3_inputs']==1800
assert win['real_map1_inputs']==android['real_map1_inputs']==18730
assert win['full_map2_inputs']==android['full_map2_inputs']==16414
assert route['exact_restart'] and route['exact_restart_frame'] and route['locked_inputs_do_not_spend_or_move'] and route['frame_rates_exact']==[20,60,144]
s=route['final'];o=s['map']['opening'];assert not o['locked'] and o['joined'] and o['objectives']==[False,True,False,False,False,False,False]
assert o['sev_body']['grounded'] and s['health']==10 and s['ammo']==20 and s['reserve']==139
assert [len(o['animations'][i]['notifies']) for i in [0,3,4,5]]==[5,9,2,3]
assert len(s['map']['events'])==win['audio_events']==android['audio_events']==30
assert [o['animations'][i]['blocked'] for i in [0,1,2]]==[42,43,43]
assert 'CheckpointLevelStart' in o['events'] and 'TargetSunFac' in o['events'] and ui['opening_and_sev_join']
scene=OUT/'geo_01c.rcplay';b=scene.read_bytes();assert b[:8]==b'RCPLAY14';a=json.loads(b[12:12+struct.unpack_from('<I',b,8)[0]])
assert len(a['sounds'])==14 and len(a['opening']['actors'])==8 and len(a['opening']['scripts'])==9
assert a['opening']['eye_height']==60 and a['opening']['player']['clips'][0]['sequence']['frames']==153
for receiver in read(WORK/'rope-receivers.json'):
 assert receiver['decoded']['Ok']['values'][0]['value']['value']=='CloneEntrance1760'
for mode in ['known','unknown']:
 r=read(WORK/f'scene-reader-{mode}.json');assert r['passed'] and r['heap_limit']==96*1024*1024 and r['sha256']==sha(scene)
tests=(WORK/'workspace-tests.log').read_text(encoding='utf-8-sig');count=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',tests)));assert count==1114 and 'FAILED' not in tests
assert 'Finished `release`' in (WORK/'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log','whitespace.log']:assert not (WORK/name).read_text(encoding='utf-8-sig').strip()
unchanged={}
for milestone in ['mission15-complete','mission16-map2','mission17-map2-gameplay','mission18-map2-route','mission19-map3']:
 prior=read(ROOT/f'analysis/{milestone}/delivery/validation-manifest.json')
 for name,record in prior['evidence'].items():
  p=ROOT/name;assert p.stat().st_size==record['bytes'] and sha(p)==record['sha256'],name
 unchanged[milestone]=len(prior['evidence'])
with zipfile.ZipFile(OUT/'republic-commando-0.37.0.apk') as z:
 for abi in ['arm64-v8a','x86_64']:
  for lib in ['librc_native.so','librc_bridge.so']:assert z.read(f'lib/{abi}/{lib}')==(ROOT/'android/app/src/main/jniLibs'/abi/lib).read_bytes()
assert "versionCode='41'" in (WORK/'apk-info.txt').read_text(encoding='utf-8-sig')
for name in ['workspace-tests.log','clippy.log','format.log','whitespace.log','android-build.log','native-build.log','scene-reader-known.json','scene-reader-unknown.json','apk-info.txt','playthrough.log','native-test.log','android-test.log','assets.log']:
 shutil.copyfile(WORK/name,OUT/name)
shutil.copyfile(ROOT/'target/release/rc_native.dll',OUT/'native/rc_native.dll');shutil.copyfile(ROOT/'docs/MAP3_OPENING.md',OUT/'README.md')
(OUT/'selected-lanes.json').write_text(json.dumps(dict(scripts=a['opening']['scripts'],actors=[{k:v for k,v in x.items() if k!='mesh'} for x in a['opening']['actors']],player_notifies=a['opening']['player_sounds'],limits=a['opening']['limits']),indent=2))
with zipfile.ZipFile(OUT/'original-bindings.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in [WORK/'animation-inventory.json',WORK/'player-curves.json',WORK/'rope-receivers.json',ROOT/'analysis/mission19-map3/mission-flow.json',ROOT/'analysis/mission19-map3/original-map-inventory.json']:z.write(p,p.name)
sources=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'README.md']
for folder,extensions in [('crates',{'.rs','.toml'}),('android',{'.java','.c','.xml','.gradle'}),('scripts',{'.py','.ps1','.c','.java'}),('docs',{'.md'})]:
 for p in (ROOT/folder).rglob('*'):
  if p.is_file() and p.suffix in extensions and not {'build','.gradle','.cxx'}.intersection(p.relative_to(ROOT/folder).parts):sources.append(p)
with zipfile.ZipFile(OUT/'source-snapshot.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in sorted(set(sources)):z.write(p,p.relative_to(ROOT).as_posix())
files=[p for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json']
result=dict(passed=True,version='0.37.0',version_code=41,map3_original_entry_integrated=True,sev_joined=True,original_completed_objectives=[1],
 complete_three_map_mission=False,sun_fac_eliminated=False,planning_room_combat=False,original_sequence_frames=dict(rappel=153,sev_intro=195),
 original_programs=[103,98,99,101,264,292,293],original_rope_receivers=[100,102],sounds=14,audio_events=30,
 actual_map1_inputs=18730,actual_map2_inputs=16414,map3_inputs=1800,exact_frame_rates=[20,60,144],library_tests=count,
 prior_delivery_evidence_unchanged=unchanged,limits=['Selected original entry lanes; planning-room factories, flyby, glass, Sun Fac escape and later objectives pending',
 'Port standing body clamps final authored motion: player42/scorch43/fixer43 frames; no native root-motion/capsule parity',
 'Original notify timing; partial side effects, particles, Pike props and Karma/ragdolls pending',
 'CheckpointLevelStart is an event only; persistent data remains the B→C receipt',
 'No full manual touch or physical ARM64 acceptance'],
 evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in sorted(files)})
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2)+'\n')
print(f"0.37.0 frozen: {len(files)} evidence files, {count} tests; prior unchanged {unchanged}")
