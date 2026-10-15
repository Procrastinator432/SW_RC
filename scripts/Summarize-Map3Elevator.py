"""Freeze the original factory-elevator exit increment after its checks."""
import hashlib,json,re,shutil,struct,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];WORK=ROOT/'analysis/mission27-map3-elevator';OUT=WORK/'delivery'
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
route=read(OUT/'playthrough/validation.json');win=read(OUT/'native/validation.json');android=read(OUT/'android/native-validation.json');ui=read(OUT/'android/validation.json')
assert all(v['passed'] for v in [route,win,android,ui])
assert route['inputs']==win['map3_inputs']==android['map3_inputs']==11800
assert win['real_map1_inputs']==android['real_map1_inputs']==18730 and win['full_map2_inputs']==android['full_map2_inputs']==16414
assert win['ordinary_failure_without_sev'] and android['ordinary_failure_without_sev'] and win['failure_inputs']==android['failure_inputs']==10526
assert route['ordinary_input_replay_exact'] and route['exact_restart_state_and_frame'] and route['immutable_completed_session'] and route['wrong_destination_atomic'] and route['door_frame_rates_exact']==[20,60,144]
s=route['final'];o=s['map']['opening'];e=o['planning']['guard']['breach']['balcony']['finale']['elevator']
assert s['phase']=='Won' and o['objectives']==[True]*7 and e['door_fraction']==1 and e['approach_tick']<e['completed_tick']
assert s['statistics']['ticks']==11800 and s['statistics']['kills']==22 and s['statistics']['shots']==82
assert [s[k] for k in ['health','ammo','reserve']]==[65,14,65] and sum(v['health']>0 for v in s['enemies'])==1
assert len(s['map']['events'])==win['audio_events']==android['audio_events']==192
for directory in ['playthrough','native','android']:
 h=read(OUT/directory/'map3-handoff.json');assert h['schema']==3 and h['trigger']==39 and h['source']=='GEO_01C.ctm' and h['destination']=='GEO_03A.ctm#playerstart'
 assert h['map3']['objectives']==[True]*7 and h['squad']['joined'] and all(h['map3'][k]['joined'] for k in ['fixer','sev'])
 assert [h['player'][k] for k in ['health','ammo','reserve']]==[65,14,65]
scene=OUT/'geo_01c.rcplay';raw=scene.read_bytes();assert raw[:8]==b'RCPLAY14';n=struct.unpack_from('<I',raw,8)[0];assert n<32*1024*1024
for mode in ['known','unknown']:
 v=read(WORK/f'scene-reader-{mode}.json');assert v['passed'] and v['heap_limit']==96*1024*1024 and v['sha256']==sha(scene)
count=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(WORK/'workspace-tests.log').read_text(encoding='utf-8-sig'))));assert count==1120
assert 'Finished `release`' in (WORK/'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log','whitespace.log']:assert not (WORK/name).read_text(encoding='utf-8-sig').strip()
unchanged={}
for milestone in ['mission15-complete','mission16-map2','mission17-map2-gameplay','mission18-map2-route','mission19-map3','mission20-map3-opening','mission21-map3-planning','mission22-map3-escape','mission23-map3-guard','mission24-map3-breach','mission25-map3-balcony','mission26-map3-finale']:
 prior=read(ROOT/f'analysis/{milestone}/delivery/validation-manifest.json')
 for name,record in prior['evidence'].items():
  p=ROOT/name;assert p.stat().st_size==record['bytes'] and sha(p)==record['sha256'],name
 unchanged[milestone]=len(prior['evidence'])
assert sum(unchanged.values())==564
with zipfile.ZipFile(OUT/'republic-commando-0.44.0.apk') as z:
 for abi in ['arm64-v8a','x86_64']:
  for lib in ['librc_native.so','librc_bridge.so']:assert z.read(f'lib/{abi}/{lib}')==(ROOT/'android/app/src/main/jniLibs'/abi/lib).read_bytes()
assert "versionCode='48'" in (WORK/'apk-info.txt').read_text(encoding='utf-8-sig')
for name in ['build.log','build-receipt.json','playthrough.log','native-test.log','android-test.log','android-build.log','workspace-tests.log','clippy.log','format.log','whitespace.log','scene-reader-known.json','scene-reader-unknown.json','apk-info.txt']:
 p=WORK/name
 if p.exists():shutil.copyfile(p,OUT/name)
shutil.copyfile(ROOT/'target/release/rc_native.dll',OUT/'native/rc_native.dll')
shutil.copyfile(ROOT/'docs/MAP3_ELEVATOR.md',OUT/'README.md')
with zipfile.ZipFile(OUT/'original-bindings.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in [ROOT/'analysis/mission19-map3/mission-flow.json',ROOT/'analysis/mission19-map3/original-map-inventory.json',OUT/'build-receipt.json']:z.write(p,p.name)
sources=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'README.md']
for folder,extensions in [('crates',{'.rs','.toml'}),('android',{'.java','.c','.xml','.gradle'}),('scripts',{'.py','.ps1','.c','.java'}),('docs',{'.md'})]:
 for p in (ROOT/folder).rglob('*'):
  if p.is_file() and p.suffix in extensions and not {'build','.gradle','.cxx'}.intersection(p.relative_to(ROOT/folder).parts):sources.append(p)
with zipfile.ZipFile(OUT/'source-snapshot.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in sorted(set(sources)):z.write(p,p.relative_to(ROOT).as_posix())
files=[p for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json']
manifest=dict(passed=True,version='0.44.0',version_code=48,original_completed_objectives=list(range(7)),three_map_mission_to_exit39_complete=True,destination_geo03a_scene_loaded=False,
 actual_map1_inputs=18730,actual_map2_inputs=16414,ordinary_map3_inputs=11800,failure_inputs=10526,player_health=65,ammo=14,reserve=65,player_shots=82,kills=22,remaining_enemies=1,audio_events=192,
 library_tests=count,third_map_handoff_schema=3,three_squad_members_carried=True,prior_delivery_evidence_unchanged=unchanged,
 limits=['GEO_03A scene import, schema3 destination restore and actual scene replacement pending',
 'Bounded door proximity/interpolation/encroachment host; native mover state machine, antiportal and end icon pending',
 'Prior Sun Fac, sniper, warrior, AI/Squad/marker, grenade, healing, shader and effect parity limits remain',
 'Native full A/B/C replay plus APK entry/controls smoke; no complete manual touch or physical ARM64 acceptance'],
 evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in sorted(files)})
(OUT/'validation-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(f"0.44.0 frozen: {len(files)} evidence files; {sum(unchanged.values())} prior files unchanged")
