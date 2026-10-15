"""Freeze the planning-room increment after production and emulator checks."""
import hashlib,json,re,shutil,struct,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
WORK=ROOT/'analysis/mission21-map3-planning';OUT=WORK/'delivery'
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
route=read(OUT/'playthrough/validation.json');win=read(OUT/'native/validation.json')
android=read(OUT/'android/native-validation.json');ui=read(OUT/'android/validation.json')
assert all(v['passed'] for v in [route,win,android,ui])
assert route['inputs']==win['map3_inputs']==android['map3_inputs']==3357
assert win['real_map1_inputs']==android['real_map1_inputs']==18730
assert win['full_map2_inputs']==android['full_map2_inputs']==16414
assert route['restart_state_and_frame'] and route['ordinary_input_replay_exact']
assert route['opening_fps']==[20,60,144] and route['real_wall_occlusion'] and route['no_enemy_damage_through_glass']
assert read(OUT/'playthrough/occlusion.json')['passed'] and read(OUT/'playthrough/failure-path.json')['passed']
s=route['final'];o=s['map']['opening'];p=o['planning']
assert o['objectives']==[False,True,True,False,False,False,False] and o['joined'] and not o['locked']
assert s['health']==10 and s['ammo']==1 and s['reserve']==141 and all(e['health']==0 for e in s['enemies'])
assert s['statistics']['kills']==5 and s['statistics']['shots']==19 and p['assists']==7
assert p['glass_hits']==5 and p['glass_health']==0 and 'WindowDestroyed' in o['events']
assert len(s['map']['events'])==win['audio_events']==android['audio_events']==57
scene=OUT/'geo_01c.rcplay';raw=scene.read_bytes();assert raw[:8]==b'RCPLAY14'
a=json.loads(raw[12:12+struct.unpack_from('<I',raw,8)[0]])
assert len(a['sounds'])==18 and len(a['opening']['planning']['factories'])==5
for mode in ['known','unknown']:
 r=read(WORK/f'scene-reader-{mode}.json');assert r['passed'] and r['heap_limit']==96*1024*1024 and r['sha256']==sha(scene)
tests=(WORK/'workspace-tests.log').read_text(encoding='utf-8-sig')
count=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',tests)));assert count==1114 and 'FAILED' not in tests
assert 'Finished `release`' in (WORK/'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log','whitespace.log']:assert not (WORK/name).read_text(encoding='utf-8-sig').strip()
unchanged={}
for milestone in ['mission15-complete','mission16-map2','mission17-map2-gameplay','mission18-map2-route','mission19-map3','mission20-map3-opening']:
 prior=read(ROOT/f'analysis/{milestone}/delivery/validation-manifest.json')
 for name,record in prior['evidence'].items():
  file=ROOT/name;assert file.stat().st_size==record['bytes'] and sha(file)==record['sha256'],name
 unchanged[milestone]=len(prior['evidence'])
assert sum(unchanged.values())==295
with zipfile.ZipFile(OUT/'republic-commando-0.38.0.apk') as z:
 for abi in ['arm64-v8a','x86_64']:
  for lib in ['librc_native.so','librc_bridge.so']:assert z.read(f'lib/{abi}/{lib}')==(ROOT/'android/app/src/main/jniLibs'/abi/lib).read_bytes()
assert "versionCode='42'" in (WORK/'apk-info.txt').read_text(encoding='utf-8-sig')
for name in ['workspace-tests.log','clippy.log','format.log','whitespace.log','android-build.log','native-build.log','scene-reader-known.json','scene-reader-unknown.json','apk-info.txt','playthrough.log','native-test.log','android-test.log','original-planning-receipt.json']:
 shutil.copyfile(WORK/name,OUT/name)
shutil.copyfile(ROOT/'target/release/rc_native.dll',OUT/'native/rc_native.dll')
shutil.copyfile(ROOT/'docs/MAP3_PLANNING.md',OUT/'README.md')
with zipfile.ZipFile(OUT/'original-bindings.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in [ROOT/'analysis/mission19-map3/mission-flow.json',ROOT/'analysis/mission19-map3/original-map-inventory.json',WORK/'original-planning-receipt.json']:z.write(p,p.name)
sources=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'README.md']
for folder,extensions in [('crates',{'.rs','.toml'}),('android',{'.java','.c','.xml','.gradle'}),('scripts',{'.py','.ps1','.c','.java'}),('docs',{'.md'})]:
 for p in (ROOT/folder).rglob('*'):
  if p.is_file() and p.suffix in extensions and not {'build','.gradle','.cxx'}.intersection(p.relative_to(ROOT/folder).parts):sources.append(p)
with zipfile.ZipFile(OUT/'source-snapshot.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in sorted(set(sources)):z.write(p,p.relative_to(ROOT).as_posix())
files=[p for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json']
result=dict(passed=True,version='0.38.0',version_code=42,original_completed_objectives=[1,2],complete_three_map_mission=False,
 planning_room_factories=[164,165,190,191,192],glass_original=263,glass_health=150,glass_faces=252,
 ordinary_map3_inputs=3357,player_shots=19,squad_assists=7,enemies_defeated=5,sounds=18,audio_events=57,
 actual_map1_inputs=18730,actual_map2_inputs=16414,library_tests=count,prior_delivery_evidence_unchanged=unchanged,
 limits=['Sun Fac escape movement, guard-room door states, SBD intro and objectives0/3/4/5/6 pending',
 'Original factory actors/health/extents; bounded AABB placement, hitscan, approach/attack rules and squad assistance',
 'Original glass destroy event; bounded collider retirement, no native Prop damage/collision parity or shatter effect/audio',
 'Original entry limits and pending native RootMotion/capsule, particles, weapon props and Karma parity remain',
 'No complete manual touch or physical ARM64 acceptance'],
 evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in sorted(files)})
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2)+'\n')
print(f'0.38.0 frozen: {len(files)} evidence files; {count} tests; 295 prior files unchanged')
