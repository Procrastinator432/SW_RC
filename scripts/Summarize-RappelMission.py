"""Freeze the selected live rappel delivery with explicit remaining intro boundaries."""
import hashlib
import json
import re
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission13-rappel/delivery'


def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()


def read(name):
    return json.loads((OUT / name).read_text(encoding='utf-8-sig'))


prior = json.loads((ROOT / 'analysis/mission12-opening/validation-manifest.json').read_text())
for name, row in prior['evidence'].items():
    p = ROOT / name
    assert p.stat().st_size == row['bytes'] and sha(p) == row['sha256'], name
for name, digest in prior['originals'].items():
    assert sha(Path(name)) == digest, name
scene = OUT / 'geo_01a-rappel.rcplay'
apk = OUT / 'republic-commando-0.31.0.apk'
raw = scene.read_bytes()
assert raw[:8] == b'RCPLAY13' and len(raw) == 108360967
n = int.from_bytes(raw[8:12], 'little')
profile = json.loads(raw[12:12+n])
old = (ROOT / 'analysis/mission12-opening/geo_01a-opening.rcplay').read_bytes()
old_n = int.from_bytes(old[8:12], 'little')
assert raw[12+n:] == old[12+old_n:]
assert len(profile['sounds']) == 15
assert len(profile['rappel']['notifies']) == 7
assert sum(n['role'] is not None for n in profile['rappel']['notifies']) == 5
assert profile['rappel']['eye_height'] == 60
check = read('playthrough/validation.json')
assert check['passed'] and check['normal_inputs_only']
assert check['opening_ticks'] == 1925 and check['injury_tick'] == 1397 and check['alignment_inputs'] == 165
for flag in ['locked_controls_tested', 'non_waiting_sound_overlap', 'restart_exact_state_frame', 'opening_144hz_events_equal']:
    assert check[flag], flag
rappel = check['rappel']
assert rappel['started'] and rappel['finished'] and rappel['rope_destroyed']
assert rappel['blocked_steps'] == 0 and rappel['finished_tick'] == 497
assert sorted(n for _, n in rappel['notify_events']) == list(range(225, 232))
state = check['complete_state']
assert state['phase'] == 'Won' and state['health'] == 20 and state['statistics']['ticks'] == 18730
assert state['statistics']['kills'] + state['squad']['kills'] == 8
old_check = read('old-scene-regression/validation.json')
assert old_check['passed'] and old_check['complete_state']['statistics']['ticks'] == 18565
native = read('native/validation.json')
android = read('android/native-replay.json')
for result in [native, android]:
    assert result['passed'] and result['opening_injury_health_checked'] and result['guarded_buffers'] and result['restart']
assert native['scene_sha256'] == android['scene.rcplay_sha256'] == sha(scene)
for i, name in enumerate(['playthrough/input-replay.bin', 'live/squad-assist-input.bin']):
    assert native['runs'][i]['input_sha256'] == sha(OUT / name)
    assert native['runs'][i]['inputs'] == android['runs'][i]['inputs'] == [18730, 16757][i]
    assert native['runs'][i]['status'][4] == android['runs'][i]['phase'] == [1, 0][i]
    assert native['runs'][i]['status'][0] == android['runs'][i]['health'] == 20
    assert native['runs'][i]['sound_counts'][76:80] == [1, 2, 1, 1]
assert android['runs'][1]['scorch_shots'] == 7 and android['runs'][1]['scorch_hits'] == 5 and android['runs'][1]['scorch_kills'] == 1
ui = read('android/validation.json')
for flag in ['passed', 'apk_scene_picker', 'gpu_render', 'touch_fire', 'touch_use', 'touch_crouch_height_change', 'pause']:
    assert ui[flag], flag
log = (OUT / 'android/app.log').read_text(encoding='utf-8')
assert 'bank sounds=80' in log and all(f' sound={r} ' in log for r in range(76, 80))
for mode in ['known', 'unknown']:
    row = read(f'scene-reader-{mode}.json')
    assert row['passed'] and row['sha256'] == sha(scene) and row['bytes'] == len(raw) and row['heap_limit'] == 112*1024*1024
log = (OUT / 'workspace-tests.log').read_text(encoding='utf-8-sig')
assert 'FAILED' not in log
tests = sum(map(int, re.findall(r'test result: ok\. (\d+) passed; 0 failed', log)))
assert tests == 1083
for name in ['format.log', 'source-diff-check.log']:
    assert not (OUT / name).read_text(encoding='utf-8-sig').strip()
log = (OUT / 'clippy.log').read_text(encoding='utf-8-sig')
assert 'Finished' in log and 'error:' not in log
assert 'BUILD SUCCESSFUL' in (OUT / 'android-build.log').read_text(encoding='utf-8-sig')
assert "versionCode='35' versionName='0.31.0-rappel'" in (OUT / 'apk-info.txt').read_text(encoding='utf-8-sig')
with ZipFile(apk) as archive:
    assert archive.testzip() is None
    for arch in ['arm64-v8a', 'x86_64']:
        for lib in ['librc_native.so', 'librc_bridge.so']:
            assert archive.read(f'lib/{arch}/{lib}') == (ROOT / f'android/app/src/main/jniLibs/{arch}/{lib}').read_bytes()
names = set(prior['sources'])
for folder in ['crates', 'docs', 'scripts', 'android/app/src/main/java']:
    names.update(p.relative_to(ROOT).as_posix() for p in (ROOT / folder).rglob('*')
                 if p.is_file() and p.suffix in ['.rs', '.md', '.py', '.ps1', '.java', '.toml', '.c'])
names.update(['README.md', 'Cargo.toml', 'Cargo.lock', 'android/app/build.gradle'])
sources = {name: sha(ROOT / name) for name in sorted(names)}
with ZipFile(OUT / 'source-snapshot.zip', 'w', ZIP_DEFLATED) as archive:
    for name in sources:
        archive.write(ROOT / name, name)
with ZipFile(OUT / 'source-snapshot.zip') as archive:
    assert archive.testzip() is None
    for name, digest in sources.items():
        assert hashlib.sha256(archive.read(name)).hexdigest() == digest
evidence = {p.relative_to(ROOT).as_posix(): dict(bytes=p.stat().st_size, sha256=sha(p))
            for p in sorted(OUT.rglob('*')) if p.is_file() and p.name != 'validation-manifest.json'
            and not {'java', 'java-cache'}.intersection(p.relative_to(OUT).parts)}
result = dict(passed=True, selected_live_rappel_milestone_complete=True,
              version='0.31.0', version_code=35, library_tests=tests, evidence=evidence,
              sources=sources, originals=prior['originals'], prior_evidence_unchanged=len(prior['evidence']),
              scene_sha256=sha(scene), apk_sha256=sha(apk), scene_bytes=len(raw), sound_bank=80,
              original_foley_samples_added=4, original_foley_events=5, original_notify_events=7,
              opening_ticks=1925, rappel_finished_tick=497, normal_inputs=18730, alignment_inputs=165,
              assist_inputs=16757, windows_android_native_replays=True, android_apk_gpu_audio_touch=True,
              entire_intro_complete=False, entire_first_mission_complete=False, next_map_loaded=False,
              physical_arm64_acceptance=False, complete_manual_touch_playthrough=False,
              limitations=['Selected EntranceRope600 player channel, first-person view, Rope7 lifetime and Foley; no full intro VM',
                           'Pre-animation hold, static body sweeps and final root pose once; no general PlayerStand/moveSmooth/Sphyll parity',
                           'EyeHeight/WalkBob/shake smoothing and material-dependent footstep sounds remain open',
                           'Intro Warrior/Trooper lifecycles, remaining LZ staging and general rope-animation receivers remain open',
                           'General AI/squad parity, GEO_01B loading/state transfer and whole first mission remain open'])
(OUT / 'validation-manifest.json').write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8')
print(f'Frozen live rappel0.31.0: {tests} tests, {len(evidence)} evidence files; prior delivery unchanged')
