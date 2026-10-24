#!/usr/bin/env python3
"""Report local toolchain readiness separately from historical mission inputs."""
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent
TOOLS = ROOT / 'tools'


def version(command):
    try:
        result = subprocess.run(command, capture_output=True, text=True, timeout=30)
        return {'passed': result.returncode == 0,
                'version': (result.stdout + result.stderr).strip()[:2000]}
    except (OSError, subprocess.TimeoutExpired) as error:
        return {'passed': False, 'error': str(error)}


def main():
    report = {'schema': 1, 'platform': 'linux-x86_64'}
    report['tools'] = {
        'rustc': version(['rustc', '--version']),
        'cargo': version(['cargo', '--version']),
        'java': version([str(TOOLS / 'jdk21/jdk-21.0.13+9/bin/java'), '-version']),
        'powershell': version([str(TOOLS / 'powershell/pwsh'), '-NoLogo', '-NoProfile',
                               '-Command', '$PSVersionTable.PSVersion.ToString()']),
    }
    report['analysis_packages'] = {}
    for name in ['unicorn', 'pefile', 'capstone']:
        try:
            report['analysis_packages'][name] = importlib.metadata.version(name)
        except importlib.metadata.PackageNotFoundError:
            report['analysis_packages'][name] = None
    game_data = Path(os.environ['RC_GAME_DATA'])
    report['original_binaries'] = {}
    for name, expected in {
        'engine.dll': 'f8b3fc9d62414828a1c31d8b5f3b2bcb951f7c0f142fdf2a9846c98d7b5d35a5',
        'msvcr71.dll': '8094af5ee310714caebccaeee7769ffb08048503ba478b879edfef5f1a24fefe',
    }.items():
        path = game_data / 'System' / name
        actual = hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
        report['original_binaries'][name] = {'sha256': actual, 'matches_reference': actual == expected}
    report['ghidra_installed'] = (TOOLS / 'ghidra_12.1.4_PUBLIC/support/analyzeHeadless').is_file()
    report['android_files'] = {str(path): (TOOLS / path).is_file() for path in [
        Path('gradle-8.11.1/bin/gradle'), Path('android-sdk/platform-tools/adb'),
        Path('android-sdk/platforms/android-35/android.jar'),
        Path('android-sdk/build-tools/35.0.0/aapt2'),
        Path('android-sdk/ndk/27.2.12479018/toolchains/llvm/prebuilt/linux-x86_64/bin/clang'),
    ]}
    targets = subprocess.check_output(['rustup', 'target', 'list', '--installed'], text=True).splitlines()
    report['rust_targets'] = targets
    report['source_work_ready'] = all(t['passed'] for t in report['tools'].values())
    report['decompilation_ready'] = (report['source_work_ready'] and report['ghidra_installed']
        and all(report['analysis_packages'].values())
        and all(t['matches_reference'] for t in report['original_binaries'].values()))
    report['android_build_ready'] = (report['source_work_ready'] and all(report['android_files'].values())
        and {'aarch64-linux-android', 'x86_64-linux-android'} <= set(targets))
    manifest = ROOT / 'local-assets/mission1/scene-inputs.json'
    report['mission_manifest_present'] = manifest.is_file()
    report['full_mission_build_ready'] = False
    if manifest.is_file():
        entries = json.loads(manifest.read_text(encoding='utf-8-sig'))['inputs']
        recipes = json.loads((ROOT / 'config/scene-builds.json').read_text())
        available = set()
        required = {'map_a_replay', 'map_b_replay', 'map_c_replay'}
        for stage in recipes['mission']:
            recipe = recipes['stages'][stage]
            required.update(set(recipe['inputs']) - available)
            available.add(recipe['output_key'])
        valid = {}
        for key in sorted(required):
            entry = entries.get(key)
            if not entry:
                valid[key] = False
                continue
            path = manifest.parent / entry['path']
            valid[key] = (path.is_file() and hashlib.sha256(path.read_bytes()).hexdigest() == entry['sha256'])
        report['mission_inputs'] = valid
        report['full_mission_build_ready'] = report['android_build_ready'] and all(valid.values())
    output = ROOT / 'build/linux-setup/environment.json'
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
