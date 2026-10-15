param([string]$OutputDirectory = 'build\input-probe')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$jdk = Join-Path $root 'tools\jdk21\jdk-21.0.12.1+1'
$sdk = Join-Path $root 'tools\android-sdk'
$buildTools = Join-Path $sdk 'build-tools\35.0.0'
$androidJar = Join-Path $sdk 'platforms\android-35\android.jar'
$out = Join-Path $root $OutputDirectory
New-Item -ItemType Directory -Force -Path $out,(Join-Path $out 'classes'),(Join-Path $out 'dex') | Out-Null
$env:JAVA_HOME = $jdk
& (Join-Path $jdk 'bin\javac.exe') -source 8 -target 8 -bootclasspath $androidJar -d (Join-Path $out 'classes') (Join-Path $PSScriptRoot 'android-input-probe\InputProbe.java')
if ($LASTEXITCODE -ne 0) {throw 'Input probe Java compilation failed'}
& (Join-Path $buildTools 'd8.bat') --min-api 26 --lib $androidJar --output (Join-Path $out 'dex') (Join-Path $out 'classes\org\rcport\inputprobe\InputProbe.class')
if ($LASTEXITCODE -ne 0) {throw 'Input probe DEX failed'}
& (Join-Path $buildTools 'aapt2.exe') link -I $androidJar --manifest (Join-Path $PSScriptRoot 'android-input-probe\AndroidManifest.xml') -o (Join-Path $out 'unsigned.apk')
if ($LASTEXITCODE -ne 0) {throw 'Input probe APK failed'}
& (Join-Path $jdk 'bin\jar.exe') uf (Join-Path $out 'unsigned.apk') -C (Join-Path $out 'dex') classes.dex
if ($LASTEXITCODE -ne 0) {throw 'Input probe packaging failed'}
& (Join-Path $buildTools 'apksigner.bat') sign --ks (Join-Path $env:USERPROFILE '.android\debug.keystore') --ks-key-alias androiddebugkey --ks-pass pass:android --key-pass pass:android --out (Join-Path $out 'input-probe.apk') (Join-Path $out 'unsigned.apk')
if ($LASTEXITCODE -ne 0) {throw 'Input probe signing failed'}
Write-Output (Join-Path $out 'input-probe.apk')
