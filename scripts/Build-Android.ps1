param([ValidateSet('arm64-v8a', 'x86_64')][string[]]$Abis = @('arm64-v8a'))
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$tools = Join-Path $projectRoot 'tools'
$sdk = Join-Path $tools 'android-sdk'
$ndkBin = Join-Path $sdk 'ndk\27.2.12479018\toolchains\llvm\prebuilt\windows-x86_64\bin'
$env:JAVA_HOME = (Get-ChildItem (Join-Path $tools 'jdk21') -Directory | Select-Object -First 1).FullName
$env:ANDROID_HOME = $sdk
$env:GRADLE_USER_HOME = Join-Path $tools 'gradle-home'
Push-Location $projectRoot
try {
    foreach ($abi in $Abis) {
    $target = if ($abi -eq 'arm64-v8a') { 'aarch64-linux-android' } else { 'x86_64-linux-android' }
    $clang = Join-Path $ndkBin "${target}26-clang.cmd"
    if (!(Test-Path -LiteralPath $clang)) { throw 'Run scripts/Setup-Android.ps1 first' }
    $targetKey = $target.Replace('-', '_').ToUpperInvariant()
    [Environment]::SetEnvironmentVariable("CARGO_TARGET_${targetKey}_LINKER", $clang, 'Process')
    [Environment]::SetEnvironmentVariable("CARGO_TARGET_${targetKey}_RUSTFLAGS", '-C link-arg=-Wl,-z,max-page-size=16384', 'Process')
    $libs = Join-Path $projectRoot "android\app\src\main\jniLibs\$abi"
    cargo build -p rc-native --release --target $target --locked --offline
    if ($LASTEXITCODE -ne 0) { throw 'Rust Android build failed' }
    New-Item -ItemType Directory -Force -Path $libs | Out-Null
    Copy-Item -LiteralPath "target\$target\release\librc_native.so" -Destination $libs
    & $clang -shared -fPIC -O2 -Wall -Wextra -Werror '-Wl,-z,max-page-size=16384' '-Wl,--no-undefined' '-Wl,-soname,librc_bridge.so' (Join-Path $projectRoot 'android\native\bridge.c') "-L$libs" -lrc_native -o (Join-Path $libs 'librc_bridge.so')
    if ($LASTEXITCODE -ne 0) { throw 'JNI bridge build failed' }
    }
    $gradle = Join-Path $tools 'gradle-8.11.1\bin\gradle.bat'
    & $gradle -p (Join-Path $projectRoot 'android') "-PrcAbis=$($Abis -join ',')" --no-daemon assembleDebug
    if ($LASTEXITCODE -ne 0) { throw 'Android APK build failed' }
} finally { Pop-Location }
Write-Output (Join-Path $projectRoot 'android\app\build\outputs\apk\debug\app-debug.apk')
