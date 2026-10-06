param([string]$Device='emulator-5554', [string]$OriginalMap='D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Maps\geo_01a.ctm')
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$adb=Join-Path $projectRoot 'tools\android-sdk\platform-tools\adb.exe'
$output=Join-Path $projectRoot 'analysis\emulator'
function Invoke-Adb([string[]]$Arguments) {
    & $adb -s $Device @Arguments
    if($LASTEXITCODE -ne 0){ throw "ADB failed: $Arguments" }
}
function Read-TestUi {
    Invoke-Adb @('shell','uiautomator','dump','/data/local/tmp/rc-start-ui.xml') | Out-Null
    [xml](Invoke-Adb @('shell','cat','/data/local/tmp/rc-start-ui.xml'))
}
function Tap-TestText([string]$Text) {
    $ui=Read-TestUi
    $node=$ui.SelectNodes('//node') | Where-Object text -EQ $Text | Select-Object -First 1
    if(!$node){throw "Control missing: $Text"}
    $c=[regex]::Matches($node.bounds,'\d+') | ForEach-Object { [int]$_.Value }
    Invoke-Adb @('shell','input','tap', [string][int](($c[0]+$c[2])/2), [string][int](($c[1]+$c[3])/2))
}
function Assert-TestText([string]$Prefix) {
    $ui=Read-TestUi
    $found=$ui.SelectNodes('//node') | Where-Object { $_.text.StartsWith($Prefix) }
    if(!$found){throw "Expected status missing: $Prefix"}
    $found | ForEach-Object { $_.text }
}
function Capture-TestFrame([string]$Name) {
    Read-TestUi | Out-Null
    Start-Sleep -Milliseconds 300
    Invoke-Adb @('pull','/data/local/tmp/rc-start-ui.xml',(Join-Path $output "$Name.xml")) | Out-Null
    Invoke-Adb @('shell','screencap','-p','/data/local/tmp/rc-start.png')
    Invoke-Adb @('pull','/data/local/tmp/rc-start.png',(Join-Path $output "$Name.png")) | Out-Null
}
function Scroll-TestFiles([bool]$ToTop) {
    $ui=Read-TestUi
    $node=$ui.SelectSingleNode('//node[@scrollable="true"]')
    if(!$node){throw 'File list is not scrollable'}
    $c=[regex]::Matches($node.bounds,'\d+') | ForEach-Object { [int]$_.Value }
    $x=[int](($c[0]+$c[2])/2); $y1=$c[1]+100; $y2=$c[3]-100
    if($ToTop){$swap=$y1; $y1=$y2; $y2=$swap}
    # Finger moves down to reveal earlier items, up to reveal later items.
    Invoke-Adb @('shell','input','swipe',[string]$x,[string]$y2,[string]$x,[string]$y1,'300')
}
function Load-TestScene([string]$Name) {
    Tap-TestText 'KARTE / SZENE ÖFFNEN'
    $ui=Read-TestUi
    if(!($ui.SelectNodes('//node') | Where-Object text -EQ $Name)) {
        Scroll-TestFiles $true; Scroll-TestFiles $true
        for($attempt=0;$attempt -lt 4;$attempt++) {
            $ui=Read-TestUi
            if($ui.SelectNodes('//node') | Where-Object text -EQ $Name){break}
            Scroll-TestFiles $false
        }
    }
    Tap-TestText $Name
}
if((Invoke-Adb @('shell','getprop','sys.boot_completed')) -ne '1'){throw 'Start the authorized test emulator first'}
Invoke-Adb @('install','-r',(Join-Path $projectRoot 'android\app\build\outputs\apk\debug\app-debug.apk'))
Invoke-Adb @('push',(Join-Path $projectRoot 'analysis\scenes\geo_01a-start\world.rcscene'),'/sdcard/Download/geo_01a-start.rcscene')
Invoke-Adb @('push',$OriginalMap,'/sdcard/Download/geo_01a.ctm')
Invoke-Adb @('push',(Join-Path $projectRoot 'analysis\scenes\geo_01a-textured\world.rcscene'),'/sdcard/Download/geo_01a-textured.rcscene')
Invoke-Adb @('push',(Join-Path $projectRoot 'analysis\emulator\truncated-start.rcscene'),'/sdcard/Download/truncated-start.rcscene')
Invoke-Adb @('shell','am','force-stop','org.rcport.diagnostic')
Invoke-Adb @('logcat','-c')
Invoke-Adb @('shell','am','start','-W','-f','0x10008000','-n','org.rcport.diagnostic/.MainActivity')
Load-TestScene 'geo_01a-start.rcscene'
Assert-TestText 'Original scene: 134790 triangles, 41 textures, 1 starts'
Capture-TestFrame 'start-overview'
Tap-TestText 'STARTPUNKT'; Assert-TestText 'Startpunkt 1 / 1'; Capture-TestFrame 'start-anchor'
Tap-TestText 'VOR'; Assert-TestText 'Original scene:'; Capture-TestFrame 'start-moved'
Tap-TestText 'STARTPUNKT'; Assert-TestText 'Startpunkt 1 / 1'; Capture-TestFrame 'start-returned'
Tap-TestText 'ÜBERSICHT'; Assert-TestText 'Original scene:'; Capture-TestFrame 'start-reset'
Load-TestScene 'geo_01a.ctm'; Assert-TestText 'SWRC'; Tap-TestText 'STARTPUNKT'
Assert-TestText 'Startpunkt 1 / 1'; Capture-TestFrame 'start-ctm'
Load-TestScene 'geo_01a-textured.rcscene'; Assert-TestText 'Original scene: 134790 triangles, 41 textures, 0 starts'
Tap-TestText 'STARTPUNKT'; Assert-TestText 'Kein unterstützter Startpunkt'; Capture-TestFrame 'start-legacy'
Load-TestScene 'truncated-start.rcscene'; Assert-TestText 'Fehler:'; Capture-TestFrame 'start-invalid'
Load-TestScene 'geo_01a-start.rcscene'; Assert-TestText 'Original scene:'; Capture-TestFrame 'start-recovery'
Invoke-Adb @('logcat','-d','-b','crash') | Out-File (Join-Path $output 'start-crash-log.txt') -Encoding utf8
Invoke-Adb @('shell','dumpsys','package','org.rcport.diagnostic') | Select-String 'versionCode=|versionName=' | Out-File (Join-Path $output 'start-version.txt') -Encoding utf8
Write-Output 'Start-anchor smoke test passed; compare captured render regions separately.'
