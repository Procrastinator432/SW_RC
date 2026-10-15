"""Freeze the verified campaign menu APK, one-time scene package and UI evidence."""
import hashlib,json,re,shutil,subprocess,zipfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1];WORK=ROOT/'build/campaign';OUT=ROOT/'build/releases/0.49.0'
def read(path):return json.loads(path.read_text(encoding='utf-8-sig'))
def sha(path):
 h=hashlib.sha256()
 with path.open('rb') as f:
  for block in iter(lambda:f.read(1024*1024),b''):h.update(block)
 return h.hexdigest()

assert not (OUT/'validation-manifest.json').exists(),'Release is already frozen'
android=read(WORK/'android/validation.json');checks=read(WORK/'checks/validation.json');memory=read(WORK/'package-memory-validation.json')
assert android['passed'] and checks['passed'] and memory['passed'] and memory['heap_limit']<=100663296
for key in ['actual_package_picker_and_native_import','import_preserves_checkpoint','invalid_import_preserves_package_and_save','cold_start_keeps_package','new_game_without_picker','android_background_kill_recreates_original_intent_exactly','all_touch_targets_min_48dp','touch_movement_fire_jump','sensitivity_saved_and_density_independent','both_look_axes_verified']:assert android[key],key
assert android['logical_display']==[640,360] and android['viewport_dp']==[640,312]
assert len(android['automatic_transitions'])==3 and all(r['carried_entry_exact'] and r['no_file_picker'] for r in android['automatic_transitions'])
for name in ['debug-tests','release-tests']:
 text=(WORK/'checks/logs'/f'{name}.log').read_text(encoding='utf-8-sig');assert sum(map(int,re.findall(r'test result: ok\. (\d+) passed',text)))==1124
java=(WORK/'checks/logs/java-checks.log').read_text(encoding='utf-8-sig')
for key in ['four_maps_imported','native_rejection_atomic','reimport_repairs_cache_preserves_checkpoint','minimum_48dp','no_clipping_or_overlap','density_independent_look']:assert f'"{key}":true' in java,key
apk=WORK/'artifacts/republic-commando-0.49.0-campaign.apk';package=WORK/'artifacts/mission1-scenes.rcpack.zip'
assert sha(apk)==android['apk_sha256'] and sha(package)==android['package_sha256']
with zipfile.ZipFile(apk) as z:
 for abi in ['arm64-v8a','x86_64']:
  for library in ['librc_native.so','librc_bridge.so']:assert z.read(f'lib/{abi}/{library}')==(ROOT/'android/app/src/main/jniLibs'/abi/library).read_bytes()
with zipfile.ZipFile(package) as z:
 assert z.namelist()[0]=='campaign.properties' and len(z.namelist())==5
 properties=dict(line.split('=',1) for line in z.read('campaign.properties').decode('ascii').splitlines() if '=' in line)
 for map in ['GEO_01A','GEO_01B','GEO_01C','GEO_03A']:
  with z.open(properties[map+'.file']) as f:
   h=hashlib.sha256();size=0
   for block in iter(lambda:f.read(1024*1024),b''):h.update(block);size+=len(block)
  assert h.hexdigest()==properties[map+'.sha256'] and size==int(properties[map+'.bytes'])
assert not subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)
commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
assert subprocess.check_output(['git','rev-parse','v0.49.0^{}'],cwd=ROOT,text=True).strip()==commit
OUT.mkdir(parents=True,exist_ok=True)
for source in [apk,package]:shutil.copyfile(source,OUT/source.name)
shutil.copyfile(ROOT/'docs/CAMPAIGN_MENU.md',OUT/'README.md')
for name in ['android','checks']:
 target=OUT/name;target.mkdir()
 source=WORK/name
 if name=='android':
  for p in source.iterdir():
   if p.is_file() and p.name not in ['failure.png','failure.xml','incomplete.rcpack.zip']:shutil.copyfile(p,target/p.name)
 else:
  shutil.copyfile(source/'validation.json',target/'validation.json');shutil.copytree(source/'logs',target/'logs')
for name in ['android-test.log','package-memory-validation.json','whitespace.log']:shutil.copyfile(WORK/name,OUT/name)
subprocess.run(['git','archive','--format=zip','-o',str(OUT/'source-snapshot.zip'),'HEAD'],cwd=ROOT,check=True)
files=sorted(p for p in OUT.rglob('*') if p.is_file())
manifest=dict(passed=True,version='0.49.0',version_code=53,commit=commit,tag='v0.49.0',one_time_package_import=True,automatic_map_transitions=android['automatic_transitions'],small_display_dp=[640,360],library_tests_debug=1124,library_tests_release=1124,checkpoint_abi_unchanged='rcport-state-0.46.0-1',limits=android['limits'],evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in files})
(OUT/'validation-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(f'0.49.0 frozen: {len(files)} evidence files; APK and all four maps verified; source {commit[:7]}')
