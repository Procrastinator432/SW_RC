#!/usr/bin/env bash
set -euo pipefail
swrc_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$swrc_root/scripts/env-linux.sh"
if [[ ! -x "$swrc_root/tools/powershell/pwsh" ]]; then
    echo 'Run python3 scripts/Setup-Linux.py first.' >&2
    exit 1
fi
exec "$swrc_root/tools/powershell/pwsh" -NoLogo -NoProfile -File "$swrc_root/build.ps1" "$@"
