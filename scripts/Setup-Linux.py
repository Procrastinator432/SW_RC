#!/usr/bin/env python3
"""Install the pinned development tools locally, without changing system packages."""
import argparse
import concurrent.futures
import hashlib
import os
from pathlib import Path
import platform
import subprocess
import tarfile
import zipfile
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
TOOLS = ROOT / 'tools'
ARCHIVES = [
    ('powershell-7.4.13-linux-x64.tar.gz',
     'https://github.com/PowerShell/PowerShell/releases/download/v7.4.13/powershell-7.4.13-linux-x64.tar.gz',
     '59e5df675dacbfe45374c32c1bf2480168a33243423c9b19252d0476fd1b748c', 'powershell', 'powershell/pwsh'),
    ('OpenJDK21U-jdk_x64_linux_hotspot_21.0.13_9.tar.gz',
     'https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.13%2B9/OpenJDK21U-jdk_x64_linux_hotspot_21.0.13_9.tar.gz',
     'd7f5d5cf865a2902b70ff549b97586a7d3f234e016f041ab447ea740c7d66d1c', 'jdk21', 'jdk21/jdk-21.0.13+9/bin/java'),
    ('ghidra_12.1.4_PUBLIC_20260921.zip',
     'https://github.com/NationalSecurityAgency/ghidra/releases/download/Ghidra_12.1.4_build/ghidra_12.1.4_PUBLIC_20260921.zip',
     'ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db', '.', 'ghidra_12.1.4_PUBLIC/support/analyzeHeadless'),
    ('gradle-8.11.1-bin.zip',
     'https://services.gradle.org/distributions/gradle-8.11.1-bin.zip',
     'f397b287023acdba1e9f6fc5ea72d22dd63669d59ed4a289a29b1a76eee151c6', '.', 'gradle-8.11.1/bin/gradle'),
    ('commandlinetools-linux-15859902_latest.zip',
     'https://dl.google.com/android/repository/commandlinetools-linux-15859902_latest.zip',
     '4e4c464f145a7512b57d088ac6c278c03c9eea610886b35a5e0804e74eedf583', 'android-commandline', 'android-sdk/cmdline-tools/latest/bin/sdkmanager'),
]


def run(*args, **kwargs):
    print('+', ' '.join(map(str, args)), flush=True)
    subprocess.run(list(map(str, args)), cwd=ROOT, check=True, **kwargs)


def download(url, partial):
    # Retain partial downloads, and use a few independent ranges on large files.
    with urllib.request.urlopen(urllib.request.Request(url, method='HEAD'), timeout=30) as response:
        size = int(response.headers.get('Content-Length', '0'))
        ranges = response.headers.get('Accept-Ranges', '').lower() == 'bytes'
    offset = partial.stat().st_size if partial.exists() else 0
    if not ranges or size < 64 * 1024 * 1024:
        run('curl', '--fail', '--location', '--silent', '--show-error', '--retry', '3',
            '--connect-timeout', '30', '--speed-limit', '1024', '--speed-time', '45',
            '--max-time', '1200', '--continue-at', '-', url, '--output', partial)
        return
    if offset > size:
        raise RuntimeError(f'Partial download is larger than its source: {partial}')
    pieces = []
    for start in range(offset, size, 16 * 1024 * 1024):
        end = min(start + 16 * 1024 * 1024, size) - 1
        pieces.append((start, end, partial.with_name(partial.name + f'.{start}-{end}')))

    def fetch(piece):
        start, end, target = piece
        if target.exists() and target.stat().st_size == end - start + 1:
            return
        result = subprocess.run(['curl', '--fail', '--location', '--silent', '--show-error',
            '--http1.1', '--retry', '3', '--connect-timeout', '30', '--max-time', '300',
            '--speed-limit', '1024', '--speed-time', '45', '--range', f'{start}-{end}',
            '--write-out', '%{http_code}', url, '--output', str(target)],
            capture_output=True, text=True)
        if result.returncode:
            raise RuntimeError(f'Download failed for {target.name}: {result.stderr.strip()}')
        if result.stdout != '206' or target.stat().st_size != end - start + 1:
            raise RuntimeError(f'Server did not honor the requested download range: {target}')

    print(f'Downloading {partial.name}: {offset // 1048576}/{size // 1048576} MiB; 2 connections', flush=True)
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        list(pool.map(fetch, pieces))
    with partial.open('ab') as stream:
        for _, _, target in pieces:
            with target.open('rb') as chunk:
                while data := chunk.read(1024 * 1024):
                    stream.write(data)
    for _, _, target in pieces:
        target.unlink()


def install(archive):
    name, url, expected, directory, executable = archive
    if (TOOLS / executable).is_file():
        return
    cache = TOOLS / 'downloads' / name
    cache.parent.mkdir(parents=True, exist_ok=True)
    if not cache.exists():
        partial = cache.with_name(name + '.partial')
        download(url, partial)
        partial.replace(cache)
    with cache.open('rb') as stream:
        actual = hashlib.file_digest(stream, 'sha256').hexdigest()
    if actual != expected:
        raise RuntimeError(f'SHA-256 mismatch: {cache}; remove the invalid archive before retrying')
    destination = TOOLS / directory
    destination.mkdir(parents=True, exist_ok=True)
    if name.endswith('.tar.gz'):
        with tarfile.open(cache) as bundle:
            bundle.extractall(destination, filter='data')
    else:
        with zipfile.ZipFile(cache) as bundle:
            for member in bundle.infolist():
                target = (destination / member.filename).resolve()
                if not target.is_relative_to(destination.resolve()):
                    raise RuntimeError(f'Unsafe archive entry: {member.filename}')
                bundle.extract(member, destination)
                if not member.is_dir():
                    mode = (member.external_attr >> 16) & 0o777
                    if mode:
                        target.chmod(mode)
    if name.startswith('commandlinetools-'):
        target = TOOLS / 'android-sdk/cmdline-tools/latest'
        target.parent.mkdir(parents=True, exist_ok=True)
        (destination / 'cmdline-tools').rename(target)
    (TOOLS / executable).chmod((TOOLS / executable).stat().st_mode | 0o111)
    print(f'Installed and verified: {name}', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--android', action='store_true', help='Install SDK/NDK and Rust Android targets')
    parser.add_argument('--accept-sdk-licenses', action='store_true', help='Only use with explicit user consent')
    args = parser.parse_args()
    if platform.system() != 'Linux' or platform.machine() != 'x86_64':
        parser.error('This setup is pinned for Linux x86_64')
    if args.accept_sdk_licenses and not args.android:
        parser.error('--accept-sdk-licenses requires --android')
    selected = ARCHIVES if args.android else ARCHIVES[:3]
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        list(pool.map(install, selected))
    env = os.environ.copy()
    env.update(JAVA_HOME=str(TOOLS / 'jdk21/jdk-21.0.13+9'),
               CARGO_HOME=str(TOOLS / 'cargo-home'),
               GRADLE_USER_HOME=str(TOOLS / 'gradle-home'),
               ANDROID_USER_HOME=str(TOOLS / 'android-user'))
    env['PATH'] = str(Path(env['JAVA_HOME']) / 'bin') + os.pathsep + env['PATH']
    venv = TOOLS / 'python'
    if not (venv / 'bin/python').exists():
        run('python3', '-m', 'venv', venv)
    run(venv / 'bin/python', '-m', 'pip', 'install', '--disable-pip-version-check',
        '--cache-dir', TOOLS / 'pip-cache', '-r', ROOT / 'config/analysis-requirements.txt')
    run('cargo', 'fetch', '--locked', env=env)
    if args.android:
        sdk = TOOLS / 'android-sdk'
        manager = sdk / 'cmdline-tools/latest/bin/sdkmanager'
        if args.accept_sdk_licenses:
            run(manager, f'--sdk_root={sdk}', '--licenses', input='y\n' * 50, text=True, env=env)
        else:
            run(manager, f'--sdk_root={sdk}', '--licenses', env=env)
        run(manager, f'--sdk_root={sdk}', 'platform-tools', 'platforms;android-35',
            'build-tools;35.0.0', 'ndk;27.2.12479018', env=env)
        run('rustup', 'target', 'add', 'aarch64-linux-android', 'x86_64-linux-android')
    print('Setup complete. Run: source scripts/env-linux.sh', flush=True)


if __name__ == '__main__':
    main()
