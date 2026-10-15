"""Freeze the continuous GEO_01B route without modifying earlier deliveries."""
import hashlib,json,re,shutil,zipfile,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];WORK=ROOT/'analysis/mission18-map2-route';OUT=WORK/'delivery'
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
replay=read(OUT/'replay/validation.json');win=read(OUT/'native/validation.json');native=read(OUT/'android/native-validation.json');ui=read(OUT/'android/validation.json')
assert all(r['passed'] for r in [replay,win,native,ui])
assert replay['exact_final_state'] and replay['restart_exact_state_frame_and_collision']
assert replay['inputs']==win['full_map2_inputs']==native['full_map2_inputs']==16414
assert win['real_map1_inputs']==native['real_map1_inputs']==18730
assert win['explored']==[10,20,141,7,1,16414,112,35,0,23]
assert native['maximum_position_delta']<0.5 and native['position_tolerance']==0.5
assert native['player_health']==10 and native['kills']==35 and ui['apk_saved_handoff_resume']
scene=OUT/'geo_01b.rcplay';data=scene.read_bytes();assert data[:8]==b'RCPLAY14'
a=json.loads(data[12:12+struct.unpack_from('<I',data,8)[0]])
assert len(a['sounds'])==32 and len(a['gameplay']['objectives'])==8
assert len(a['gameplay']['factories'])==42 and len(a['gameplay']['gates'])==30
nav=a['gameplay']['navigation'];assert len(nav['nodes'])==345 and len(nav['edges'])==3562
for mode in ['known','unknown']:
 r=read(WORK/f'scene-reader-{mode}.json');assert r['passed'] and r['heap_limit']==96*1024*1024 and r['sha256']==sha(scene)
tests=(WORK/'workspace-tests.log').read_text(encoding='utf-8-sig');count=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',tests)))
assert count==1104 and 'FAILED' not in tests
assert 'Finished `release`' in (WORK/'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log','whitespace.log']:assert not (WORK/name).read_text(encoding='utf-8-sig').strip()
unchanged={}
for milestone in ['mission15-complete','mission16-map2','mission17-map2-gameplay']:
 prior=read(ROOT/f'analysis/{milestone}/delivery/validation-manifest.json')
 for name,record in prior['evidence'].items():
  p=ROOT/name;assert p.stat().st_size==record['bytes'] and sha(p)==record['sha256'],name
 unchanged[milestone]=len(prior['evidence'])
with zipfile.ZipFile(OUT/'republic-commando-0.35.0.apk') as z:
 for abi in ['arm64-v8a','x86_64']:
  for lib in ['librc_native.so','librc_bridge.so']:
   assert z.read(f'lib/{abi}/{lib}')==(ROOT/'android/app/src/main/jniLibs'/abi/lib).read_bytes()
assert "versionCode='39'" in (WORK/'apk-info.txt').read_text(encoding='utf-8-sig')
for name in ['workspace-tests.log','clippy.log','format.log','whitespace.log','android-build.log','scene-reader-known.json','scene-reader-unknown.json','apk-info.txt','input-replay.bin','final-state.json','validation.json','standing-graph.json','replay.log','android-test.log']:
 shutil.copyfile(WORK/name,OUT/name)
checkpoints=OUT/'checkpoints';checkpoints.mkdir(exist_ok=True)
for p in WORK.glob('*.json'):
 if p.name.startswith(('entrance-','fixer-','interaction-','retreat-','control-','hangar-','middle-','final-hallway','balcony')):shutil.copyfile(p,checkpoints/p.name)
shutil.copyfile(ROOT/'target/release/rc_native.dll',OUT/'native/rc_native.dll')
shutil.copyfile(ROOT/'docs/MAP2_CONTINUOUS_PLAYTHROUGH.md',OUT/'README.md')
with zipfile.ZipFile(OUT/'original-bindings.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in [ROOT/'analysis/mission17-map2-gameplay/original-map-inventory.json',ROOT/'analysis/mission17-map2-gameplay/gameplay-spec.json',ROOT/'analysis/mission17-map2-gameplay/script-extraction-receipt.json',WORK/'standing-graph.json']:
  z.write(p,p.name)
sources=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'README.md']
for folder,extensions in [('crates',{'.rs','.toml'}),('android',{'.java','.c','.xml','.gradle'}),('scripts',{'.py','.ps1','.c','.java'}),('docs',{'.md'})]:
 for p in (ROOT/folder).rglob('*'):
  if p.is_file() and p.suffix in extensions and not {'build','.gradle','.cxx'}.intersection(p.relative_to(ROOT/folder).parts):sources.append(p)
with zipfile.ZipFile(OUT/'source-snapshot.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in sorted(set(sources)):z.write(p,p.relative_to(ROOT).as_posix())
files=[p for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json']
result=dict(passed=True,version='0.35.0',version_code=39,full_natural_map2_playthrough_verified=True,manual_playthrough=False,
 map2_native_script_ai_equivalence=False,complete_three_map_mission=False,original_objectives=8,interactions=5,
 original_factories=42,naturally_activated_factories=39,total_spawned=42,kills=35,optional_enemies_alive=7,
 navigation_nodes=345,geometric_directed_edges=3562,follow_speed=450,original_reachspec_equivalence=False,
 collision_gates=30,original_sounds=32,map1_real_inputs=18730,map2_real_inputs=16414,library_tests=count,
 prior_delivery_evidence_unchanged=unchanged,limits=['Controller replay uses precise automated aim; no complete manual touch or physical ARM64 acceptance',
 'Bounded script/AI/squad/animation/projectile host; original parity incomplete',
 'Retired wreck collision follows selected event/lifetime timing; transient Karma physics/rendering incomplete',
 'Original shader/falloff FX and 12 null-material BSP surfaces pending; GEO_01C is a request only'],
 evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in sorted(files)})
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2)+'\n')
print(f"0.35.0 frozen: {len(files)} evidence files, {count} tests; prior unchanged {unchanged}")
