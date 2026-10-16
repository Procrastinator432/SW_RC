"""Freeze the verified alpha-blocker correction without claiming physical mission acceptance."""
import hashlib,json,re,shutil,subprocess,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];WORK=ROOT/'build/phone-blockers';OUT=ROOT/'build/releases/0.50.0'
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for block in iter(lambda:f.read(1048576),b''):h.update(block)
 return h.hexdigest()
assert not OUT.exists(),'Delivery already exists; do not overwrite frozen evidence'
reports={name:read(WORK/path) for name,path in {
 'checks':'checks-release/validation.json','campaign':'campaign-delivery/validation.json','checkpoints':'checkpoint-delivery/validation.json','entry_failure':'entry-failure/validation.json',
 'package':'package-validation.json','material':'material-validation/validation.json','runtime':'runtime-final/validation.json',
 'pipeline':'pipeline/scenes/validation.json','pipeline_runtime':'pipeline/scenes/runtime-validation/validation.json','android':'android-delivery/validation.json'}.items()}
assert all(r['passed'] for r in reports.values())
assert len(reports['runtime']['checkpoints'])==25 and len(reports['pipeline_runtime']['checkpoints'])==25
assert len(reports['pipeline']['results'])==14 and reports['pipeline']['mission_runtime_checked']
assert len(reports['package']['legacy_checkpoints'])==4
for name in ['debug-tests','release-tests']:
 text=(WORK/'checks-release/logs'/f'{name}.log').read_text(encoding='utf-8-sig');assert sum(map(int,re.findall(r'test result: ok\. (\d+) passed',text)))==1127
for key in ['quiet_grounded_play_creates_safe_checkpoint','background_and_combat_preserve_safe_and_manual','manual_load_restores_explicit_choice','android_background_process_kill','repeated_real_combat_deaths_counted_persistently','ordinary_defeat_safe_choice','corrupt_resume_safe_recovery']:assert reports['checkpoints'][key],key
assert len(reports['campaign']['automatic_transitions'])==3
apk=WORK/'android-delivery/artifacts/republic-commando-0.50.0-phone-fixes.apk';package=WORK/'package-final/artifacts/mission1-scenes.rcpack.zip'
assert sha(apk)==reports['campaign']['apk_sha256']==reports['checkpoints']['apk_sha256']==reports['android']['apk']['sha256']
assert sha(apk)==reports['entry_failure']['apk_sha256'] and reports['entry_failure']['entry_write_failure_preserves_entry_and_scene_usable']
assert sha(package)==reports['campaign']['package_sha256']==reports['package']['package_sha256']
with zipfile.ZipFile(apk) as z:
 for abi in ['arm64-v8a','x86_64']:
  for library in ['librc_native.so','librc_bridge.so']:assert z.read(f'lib/{abi}/{library}')==(ROOT/'android/app/src/main/jniLibs'/abi/library).read_bytes()
with zipfile.ZipFile(package) as z:
 assert z.namelist()[0]=='campaign.properties' and len(z.namelist())==5
 props=dict(line.split('=',1) for line in z.read('campaign.properties').decode('ascii').splitlines() if '=' in line)
 for map,key in [('GEO_01A','map_a_scene'),('GEO_01B','map_b_scene'),('GEO_01C','map_c_scene'),('GEO_03A','factory_scene')]:
  h=hashlib.sha256();size=0
  with z.open(props[map+'.file']) as f:
   for block in iter(lambda:f.read(1048576),b''):h.update(block);size+=len(block)
  assert h.hexdigest()==props[map+'.sha256']==reports['pipeline']['inputs'][key]['sha256'] and size==int(props[map+'.bytes'])
assert not subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True),'Commit reviewed source before freezing'
commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
assert subprocess.check_output(['git','rev-parse','v0.50.0^{}'],cwd=ROOT,text=True).strip()==commit
OUT.mkdir(parents=True)
for p in [apk,package]:shutil.copyfile(p,OUT/p.name)
shutil.copyfile(ROOT/'docs/PHONE_TEST_BLOCKERS.md',OUT/'README.md')
for name,report in reports.items():(OUT/f'{name}-validation.json').write_text(json.dumps(report,indent=2)+'\n')
shutil.copytree(WORK/'checks-release/logs',OUT/'check-logs')
for name in ['campaign-delivery','checkpoint-delivery','entry-failure','material-validation']:
 target=OUT/name;target.mkdir()
 for p in (WORK/name).iterdir():
  if p.suffix in ['.png','.xml','.log'] and not p.name.startswith(('failure','incomplete')):shutil.copyfile(p,target/p.name)
shutil.copyfile(WORK/'pipeline/logs/scenes.log',OUT/'scene-build.log')
for name,path in [('opacity-audit.json','scenes/map-a-opacity/opacity-audit.json'),('enemy-audio-a.json','scene-a/scenes/map-a-feedback/enemy-audio-binding.json'),('enemy-audio-b.json','scene-b/scenes/map-b-feedback/enemy-audio-binding.json')]:shutil.copyfile(WORK/path,OUT/name)
subprocess.run(['git','archive','--format=zip','-o',str(OUT/'source-snapshot.zip'),'HEAD'],cwd=ROOT,check=True)
files=sorted(p for p in OUT.rglob('*') if p.is_file())
manifest=dict(passed=True,version='0.50.0',version_code=54,commit=commit,tag='v0.50.0',mission_status='alpha; physical and complete visual/audio mission acceptance remains open',library_tests_debug=1127,library_tests_release=1127,checkpoint_abi='rcport-state-0.46.0-1',limits=reports['checkpoints']['limits']+['Full intro, gun-position readability, specialized enemies and texture-quality profile remain open'],evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in files})
(OUT/'validation-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');print(f'0.50.0 frozen: {len(files)} evidence files; source {commit[:7]}; mission remains alpha')
