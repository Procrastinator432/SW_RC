param([string]$OutputDirectory=(Join-Path (Split-Path $PSScriptRoot -Parent) 'build/revival'))
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'Host-Tools.ps1')
$root=Split-Path $PSScriptRoot -Parent
$bin=Join-Path $root "tools/android-sdk/ndk/27.2.12479018/toolchains/llvm/prebuilt/$(Get-RcNdkHost)/bin"
$clangName=if($IsWindows){'x86_64-linux-android26-clang.cmd'}else{'x86_64-linux-android26-clang'}
$previousLinker=$env:CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER
$env:CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=Join-Path $bin $clangName
$resource=& (Join-Path $bin (Get-RcExecutable 'clang')) --print-resource-dir
if($LASTEXITCODE -ne 0){throw 'NDK clang unavailable'}
$savedFlags=$env:CARGO_ENCODED_RUSTFLAGS
try {
    $env:CARGO_ENCODED_RUSTFLAGS=@('-L',"native=$(Join-Path $resource 'lib/linux/x86_64')",'-C','link-arg=-Wl,-z,max-page-size=16384') -join [char]31
    Push-Location $root
    try {
        cargo build -p rc-inspect --bin rc-squad-check --release --target x86_64-linux-android --locked --offline
        if($LASTEXITCODE -ne 0){throw 'Android revival fixture build failed'}
        New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
        Copy-Item -LiteralPath target/x86_64-linux-android/release/rc-squad-check -Destination (Join-Path $OutputDirectory 'android-revival-check')
    } finally {Pop-Location}
} finally {
    $env:CARGO_ENCODED_RUSTFLAGS=$savedFlags
    $env:CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=$previousLinker
}
