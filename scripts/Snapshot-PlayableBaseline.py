"""Freeze the verified 0.8 scene before changes; never overwrite an existing baseline."""
import hashlib
import json
import shutil
import subprocess
import zipfile
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/baselines/0.8.0-playable-scene'


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def command(*args):
    return subprocess.run(args, cwd=ROOT, check=True, capture_output=True).stdout.decode('utf-8', 'replace').strip()


def main():
    if OUT.exists():
        raise SystemExit(f'Baseline already exists; preserved: {OUT}')
    evidence = json.loads((ROOT / 'analysis/playable/validation-manifest.json').read_text())
    # The prior manifest's exact source layout is inspected rather than assumed.
    sources = evidence['source_sha256']
    for name, expected in sources.items():
        assert sha(ROOT / name) == expected, f'0.8 source changed before snapshot: {name}'
    for name, expected in evidence['artifacts_sha256'].items():
        assert sha(ROOT / name) == expected, f'0.8 artifact changed before snapshot: {name}'
    names = command('git', 'ls-files', '-c', '-o', '--exclude-standard').splitlines()
    selected = set(sources)
    for name in names:
        if name.startswith(('crates/', 'docs/', 'scripts/')) or (
            name.startswith('android/') and not name.startswith(('android/build/', 'android/app/build/'))
        ) or '/' not in name:
            if (ROOT / name).is_file():
                selected.add(name)
    OUT.mkdir(parents=True)
    with zipfile.ZipFile(OUT / 'source.zip', 'w', zipfile.ZIP_DEFLATED) as archive:
        for name in sorted(selected):
            archive.write(ROOT / name, name)
    artifacts = [
        'android/app/build/outputs/apk/debug/app-debug.apk',
        'analysis/playable/geo_01a.rcplay', 'analysis/playable/geo_01a.json',
        'analysis/playable/validation-manifest.json', 'analysis/playable/abi-validation.json',
        'analysis/playable/validation/validation.json', 'analysis/playable/validation-debug/validation.json',
    ]
    artifacts += [p.relative_to(ROOT).as_posix() for p in (ROOT / 'analysis/playable/android').iterdir() if p.is_file()]
    artifacts += [p.relative_to(ROOT).as_posix() for p in (ROOT / 'analysis/reports').glob('playable-*.log')]
    for name in artifacts:
        dest = OUT / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / name, dest)
    source_hashes = {name: sha(ROOT / name) for name in sorted(selected)}
    with zipfile.ZipFile(OUT / 'source.zip') as archive:
        for name, expected in source_hashes.items():
            assert hashlib.sha256(archive.read(name)).hexdigest() == expected, name
    manifest = {
        'schema': 1, 'created_utc': datetime.now(timezone.utc).isoformat(),
        'version': '0.8.0-playable-scene', 'git_head': command('git', 'rev-parse', 'HEAD'),
        'git_status': command('git', 'status', '--short'),
        'scope': 'Local source snapshot including uncommitted files, verified APK/bundle and receipts. Original game installation and dependency/tool caches remain external.',
        'prior_manifest_sources_verified': len(sources), 'source_files': source_hashes,
        'artifacts': {name: sha(OUT / name) for name in artifacts},
        'source_zip_sha256': sha(OUT / 'source.zip'),
        'rustc': command('rustc', '-Vv'), 'cargo': command('cargo', '-V'),
        'tools': {'jdk': 'tools/jdk21/jdk-21.0.12.1+1', 'gradle': '8.11.1',
                  'agp': '8.9.2', 'sdk': 35, 'ndk': '27.2.12479018'},
    }
    (OUT / 'manifest.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
    print(json.dumps({'baseline': str(OUT), 'archived_sources': len(selected), 'copied_artifacts': len(artifacts)}))


if __name__ == '__main__':
    main()
