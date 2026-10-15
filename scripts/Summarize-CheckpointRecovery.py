"""Freeze Android checkpoint recovery hardening; never rewrite prior deliveries."""
import hashlib
import json
import re
import shutil
import zipfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
WORK=ROOT/'analysis/mission30-checkpoint-recovery';OUT=WORK/'delivery'
def read(path):return json.loads(path.read_text(encoding='utf-8-sig'))
def sha(path):
 h=hashlib.sha256()
 with path.open('rb') as f:
  for block in iter(lambda:f.read(1024*1024),b''):h.update(block)
 return h.hexdigest()

store=read(WORK/'store-validation.json');android=read(OUT/'android/validation.json');mission=read(OUT/'android-mission/validation.json')
assert store['passed'] and android['passed'] and mission['passed']
for key in ['primary_and_backup_write_failures_preserve_both_generations','staged_payload_cleanup','retry_after_write_failure','duplicate_preserves_previous']:assert store[key]
for key in ['previous_recovery_immediately_promoted','actual_write_failure_preserves_both_generations','retry_after_write_failure','rejected_scene_preserves_recovery_scene','actual_force_stop_and_continue_exact_state','corrupt_handoff_fresh_start_recovery']:assert android[key]
assert android['repeated_cold_continue_cycles']==3
samples=mission['actual_mission_checkpoints'];assert [s['tick'] for s in samples]==[3400,7300,10000]
for sample in samples:
 for key in ['exact_state_after_stale_audio_drain','exact_entry','paused_clock_stable','actual_kill_and_continue_exact']:assert sample[key]
 assert sample['background_ticks']>sample['tick']
assert 'BUILD SUCCESSFUL' in (WORK/'android-build.log').read_text(encoding='utf-8-sig')
assert "versionCode='51'" in (WORK/'apk-info.txt').read_text(encoding='utf-8-sig')
assert not (WORK/'whitespace.log').read_text(encoding='utf-8-sig').strip()
prior={}
for path in sorted((ROOT/'analysis').glob('mission*/delivery/validation-manifest.json')):
 if path.parent==OUT:continue
 if int(re.match(r'mission(\d+)',path.parent.parent.name)[1])<15:continue
 manifest=read(path)
 # Keep the same frozen-delivery baseline used by the 0.46.0 release.
 if 'evidence' not in manifest:continue
 for name,value in manifest['evidence'].items():
  file=ROOT/name
  assert file.stat().st_size==value['bytes'] and sha(file)==value['sha256'],name
 prior[path.parent.parent.name]=len(manifest['evidence'])
assert sum(prior.values())==704,prior
old_apk=ROOT/'analysis/mission29-checkpoints/delivery/republic-commando-0.46.0.apk'
native={}
with zipfile.ZipFile(old_apk) as old,zipfile.ZipFile(OUT/'republic-commando-0.47.0.apk') as new:
 for abi in ['arm64-v8a','x86_64']:
  for library in ['librc_native.so','librc_bridge.so']:
   name=f'lib/{abi}/{library}';data=new.read(name)
   assert data==old.read(name)==(ROOT/'android/app/src/main/jniLibs'/abi/library).read_bytes()
   native[name]=hashlib.sha256(data).hexdigest()
for name in ['android-build.log','android-test.log','mission-resume-test.log','store-validation.json','apk-info.txt','whitespace.log']:shutil.copyfile(WORK/name,OUT/name)
shutil.copyfile(ROOT/'docs/CHECKPOINT_RECOVERY.md',OUT/'README.md')
sources=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'README.md']
for folder,extensions in [('crates',{'.rs','.toml'}),('android',{'.java','.c','.xml','.gradle'}),('scripts',{'.py','.ps1','.c','.java'}),('docs',{'.md'})]:
 for path in (ROOT/folder).rglob('*'):
  if path.is_file() and path.suffix in extensions and not {'build','.gradle','.cxx'}.intersection(path.relative_to(ROOT/folder).parts):sources.append(path)
with zipfile.ZipFile(OUT/'source-snapshot.zip','w',zipfile.ZIP_DEFLATED) as archive:
 for path in sorted(set(sources)):archive.write(path,path.relative_to(ROOT).as_posix())
files=sorted(p for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json')
result=dict(passed=True,version='0.47.0',version_code=51,checkpoint_abi_unchanged='rcport-state-0.46.0-1',recovery_immediately_persistent=True,rejected_scene_preserves_recovery_reference=True,staged_before_backup_rotation=True,android_cold_continue_cycles=3,android_mission_checkpoint_ticks=[s['tick'] for s in samples],native_libraries_identical_to_046=native,prior_delivery_evidence_unchanged=prior,
 reused_native_validation='analysis/mission29-checkpoints/delivery/validation-manifest.json',
 limits=['Android x86_64 automated; physical ARM64 and complete manual touch mission run remain open','Write failures use blocked staging paths; actual storage exhaustion and power loss not exercised','Mission checkpoint fixtures came from genuine replays and were supplied directly to private test storage','Existing PCM-position and original engine parity limits unchanged'],
 evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in files})
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2)+'\n')
print(f'0.47.0 frozen: {len(files)} evidence files; {sum(prior.values())} prior files unchanged; native libraries byte-identical to 0.46.0')
