#Requires -Version 7.0
[CmdletBinding()]
param(
    [ValidateSet('Check','Android','Scenes','Package','All')][string]$Target='Check',
    [string]$ToolsDirectory=(Join-Path $PSScriptRoot 'tools'),
    [string]$OutputDirectory=(Join-Path $PSScriptRoot 'build'),
    [string]$SceneInputs,
    [string]$SceneStage='mission',
    [string]$GameData,
    [ValidateSet('arm64-v8a','x86_64')][string[]]$Abis=@('arm64-v8a','x86_64'),
    [switch]$Online
)
$ErrorActionPreference='Stop'
$root=$PSScriptRoot
$out=[IO.Path]::GetFullPath($OutputDirectory)
$logs=Join-Path $out 'logs'
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$steps=[Collections.Generic.List[object]]::new()
$report=[ordered]@{schema=1;target=$Target;passed=$false;source_commit=(& git -C $root rev-parse HEAD);dirty=[bool](& git -C $root status --porcelain);started_utc=[DateTime]::UtcNow.ToString('o');offline=(!$Online);default_test_stack=$true;steps=$steps}
$report.running=$true
$report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $out 'validation.json') -Encoding utf8
$saved=@{}
foreach($name in @('RUST_MIN_STACK','CARGO_INCREMENTAL')) {$saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
$env:CARGO_INCREMENTAL='0'
Remove-Item Env:RUST_MIN_STACK -ErrorAction SilentlyContinue
$cargoOptions=@('--locked');if(!$Online){$cargoOptions+='--offline'}
function Native([string]$File,[string[]]$Arguments) {
    & $File @Arguments
    if($LASTEXITCODE -ne 0){throw "$File failed (exit $LASTEXITCODE)"}
}
function Step([string]$Name,[scriptblock]$Action) {
    Write-Host "[$Name]"
    $log=Join-Path $logs "$Name.log"
    $item=[ordered]@{name=$Name;passed=$false;log=$log;started_utc=[DateTime]::UtcNow.ToString('o')}
    $steps.Add($item)
    try {& $Action *> $log;$item.passed=$true}
    catch {Get-Content -LiteralPath $log -Tail 18 -ErrorAction SilentlyContinue;throw}
    finally {$item.finished_utc=[DateTime]::UtcNow.ToString('o')}
}
Push-Location $root
try {
    $report.rustc=(& rustc --version)
    $report.cargo=(& cargo --version)
    if($Target -in @('Check','All')) {
        Step 'format' {Native cargo @('fmt','--all','--','--check')}
        Step 'debug-tests' {Native cargo (@('test','--workspace','--lib')+$cargoOptions)}
        Step 'release-tests' {Native cargo (@('test','--workspace','--lib','--release')+$cargoOptions)}
        Step 'doc-tests' {Native cargo (@('test','--workspace','--doc')+$cargoOptions)}
        Step 'clippy' {Native cargo (@('clippy','--workspace','--all-targets')+$cargoOptions+@('--','-D','warnings'))}
        Step 'native-build' {Native cargo (@('build','-p','rc-native','--release')+$cargoOptions)}
        Step 'inspector-build' {Native cargo (@('build','-p','rc-inspect','--bin','rc-inspect','--release')+$cargoOptions)}
        Step 'java-checks' {
            $jdk=if($env:JAVA_HOME -and (Test-Path (Join-Path $env:JAVA_HOME 'bin/javac.exe'))) {$env:JAVA_HOME}
                 elseif(Test-Path (Join-Path $ToolsDirectory 'jdk21')) {(Get-ChildItem (Join-Path $ToolsDirectory 'jdk21') -Directory | Sort-Object Name | Select-Object -First 1).FullName}
                 else {throw 'JDK missing: set JAVA_HOME or provide -ToolsDirectory (see docs/BUILD.md)'}
            $classes=Join-Path $out 'java/classes';$cache=Join-Path $out 'java/cache'
            New-Item -ItemType Directory -Force -Path $classes,$cache | Out-Null
            $sources=@('CampaignRoutes','CheckpointStore','SceneBytes','CampaignStore','TouchLayout') | ForEach-Object {Join-Path $root "android/app/src/main/java/org/rcport/diagnostic/$_.java";Join-Path $root "scripts/java/${_}Check.java"}
            Native (Join-Path $jdk 'bin/javac.exe') (@('-d',$classes)+$sources)
            $java=Join-Path $jdk 'bin/java.exe'
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.CampaignRoutesCheck')
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.CheckpointStoreCheck',(Join-Path $out 'java/store'))
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.CampaignStoreCheck',(Join-Path $out 'java/campaign'))
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.TouchLayoutCheck')
            # Synthetic 64 MiB payload: the reader is format-agnostic. Two full
            # copies cannot coexist under this heap; no copyrighted scene needed.
            $fixture=Join-Path $out 'java/reader-fixture.bin'
            $stream=[IO.File]::Open($fixture,[IO.FileMode]::Create)
            try {$stream.SetLength(64MB)} finally {$stream.Dispose()}
            foreach($mode in @('known','unknown')) {Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.SceneBytesCheck',$mode,$fixture,$cache)}
        }
        Step 'build-contract' {& (Join-Path $root 'scripts/Test-BuildContract.ps1') -ProjectRoot $root}
    }
    if($Target -in @('Android','All')) {
        Step 'android' {
            & (Join-Path $root 'scripts/Build-Android.ps1') -Abis $Abis -ToolsDirectory $ToolsDirectory -Offline:(!$Online)
            $gradleSource=Get-Content -LiteralPath (Join-Path $root 'android/app/build.gradle') -Raw
            $version=[regex]::Match($gradleSource,"versionName '([^']+)'").Groups[1].Value
            if(!$version){throw 'Missing Android versionName'}
            $apk=Join-Path $out "artifacts/republic-commando-$version.apk"
            New-Item -ItemType Directory -Force -Path (Split-Path $apk -Parent) | Out-Null
            Copy-Item -LiteralPath (Join-Path $root 'android/app/build/outputs/apk/debug/app-debug.apk') -Destination $apk
            $report.apk=[ordered]@{path=$apk;sha256=(Get-FileHash -LiteralPath $apk -Algorithm SHA256).Hash.ToLowerInvariant();abis=$Abis}
        }
    }
    if($Target -in @('Scenes','All')) {
        Step 'scenes' {
            if(!$SceneInputs){throw 'Scenes require -SceneInputs FILE; see config/scene-inputs.example.json and docs/BUILD.md'}
            & (Join-Path $root 'scripts/Build-Scenes.ps1') -Inputs $SceneInputs -Stage $SceneStage -GameData $GameData -OutputDirectory (Join-Path $out 'scenes') -Online:$Online
        }
    }
    if($Target -in @('Package','All')) {
        Step 'campaign-package' {
            $packageInputs=if($Target -eq 'All'){Join-Path $out 'scenes/resolved-inputs.json'}else{$SceneInputs}
            if(!$packageInputs){throw 'Package requires -SceneInputs FILE'}
            & (Join-Path $root 'scripts/Build-CampaignPackage.ps1') -Inputs $packageInputs -OutputDirectory (Join-Path $out 'artifacts')
        }
    }
    $report.passed=$true
    Write-Host "Build/check passed. Report: $(Join-Path $out 'validation.json')"
} catch {
    $report.error=$_.Exception.Message
    throw
} finally {
    $report.running=$false
    $report.finished_utc=[DateTime]::UtcNow.ToString('o')
    $report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $out 'validation.json') -Encoding utf8
    foreach($name in $saved.Keys){[Environment]::SetEnvironmentVariable($name,$saved[$name],'Process')}
    Pop-Location
}
