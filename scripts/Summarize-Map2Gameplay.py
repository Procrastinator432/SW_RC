"""Freeze selected GEO_01B gameplay with explicit fixture/full-route boundaries."""
from PixelWords import pixel_words
import hashlib,json,re,shutil,zipfile,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];WORK=ROOT/'analysis/mission17-map2-gameplay';OUT=WORK/'delivery'
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
check=read(OUT/'check/validation.json');travel=read(OUT/'travel/validation.json')
win=read(OUT/'native/validation.json');native=read(OUT/'android/native-validation.json');ui=read(OUT/'android/validation.json')
assert all(r['passed'] for r in [check,travel,win,native,ui])
assert len(check['interaction_fixtures'])==5 and check['factories']==42 and check['restart_exact']
assert read(OUT/'check/exit.json')['map']['gameplay']['objectives']==[True]*8
assert check['exit']=='GEO_01C.ctm#playerstart'
assert travel['first_map_inputs']==win['real_map1_inputs']==native['real_map1_inputs']==18730
assert travel['restart_exact_state_and_frame'] and win['saved_restore_and_graphics_cache'] and native['saved_restore']
assert travel['entry']['health']==20 and travel['entry']['ammo']==13 and travel['entry']['reserve']==180
assert ui['apk_saved_handoff_resume'] and ui['original_weapon_audio']
scene=OUT/'geo_01b.rcplay';data=scene.read_bytes();assert data[:8]==b'RCPLAY14'
a=json.loads(data[12:12+struct.unpack_from('<I',data,8)[0]])
assert len(a['sounds'])==32 and len(a['gameplay']['objectives'])==8
assert len(a['gameplay']['factories'])==42 and len(a['gameplay']['gates'])==27
assert all(p>>24==255 for m in a['gameplay']['fixer_mesh']['materials'] for p in pixel_words(m['pixels']))
(OUT/'appearance-receipt.json').write_text(json.dumps(dict(fixer_original_skins=[m['source'] for m in a['gameplay']['fixer_mesh']['materials']],original_shader_without_opacity_uses_opaque_pixels=True,fixer_rifle_attachment='weaponAttach_R',hud_uses_uppercase_font=True),indent=2))
for mode in ['known','unknown']:
 r=read(WORK/f'scene-reader-{mode}.json');assert r['passed'] and r['heap_limit']==96*1024*1024 and r['sha256']==sha(scene)
tests=(WORK/'workspace-tests.log').read_text(encoding='utf-8-sig');count=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',tests)))
assert count==1097 and 'FAILED' not in tests
assert 'Finished `release`' in (WORK/'clippy.log').read_text(encoding='utf-8-sig')
for name in ['format.log','whitespace.log']:assert not (WORK/name).read_text(encoding='utf-8-sig').strip()
unchanged={}
for milestone in ['mission15-complete','mission16-map2']:
 prior=read(ROOT/f'analysis/{milestone}/delivery/validation-manifest.json')
 for name,record in prior['evidence'].items():
  p=ROOT/name;assert p.stat().st_size==record['bytes'] and sha(p)==record['sha256'],name
 unchanged[milestone]=len(prior['evidence'])
with zipfile.ZipFile(OUT/'republic-commando-0.34.0.apk') as z:
 assert all(f'lib/{abi}/librc_native.so' in z.namelist() for abi in ['arm64-v8a','x86_64'])
assert "versionCode='38'" in (WORK/'apk-info.txt').read_text(encoding='utf-8-sig')
for name in ['workspace-tests.log','clippy.log','format.log','whitespace.log','android-build.log','scene-reader-known.json','scene-reader-unknown.json','apk-info.txt','script-extraction-receipt.json']:
 shutil.copyfile(WORK/name,OUT/name)
shutil.copyfile(WORK/'surfaces/surface-receipt.json',OUT/'surface-receipt.json')
shutil.copyfile(ROOT/'target/release/rc_native.dll',OUT/'native/rc_native.dll')
with zipfile.ZipFile(OUT/'original-bindings.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in [WORK/'original-map-inventory.json',WORK/'gameplay-spec.json',WORK/'script-extraction-receipt.json',WORK/'surfaces/actor-faces.json']:
  z.write(p,p.relative_to(WORK).as_posix())
sources=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'README.md']
for folder,extensions in [('crates',{'.rs','.toml'}),('android',{'.java','.c','.xml','.gradle'}),('scripts',{'.py','.ps1','.c','.java'}),('docs',{'.md'})]:
 for p in (ROOT/folder).rglob('*'):
  if p.is_file() and p.suffix in extensions and not {'build','.gradle','.cxx'}.intersection(p.relative_to(ROOT/folder).parts):sources.append(p)
with zipfile.ZipFile(OUT/'source-snapshot.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in sorted(set(sources)):z.write(p,p.relative_to(ROOT).as_posix())
files=[p for p in OUT.rglob('*') if p.is_file() and p.name!='validation-manifest.json']
result=dict(passed=True,version='0.34.0',version_code=38,selected_original_map2_gameplay_integrated=True,
 full_natural_map2_playthrough_verified=False,map2_native_script_ai_equivalence=False,complete_three_map_mission=False,
 original_objectives=8,selected_scripts=31,original_dialogues=29,original_factories=42,interactions=5,dynamic_actors=27,
 map1_real_inputs=18730,library_tests=count,prior_delivery_evidence_unchanged=unchanged,
 original_map_sha256=sha(Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData/Maps/geo_01b.ctm')),
 limits=['Five actor-position/health fixtures and injected factory events; no continuous natural GEO_01B route acceptance',
 'Bounded ground AI, local hold command/slice and clear-floor spawn hosts; original animation/goal/projectile/squad parity incomplete',
 'Original shader/falloff FX and 12 null-material BSP surfaces pending; GEO_01C is a request only',
 'APK entry/GPU/touch/audio/pause and saved-resume smoke; no full physical ARM64 mission acceptance'],
 evidence={p.relative_to(ROOT).as_posix():dict(bytes=p.stat().st_size,sha256=sha(p)) for p in sorted(files)})
(OUT/'validation-manifest.json').write_text(json.dumps(result,indent=2)+'\n')
print(f"0.34.0 frozen: {len(files)} evidence files, {count} library tests, prior evidence unchanged {unchanged}")
