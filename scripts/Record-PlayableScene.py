"""Consolidate first-scene verification; append the giant evidence ledger without loading it."""
import hashlib
import json
import mmap
import re
import struct
import subprocess
import zipfile
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
PLAY = ROOT / 'analysis/playable'

def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as file:
        for block in iter(lambda: file.read(1024*1024), b''):
            digest.update(block)
    return digest.hexdigest()

test_log = (ROOT/'analysis/reports/playable-workspace-tests.log').read_text()
assert 'test result: FAILED' not in test_log
tests = sum(map(int, re.findall(r'test result: ok\. (\d+) passed', test_log)))
assert tests == 930, tests
assert 'Finished' in (ROOT/'analysis/reports/playable-clippy.log').read_text()
assert not (ROOT/'analysis/reports/playable-format.log').read_text().strip()
assert 'BUILD SUCCESSFUL' in (ROOT/'analysis/reports/playable-android-build.log').read_text()
assert 'Verified using v2 scheme (APK Signature Scheme v2): true' in (ROOT/'analysis/reports/playable-apksigner.log').read_text()
assert 'Verification succes' in (ROOT/'analysis/reports/playable-zipalign.log').read_text()
subprocess.run(['git','diff','--check'], cwd=ROOT, check=True, capture_output=True)

release = json.loads((PLAY/'validation/validation.json').read_text())
debug = json.loads((PLAY/'validation-debug/validation.json').read_text())
render_seconds = release.pop('render_seconds_640x400')
debug.pop('render_seconds_640x400')
assert release == debug
frames = {}
for frame in (PLAY/'validation').glob('*.ppm'):
    assert frame.read_bytes() == (PLAY/'validation-debug'/frame.name).read_bytes(), frame
    Image.open(frame).save(frame.with_suffix('.png'))
    frames[frame.name] = sha(frame)
assert len(frames) == 5
android = json.loads((PLAY/'android/validation.json').read_text())
for name in ['normal_picker_load','touch_aim_and_fire','three_kills','touch_move_and_exit_victory',
             'touch_jump','enemy_damage_and_defeat','touch_reload','restart','pause_frames_exact',
             'invalid_file_and_recovery','no_crash']:
    assert android[name] is True, name
abi = json.loads((PLAY/'abi-validation.json').read_text())
assert abi['restart_exact'] and abi['released_handle_rejected'] and abi['short_buffers_and_canaries']

apk = ROOT/'android/app/build/outputs/apk/debug/app-debug.apk'
elf = {}
with zipfile.ZipFile(apk) as archive:
    assert not any(name.lower().endswith(('.ctm','.usx','.utx','.rcplay','.rcscene')) for name in archive.namelist())
    for architecture, machine in [('arm64-v8a',183),('x86_64',62)]:
        for library in ['librc_native.so','librc_bridge.so']:
            name = f'lib/{architecture}/{library}'
            binary = archive.read(name)
            assert binary[:6] == b'\x7fELF\x02\x01'
            assert struct.unpack_from('<H',binary,18)[0] == machine
            offset = struct.unpack_from('<Q',binary,32)[0]
            size, count = struct.unpack_from('<HH',binary,54)
            assert size == 56 and 0 < count < 1024
            alignments = []
            for i in range(count):
                p_type, _, p_offset, address, _, _, _, alignment = struct.unpack_from('<IIQQQQQQ',binary,offset+i*size)
                if p_type == 1:
                    assert alignment >= 16384 and p_offset % alignment == address % alignment
                    alignments.append(alignment)
            assert alignments
            elf[name] = dict(machine=machine, load_alignments=alignments, sha256=hashlib.sha256(binary).hexdigest())

source_files = list((ROOT/'crates').rglob('*.rs')) + list((ROOT/'crates').rglob('Cargo.toml'))
source_files += [ROOT/p for p in ['Cargo.toml','Cargo.lock','README.md','docs/PLAYABLE_SCENE.md','docs/PORTING.md',
    'android/native/bridge.c','android/app/build.gradle','android/app/src/main/AndroidManifest.xml',
    'android/app/src/main/java/org/rcport/diagnostic/MainActivity.java',
    'android/app/src/main/java/org/rcport/diagnostic/PlayActivity.java',
    'scripts/Build-Android.ps1','scripts/Test-PlayableEmulator.py','scripts/Validate-PlayableAbi.py',
    'scripts/Record-PlayableScene.py']]
sources = {p.relative_to(ROOT).as_posix():sha(p) for p in sorted(set(source_files))}
game_data = Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData')
inputs = [game_data/'Maps/geo_01a.ctm', ROOT/'analysis/scenes/geo_01a-textured/world.rcscene']
for directory in ['System','Properties']:
    inputs.extend(p for p in (game_data/directory).iterdir() if p.suffix.lower()=='.u')
collision_receipt = ROOT/'analysis/collision/geo_01a-placement-probes.json'
actors = json.loads(collision_receipt.read_text())['actors']
packages = {a['result']['mesh'].split('.')[0].lower() for a in actors if 'result' in a}
mesh_files = {p.stem.lower():p for p in (game_data/'StaticMeshes').glob('*.usx')}
assert packages <= mesh_files.keys()
inputs.extend(mesh_files[name] for name in packages)
input_hashes = {str(p):sha(p) for p in sorted(set(inputs))}
evidence_files = [apk, PLAY/'geo_01a.rcplay', PLAY/'geo_01a.json', PLAY/'abi-validation.json',
    PLAY/'validation/validation.json',PLAY/'validation-debug/validation.json',PLAY/'android/validation.json']
evidence_files += list((PLAY/'android').glob('*.png')) + list((PLAY/'android').glob('*.log'))
evidence_files += [ROOT/'analysis/reports'/name for name in ['playable-workspace-tests.log','playable-clippy.log',
    'playable-format.log','playable-android-build.log','playable-apksigner.log','playable-zipalign.log']]
result = dict(date='2026-10-10', milestone='First playable bounded geo_01a Android prototype',
    scope='Original prepared environment/movement defaults/static collision. Provisional combat, robots and blaster; no original campaign/AI/GPU parity. Physical ARM64 and simultaneous multitouch unverified.',
    rust_tests=tests, debug_release_states_and_five_frames_identical=True, frame_sha256=frames,
    release_example_render_seconds_640x400=render_seconds,
    gameplay=release, native_abi=abi, android=android, elf=elf,
    source_sha256=sources, original_input_sha256=input_hashes,
    artifacts_sha256={p.relative_to(ROOT).as_posix():sha(p) for p in sorted(set(evidence_files))})
packed = json.dumps(result, indent=2, ensure_ascii=False).encode()
(PLAY/'validation-manifest.json').write_bytes(packed+b'\n')

ledger = ROOT/'analysis/evidence.json'
marker = b'\n  "playable_scene_validation": '
entry = marker + packed
with ledger.open('r+b') as file:
    with mmap.mmap(file.fileno(),0) as mapped:
        present = marker in mapped[max(0,len(mapped)-2*1024*1024):]
        at = mapped.find(b'\n  "rust_tests": ',0,1024*1024)
        assert at>=0
        start = at+len(b'\n  "rust_tests": ')
        assert mapped[start:start+3] in [b'920',b'930']
        mapped[start:start+3]=b'930'
        mapped.flush()
    if not present:
        file.seek(-3,2)
        tail=file.read(3)
        n=3 if tail==b'}\r\n' else 2
        assert tail.endswith(b'}\n') or tail==b'}\r\n'
        file.seek(-n,2)
        file.write(b','+entry+b'\n}\n')
with ledger.open('rb') as file:
    file.seek(-(len(entry)+8),2)
    tail=file.read().decode()
    fragment=tail.split(marker.decode())[-1]
    restored,end=json.JSONDecoder().raw_decode(fragment)
    assert restored==result and fragment[end:].strip()=='}'
print(json.dumps(dict(rust_tests=tests, source_hashes=len(sources), original_inputs=len(input_hashes),
    evidence_appended=not present, ledger_tail_verified=True, android=android)))
