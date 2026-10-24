# Source this file from bash: source scripts/env-linux.sh
SWRC_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
export JAVA_HOME="$SWRC_ROOT/tools/jdk21/jdk-21.0.13+9"
export CARGO_HOME="$SWRC_ROOT/tools/cargo-home"
export GRADLE_USER_HOME="$SWRC_ROOT/tools/gradle-home"
export ANDROID_HOME="$SWRC_ROOT/tools/android-sdk"
export ANDROID_SDK_ROOT="$ANDROID_HOME"
export ANDROID_USER_HOME="$SWRC_ROOT/tools/android-user"
export GHIDRA_INSTALL_DIR="$SWRC_ROOT/tools/ghidra_12.1.4_PUBLIC"
export RC_GAME_DATA="${RC_GAME_DATA:-${XDG_DATA_HOME:-$HOME/.local/share}/Steam/steamapps/common/Star Wars Republic Commando/GameData}"
export POWERSHELL_TELEMETRY_OPTOUT=1
export DOTNET_CLI_TELEMETRY_OPTOUT=1
export DOTNET_CLI_HOME="$SWRC_ROOT/tools/dotnet-home"
export XDG_CACHE_HOME="$SWRC_ROOT/tools/cache"
export XDG_CONFIG_HOME="$SWRC_ROOT/tools/config"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export PATH="$SWRC_ROOT/tools/python/bin:$SWRC_ROOT/tools/powershell:$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin:$SWRC_ROOT/tools/gradle-8.11.1/bin:$PATH"
