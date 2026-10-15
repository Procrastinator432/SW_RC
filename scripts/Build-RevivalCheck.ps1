$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$bin=Join-Path $root 'tools/android-sdk/ndk/27.2.12479018/toolchains/llvm/prebuilt/windows-x86_64/bin'
$env:CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=Join-Path $bin 'x86_64-linux-android26-clang.cmd'
$resource=& (Join-Path $bin 'clang.exe') --print-resource-dir
if($LASTEXITCODE -ne 0){throw 'NDK clang unavailable'}
$savedFlags=$env:CARGO_ENCODED_RUSTFLAGS
try {
    $env:CARGO_ENCODED_RUSTFLAGS=@('-L',"native=$(Join-Path $resource 'lib/linux/x86_64')",'-C','link-arg=-Wl,-z,max-page-size=16384') -join [char]31
    Push-Location $root
    try {
        cargo build -p rc-inspect --bin rc-squad-check --release --target x86_64-linux-android --locked --offline
        if($LASTEXITCODE -ne 0){throw 'Android revival fixture build failed'}
        Copy-Item -LiteralPath target/x86_64-linux-android/release/rc-squad-check -Destination analysis/mission11-revival/android-revival-check
    } finally {Pop-Location}
} finally {$env:CARGO_ENCODED_RUSTFLAGS=$savedFlags}
