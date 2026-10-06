param([switch]$AcceptSdkLicenses)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$tools = Join-Path $projectRoot 'tools'
$sdk = Join-Path $tools 'android-sdk'
$env:JAVA_HOME = (Get-ChildItem (Join-Path $tools 'jdk21') -Directory | Select-Object -First 1).FullName
$env:PATH = (Join-Path $env:JAVA_HOME 'bin') + ';' + $env:PATH
$sdkManager = Join-Path $sdk 'cmdline-tools\latest\bin\sdkmanager.bat'
if (!(Test-Path -LiteralPath $sdkManager)) {
    $zip = Join-Path $tools 'android-commandline.zip'
    Invoke-WebRequest 'https://dl.google.com/android/repository/commandlinetools-win-15859902_latest.zip' -OutFile $zip
    if ((Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash -ne '90AE805D20434428BFFCB699C290860F19BB5F66A67E6B330067E3DE801FB04A') { throw 'SDK tools checksum mismatch' }
    $extract = Join-Path $tools 'android-commandline-extracted'
    Expand-Archive -LiteralPath $zip -DestinationPath $extract -Force
    New-Item -ItemType Directory -Path (Join-Path $sdk 'cmdline-tools') -Force | Out-Null
    $source = [IO.Path]::GetFullPath((Join-Path $extract 'cmdline-tools'))
    $destination = [IO.Path]::GetFullPath((Join-Path $sdk 'cmdline-tools\latest'))
    if (!$source.StartsWith($tools + '\') -or !$destination.StartsWith($tools + '\')) { throw 'Unexpected tool path' }
    Move-Item -LiteralPath $source -Destination $destination
}
if ($AcceptSdkLicenses) { 1..40 | ForEach-Object { 'y' } | & $sdkManager "--sdk_root=$sdk" --licenses }
else { & $sdkManager "--sdk_root=$sdk" --licenses }
if ($LASTEXITCODE -ne 0) { throw 'SDK licenses were not accepted' }
& $sdkManager "--sdk_root=$sdk" 'platform-tools' 'platforms;android-35' 'build-tools;35.0.0' 'ndk;27.2.12479018'
if ($LASTEXITCODE -ne 0) { throw 'SDK install failed' }
$gradle = Join-Path $tools 'gradle-8.11.1\bin\gradle.bat'
if (!(Test-Path -LiteralPath $gradle)) {
    $zip = Join-Path $tools 'gradle-8.11.1-bin.zip'
    $checksumContent = (Invoke-WebRequest 'https://services.gradle.org/distributions/gradle-8.11.1-bin.zip.sha256').Content
    $checksum = if ($checksumContent -is [byte[]]) { [Text.Encoding]::UTF8.GetString($checksumContent).Trim() } else { ([string]$checksumContent).Trim() }
    Invoke-WebRequest 'https://services.gradle.org/distributions/gradle-8.11.1-bin.zip' -OutFile $zip
    if ((Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash -ne $checksum) { throw 'Gradle checksum mismatch' }
    Expand-Archive -LiteralPath $zip -DestinationPath $tools -Force
}
rustup target add aarch64-linux-android
if ($LASTEXITCODE -ne 0) { throw 'Rust Android target installation failed' }
Write-Output 'Android ARM64 toolchain installed'
