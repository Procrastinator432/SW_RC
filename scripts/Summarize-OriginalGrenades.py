"""Bind pickup foundations and the explicitly failed physical route to local sources."""
import hashlib
import json
import re
import wave
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission3-pickup'
GAME = Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData')


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def read(name):
    return json.loads((OUT / name).read_text(encoding='utf-8-sig'))


flow = read('flow-validation.json')
assert flow['passed'] and all(v for k, v in flow.items() if k != 'scope')
assets = read('grenade-assets.json')
assert (assets['initial_ammo'], assets['pickup_amount'], assets['maximum_ammo']) == (0, 5, 5)
assert [s['original'] for s in assets['provenance']['scripts']] == [745, 747]
for i, cue in enumerate(assets['cues']):
    with wave.open(str(OUT / f'sounds/{i}.wav'), 'rb') as audio:
        assert abs(audio.getnframes() / audio.getframerate() - cue['seconds']) < 1e-12
        assert audio.getnchannels() > 0 and audio.getnframes() > 0
visual = read('visual-validation.json')
assert visual['passed'] and visual['snapshot_roundtrip']
assert visual['triangles'] == visual['textured_faces'] == 402
assert visual['untextured_faces'] == 0 and visual['textures'] == 1
assert visual['materials'][0]['opaque_coverage']
assert visual['materials'][0]['original_nonopaque_texels'] == 65167
route = read('route-validation.json')
without_wall = read('without-wall/route-validation.json')
assert not route['passed'] and not route['diagnostic_without_karma_wall']
assert not without_wall['passed'] and without_wall['diagnostic_without_karma_wall']
assert route['reachable_anchors'] == 15 and without_wall['reachable_anchors'] == 17
assert route['anchors'] == without_wall['anchors'] == 152
contacts = {c['object'] for link in read('blocked-route-links.json') for c in link['sweep']['contacts']}
assert {'KarmaProp4', 'KarmaProp7'} <= contacts
tests_log = (OUT / 'workspace-tests.log').read_text(encoding='utf-8-sig')
assert 'test result: FAILED' not in tests_log
tests = sum(map(int, re.findall(r'test result: ok\. (\d+) passed', tests_log)))
assert tests == 996, tests
assert 'Finished' in (OUT / 'clippy.log').read_text(encoding='utf-8-sig')
assert not (OUT / 'format.log').read_text(encoding='utf-8-sig').strip()
assert not (OUT / 'diff-check.log').read_text(encoding='utf-8-sig').strip()

previous_path = ROOT / 'analysis/mission2-av/validation-manifest.json'
previous = json.loads(previous_path.read_text())
for name, row in previous['evidence'].items():
    assert sha(ROOT / name) == row['sha256'], name
for name, digest in previous['originals'].items():
    assert sha(Path(name)) == digest, name
originals = dict(previous['originals'])
for name in ['Maps/geo_01a.ctm', 'System/engine.u', 'System/gameplay.u', 'System/ctgame.u',
             'System/ctinventory.u', 'Properties/properties.u', 'StaticMeshes/PickupMeshes.usx',
             'Textures/PickupTextures.utx', 'Sounds/GEO_Voice.uax', 'Sounds/GEN_Sound.uax']:
    originals[str(GAME / name)] = sha(GAME / name)

names = set(previous['sources'])
for folder in ['crates', 'docs', 'scripts', 'android/app/src/main/java']:
    names.update(p.relative_to(ROOT).as_posix() for p in (ROOT / folder).rglob('*')
                 if p.is_file() and p.suffix in ['.rs', '.md', '.py', '.ps1', '.java', '.toml'])
sources = {name: sha(ROOT / name) for name in sorted(names)}
with ZipFile(OUT / 'source-snapshot.zip', 'w', ZIP_DEFLATED) as archive:
    for name in sources:
        archive.write(ROOT / name, name)
with ZipFile(OUT / 'source-snapshot.zip') as archive:
    assert archive.testzip() is None
    for name, digest in sources.items():
        assert hashlib.sha256(archive.read(name)).hexdigest() == digest
evidence = {p.relative_to(ROOT).as_posix(): dict(bytes=p.stat().st_size, sha256=sha(p))
            for p in OUT.rglob('*') if p.is_file() and p.name != 'validation-manifest.json'}
manifest = dict(stage='original-objective2-foundation',
    scope='Original pickup event host, PCM samples, textured mesh preparation. Physical route fails: Karma wall and remaining collision transitions unresolved. No new playable scene/APK or Android acceptance; throwing and objective3 remain open.',
    checks=dict(workspace_library_tests=tests, new_pickup_tests=6, clippy=True, format=True,
                selected_original_flow=True, four_pcm_durations_verified=True,
                all_402_pickup_faces_textured=True, opaque_shader_coverage=True,
                physical_route_passed=False, android_tested=False,
                previous_evidence_unchanged=len(previous['evidence']), source_snapshot_verified=True),
    originals=originals, sources=sources, evidence=evidence,
    previous_delivery=dict(version='0.20.1', manifest_sha256=sha(previous_path)),
    next_step='Resolve original KnockOverWall and remaining collision/navigation path; then attach pickup host, visuals/audio/HUD and validate contiguous scene.')
(OUT / 'validation-manifest.json').write_bytes((json.dumps(manifest, indent=2) + '\n').encode())
print(f'{tests} tests, four original sounds, textured pickup; route explicitly OPEN; {len(previous["evidence"])} previous evidence files unchanged')
