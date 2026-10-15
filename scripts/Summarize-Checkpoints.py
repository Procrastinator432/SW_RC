"""Freeze checkpoint/Continue implementation and persistence acceptance evidence."""
import hashlib,json,re,shutil,struct,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];WORK=ROOT/'analysis/mission29-checkpoints';OUT=WORK/'delivery'
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
r=read(OUT/'runtime/validation.json');w=read(OUT/'native/validation.json');a=read(OUT/'android/native-validation.json');u=read(OUT/'android/validation.json');store=read(WORK/'store-validation.json')
assert all(v['passed'] for v in [r,w,a,u,store])
assert len(r['checkpoints'])==25 and r['actual_a_b_c_completion'] and r['factory_transition'] and r['pending_input_and_fractional_time_exact'] and r['scene_mismatch_atomic']
assert all(v['exact_state_frame'] and v['atomic_corruption'] and v['restart_entry_exact'] for v in r['checkpoints'])
for p in (OUT/'runtime').glob('*.rcsave'):
 b=p.read_bytes();assert b[:8]==b'RCSAVE01' and struct.unpack_from('<I',b,8)[0]==len(b)-20
 assert json.loads(b[20:])['abi']=='rcport-state-0.46.0-1'
assert w['released_session_before_restore'] and w['exact_status_and_graphics'] and w['continuation_exact'] and w['carried_entry_restart_exact'] and w['invalid_checkpoints_atomic']==8 and w['wrong_scene_rejected']
assert a['separate_native_process'] and a['exact_status_and_graphics'] and a['continuation_exact'] and a['entry_restart_exact'] and a['corrupt_restore_atomic']
for key in ['periodic_autosave','background_save','actual_force_stop_and_continue_exact_state','private_scene_cache_without_picker','corrupt_primary_previous_checkpoint_recovery','corrupt_handoff_fresh_start_recovery','corrupt_files_quarantined','pause']:assert u[key],key
assert u['background_ticks']>=600 and u['background_health']==65 and u['background_ammo']<14
for key in ['two_generations','corrupt_primary_preserves_previous','interrupted_temp_ignored','scene_integrity_and_repair','truncation_and_path_escape_rejected']:assert store[key]
count=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(WORK/'workspace-tests.log').read_text(encoding='utf-8-sig'))));assert count==1120
assert 'Finished `release`' in (WORK/'clippy.log').read_text(encoding='utf-8-sig') and 'error:' not in (WORK/'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log','whitespace.log']:assert not (WORK/name).read_text(encoding='utf-8-sig').strip()
prior={}
for name in ['mission15-complete','mission16-map2','mission17-map2-gameplay','mission18-map2-route','mission19-map3','mission20-map3-opening','mission21-map3-planning','mission22-map3-escape','mission23-map3-guard','mission24-map3-breach','mission25-map3-balcony','mission26-map3-finale','mission27-map3-elevator','mission28-first-mission-complete']:
 m=read(ROOT/f'analysis/{name}/delivery/validation-manifest.json')
 for path,v in m['evidence'].items():
  p=ROOT/path;assert p.stat().st_size==v['bytes'] and sha(p)==v['sha256'],path
 prior[name]=len(m['evidence'])
assert sum(prior.values())==652
with zipfile.ZipFile(OUT/'republic-commando-0.46.0.apk') as z:
 for abi in ['arm64-v8a','x86_64']:
  for lib in ['librc_native.so','librc_bridge.so']:assert z.read(f'lib/{abi}/{lib}')==(ROOT/'android/app/src/main/jniLibs'/abi/lib).read_bytes()
assert "versionCode='50'" in (WORK/'apk-info.txt').read_text(encoding='utf-8-sig')
for name in ['runtime-test.log','native-build.log','native-test.log','android-build.log','android-test.log','workspace-tests.log','clippy.log','format.log','whitespace.log','store-validation.json','apk-info.txt']:
 shutil.copyfile(WORK/name,OUT/name)
shutil.copyfile(ROOT/'target/release/rc_native.dll',OUT/'native/rc_native.dll')
shutil.copyfile(ROOT/'docs/CHECKPOINTS.md',OUT/'README.md')
sources=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'README.md']
for folder,extensions in [('crates',{'.rs','.toml'}),('android',{'.java','.c','.xml','.gradle'}),('scripts',{'.py','.ps1','.c','.java'}),('docs',{'.md'})]:
 for p in (ROOT/folder).rglob('*'):
  if p.is_file() and p.suffix in extensions and not {'build','.gradle','.cxx'}.intersection(p.relative_to(ROOT/folder).parts):sources.append(p)
with zipfile.ZipFile(OUT/'source-snapshot.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in sorted(set(sources)):z.write(p,p.relative_to(ROOT).as_posix())
files=[p for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json']
scene_paths=['analysis/mission15-complete/delivery/geo_01a-first-mission.rcplay','analysis/mission18-map2-route/delivery/geo_01b.rcplay','analysis/mission28-first-mission-complete/delivery/geo_01c.rcplay','analysis/mission28-first-mission-complete/delivery/geo_03a.rcplay']
manifest=dict(passed=True,version='0.46.0',version_code=50,full_in_map_runtime_checkpoints=True,continue_after_actual_android_process_loss=True,checkpoint_samples=25,automatic_interval_ticks=600,two_atomic_generations=True,private_immutable_scene_cache=True,corrupt_checkpoint_and_handoff_visible_recovery=True,library_tests=count,checkpoint_format='RCSAVE01/rcport-state-0.46.0-1',
 reused_scenes={p:dict(bytes=(ROOT/p).stat().st_size,sha256=sha(ROOT/p)) for p in scene_paths},prior_delivery_evidence_unchanged=prior,
 limits=['Abrupt kill can lose progress after the latest successful checkpoint; normal background transition requests immediate save','Current and previous generation only; no arbitrary save-slot selector','In-flight PCM audio offset is not persisted; stale input/audio queues are cleared on resume','Checkpoint format/scene revision binding; incompatible versions use visible recovery rather than silent migration','Automated Android x86_64; physical ARM64 and complete manual touch mission acceptance remain open','Existing mission rendering/AI/physics parity limits unchanged'],
 evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in sorted(files)})
(OUT/'validation-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(f'0.46.0 frozen: {len(files)} evidence files; {sum(prior.values())} prior files unchanged')
