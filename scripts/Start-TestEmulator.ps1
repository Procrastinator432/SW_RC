param(
    [string]$EmulatorPath = "$env:LOCALAPPDATA\Android\Sdk\emulator\emulator.exe",
    [switch]$Visible
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$env:ANDROID_HOME = Join-Path $projectRoot 'tools\android-sdk'
$env:ANDROID_USER_HOME = Join-Path $projectRoot 'tools\android-user'
$env:ANDROID_AVD_HOME = Join-Path $projectRoot 'tools\avd'
$systemImage = Join-Path $env:ANDROID_HOME 'system-images\android-35\google_apis\x86_64'
if (!(Test-Path -LiteralPath $EmulatorPath)) { throw "Emulator fehlt: $EmulatorPath" }
if (!(Test-Path -LiteralPath (Join-Path $env:ANDROID_AVD_HOME 'RC_Test_API35.ini'))) { throw 'Testgerät fehlt; siehe docs/EMULATOR.md.' }
$adb = Join-Path $env:ANDROID_HOME 'platform-tools\adb.exe'
$devices = & $adb devices
if ($devices -match '^emulator-5554\s') { throw 'Port 5554 ist bereits belegt; vorhandenes Gerät verwenden.' }
$launchArgs = @('-avd', 'RC_Test_API35', '-sysdir', ('"' + $systemImage + '"'), '-no-snapshot', '-gpu', 'software', '-port', '5554')
if (!$Visible) { $launchArgs += '-no-window' }
$launch = @{
    FilePath = $EmulatorPath
    ArgumentList = $launchArgs
    RedirectStandardOutput = (Join-Path $projectRoot 'analysis\emulator-stdout.log')
    RedirectStandardError = (Join-Path $projectRoot 'analysis\emulator-stderr.log')
    PassThru = $true
}
if (!$Visible) { $launch.WindowStyle = 'Hidden' }
$process = Start-Process @launch
Write-Output "Emulator PID $($process.Id). Android-Start dauert beim ersten Mal etwa eine Minute."
Write-Output "ADB: $adb"
