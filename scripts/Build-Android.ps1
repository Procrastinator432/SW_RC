param(
    [ValidateSet('arm64-v8a', 'x86_64')][string[]]$Abis = @('arm64-v8a'),
    [string]$ToolsDirectory = (Join-Path (Split-Path $PSScriptRoot -Parent) 'tools'),
    [switch]$Offline=$true
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$tools = [IO.Path]::GetFullPath($ToolsDirectory)
$sdk = Join-Path $tools 'android-sdk'
$ndkBin = Join-Path $sdk 'ndk\27.2.12479018\toolchains\llvm\prebuilt\windows-x86_64\bin'
$savedEnvironment=@{}
foreach($key in @('JAVA_HOME','ANDROID_HOME','GRADLE_USER_HOME','CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER','CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER')) {$savedEnvironment[$key]=[Environment]::GetEnvironmentVariable($key,'Process')}
$env:JAVA_HOME = (Get-ChildItem (Join-Path $tools 'jdk21') -Directory | Select-Object -First 1).FullName
$env:ANDROID_HOME = $sdk
$env:GRADLE_USER_HOME = Join-Path $tools 'gradle-home'
$previousEncodedRustFlags = $env:CARGO_ENCODED_RUSTFLAGS
Push-Location $projectRoot
try {
    foreach ($abi in $Abis) {
    $target = if ($abi -eq 'arm64-v8a') { 'aarch64-linux-android' } else { 'x86_64-linux-android' }
    $clang = Join-Path $ndkBin "${target}26-clang.cmd"
    if (!(Test-Path -LiteralPath $clang)) { throw 'Run scripts/Setup-Android.ps1 first' }
    $targetKey = $target.Replace('-', '_').ToUpperInvariant()
    [Environment]::SetEnvironmentVariable("CARGO_TARGET_${targetKey}_LINKER", $clang, 'Process')
    # Recent Rust Android std explicitly links libunwind. NDK r27 keeps it in the
    # architecture-specific clang resource directory, outside clang's default -L set.
    $resourceDirectory = & (Join-Path $ndkBin 'clang.exe') --print-resource-dir
    if ($LASTEXITCODE -ne 0) { throw 'Cannot locate NDK clang resources' }
    $unwindArchitecture = if ($abi -eq 'arm64-v8a') { 'aarch64' } else { 'x86_64' }
    $unwindDirectory = Join-Path $resourceDirectory "lib\linux\$unwindArchitecture"
    if (!(Test-Path -LiteralPath (Join-Path $unwindDirectory 'libunwind.a'))) { throw 'NDK unwinder missing' }
    # Unit separators preserve spaces in the project path without shell quoting.
    $env:CARGO_ENCODED_RUSTFLAGS = @('-L', "native=$unwindDirectory", '-C', 'link-arg=-Wl,-z,max-page-size=16384') -join [char]31
    $libs = Join-Path $projectRoot "android\app\src\main\jniLibs\$abi"
    $cargoOptions=@('--locked');if($Offline){$cargoOptions+='--offline'}
    cargo build -p rc-native --release --target $target @cargoOptions
    if ($LASTEXITCODE -ne 0) { throw 'Rust Android build failed' }
    New-Item -ItemType Directory -Force -Path $libs | Out-Null
    Copy-Item -LiteralPath "target\$target\release\librc_native.so" -Destination $libs
    & $clang -shared -fPIC -O2 -Wall -Wextra -Werror '-Wl,-z,max-page-size=16384' '-Wl,--no-undefined' '-Wl,-soname,librc_bridge.so' (Join-Path $projectRoot 'android\native\bridge.c') "-L$libs" -lrc_native -o (Join-Path $libs 'librc_bridge.so')
    if ($LASTEXITCODE -ne 0) { throw 'JNI bridge build failed' }
    }
    $gradle = Join-Path $tools 'gradle-8.11.1\bin\gradle.bat'
    $gradleOptions=@('-p',(Join-Path $projectRoot 'android'),"-PrcAbis=$($Abis -join ',')",'--no-daemon')
    if($Offline){$gradleOptions+='--offline'}
    & $gradle @gradleOptions assembleDebug
    if ($LASTEXITCODE -ne 0) { throw 'Android APK build failed' }
} finally {
    $env:CARGO_ENCODED_RUSTFLAGS = $previousEncodedRustFlags
    foreach($key in $savedEnvironment.Keys){[Environment]::SetEnvironmentVariable($key,$savedEnvironment[$key],'Process')}
    Pop-Location
}
Write-Output (Join-Path $projectRoot 'android\app\build\outputs\apk\debug\app-debug.apk')
