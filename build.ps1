#Requires -Version 7.0
[CmdletBinding()]
param(
    [ValidateSet('Check','Android','Scenes','Package','Soundtrack','Markers','WorkAnimations','WorkEquipment','Map2Voices','All')][string]$Target='Check',
    [string]$ToolsDirectory=(Join-Path $PSScriptRoot 'tools'),
    [string]$OutputDirectory=(Join-Path $PSScriptRoot 'build'),
    [string]$SceneInputs,
    [string]$SceneStage='mission',
    [string]$GameData,
    [string]$CampaignPackage,
    [string]$MarkerInventory,
    [string]$Python='python',
    [ValidateSet('arm64-v8a','x86_64')][string[]]$Abis=@('arm64-v8a','x86_64'),
    [switch]$Online
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'scripts/Host-Tools.ps1')
$root=$PSScriptRoot
$out=[IO.Path]::GetFullPath($OutputDirectory)
$logs=Join-Path $out 'logs'
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$steps=[Collections.Generic.List[object]]::new()
$report=[ordered]@{schema=1;target=$Target;passed=$false;source_commit=(& git -C $root rev-parse HEAD);dirty=[bool](& git -C $root status --porcelain);started_utc=[DateTime]::UtcNow.ToString('o');offline=(!$Online);default_test_stack=$true;steps=$steps}
$report.running=$true
$report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $out 'validation.json') -Encoding utf8
$saved=@{}
foreach($name in @('RUST_MIN_STACK','CARGO_INCREMENTAL','RC_OPENAL_SOFT_DLL')) {$saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
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
    if($Target -eq 'Map2Voices') {
        if(!$CampaignPackage -or !$GameData){throw 'Map2Voices requires -CampaignPackage and -GameData'}
        Step 'map2-voice-audit' {
            Native cargo (@('build','-p','rc-inspect','--bin','rc-map2-voice-audit','--release')+$cargoOptions)
            $source=Join-Path $out 'input/geo_01b.rcplay';New-Item -ItemType Directory -Force -Path (Split-Path $source -Parent) | Out-Null
            $archive=[IO.Compression.ZipFile]::OpenRead([IO.Path]::GetFullPath($CampaignPackage))
            try {$entry=$archive.GetEntry('geo_01b.rcplay');if(!$entry){throw 'Missing B scene'};[IO.Compression.ZipFileExtensions]::ExtractToFile($entry,$source,$true)}finally{$archive.Dispose()}
            Native (Join-Path $root "target/release/$(Get-RcExecutable 'rc-map2-voice-audit')") @($GameData,$source,(Join-Path $out 'assets'))
        }
        Step 'map2-voice-package' {Native $Python @('-X','utf8',(Join-Path $root 'scripts/Build-Map2Voices.py'),'--package',$CampaignPackage,'--assets',(Join-Path $out 'assets'),'--output',(Join-Path $out 'package'))}
    }
    if($Target -eq 'WorkEquipment') {
        if(!$CampaignPackage -or !$GameData -or !$MarkerInventory){throw 'WorkEquipment requires -CampaignPackage, -GameData and -MarkerInventory'}
        Step 'work-equipment-export' {
            Native cargo (@('build','-p','rc-inspect','--bin','rc-work-clips-export','--bin','rc-work-notify-audit','--bin','rc-work-equipment-export','--release')+$cargoOptions)
            $source=Join-Path $out 'input/geo_01b.rcplay';New-Item -ItemType Directory -Force -Path (Split-Path $source -Parent) | Out-Null
            $archive=[IO.Compression.ZipFile]::OpenRead([IO.Path]::GetFullPath($CampaignPackage))
            try {$entry=$archive.GetEntry('geo_01b.rcplay');if(!$entry){throw 'Missing B scene'};[IO.Compression.ZipFileExtensions]::ExtractToFile($entry,$source,$true)}finally{$archive.Dispose()}
            Native (Join-Path $root "target/release/$(Get-RcExecutable 'rc-work-clips-export')") @($GameData,(Join-Path $out 'clips'))
            Native (Join-Path $root "target/release/$(Get-RcExecutable 'rc-work-notify-audit')") @($GameData,(Join-Path $out 'clips/work-clips.json'),(Join-Path $out 'original-notifies.json'))
            Native (Join-Path $root "target/release/$(Get-RcExecutable 'rc-work-equipment-export')") @($GameData,$source,$MarkerInventory,(Join-Path $out 'original-notifies.json'),(Join-Path $out 'assets'))
        }
        Step 'work-equipment-package' {Native $Python @('-X','utf8',(Join-Path $root 'scripts/Build-WorkEquipment.py'),'--package',$CampaignPackage,'--assets',(Join-Path $out 'assets'),'--output',(Join-Path $out 'package'))}
    }
    if($Target -eq 'WorkAnimations') {
        if(!$CampaignPackage -or !$GameData){throw 'WorkAnimations requires -CampaignPackage and -GameData'}
        Step 'work-clips' {
            Native cargo (@('build','-p','rc-inspect','--bin','rc-work-clips-export','--release')+$cargoOptions)
            Native (Join-Path $root "target/release/$(Get-RcExecutable 'rc-work-clips-export')") @($GameData,(Join-Path $out 'clips'))
        }
        Step 'work-animation-package' {Native $Python @('-X','utf8',(Join-Path $root 'scripts/Build-WorkAnimations.py'),'--package',$CampaignPackage,'--clips',(Join-Path $out 'clips/work-clips.json'),'--output',(Join-Path $out 'package'))}
    }
    if($Target -eq 'Markers') {
        if(!$CampaignPackage -or !$MarkerInventory){throw 'Markers requires -CampaignPackage and -MarkerInventory'}
        Step 'map2-markers' {Native $Python @('-X','utf8',(Join-Path $root 'scripts/Build-Map2Markers.py'),'--package',$CampaignPackage,'--inventory',$MarkerInventory,'--output',(Join-Path $out 'package'))}
    }
    if($Target -eq 'Soundtrack') {
        if(!$GameData -or !$CampaignPackage){throw 'Soundtrack requires -GameData and -CampaignPackage'}
        Step 'music-audit' {
            Native cargo (@('build','--release','-p','rc-inspect','--bin','rc-music-audit')+$cargoOptions)
            Native (Join-Path $root "target/release/$(Get-RcExecutable 'rc-music-audit')") @($GameData,(Join-Path $out 'original-music.json'))
        }
        Step 'music-package' {Native $Python @('-X','utf8',(Join-Path $root 'scripts/Build-MissionSoundtrack.py'),'--game-data',$GameData,'--package',$CampaignPackage,'--audit',(Join-Path $out 'original-music.json'),'--output',(Join-Path $out 'package'))}
    }
    if($Target -in @('Check','All')) {
        Step 'format' {Native cargo @('fmt','--all','--','--check')}
        if($IsWindows){
            Step 'openal-dependency' {
                $dependency=& (Join-Path $root 'scripts/Setup-OpenALSoft.ps1') -ToolsDirectory (Join-Path $ToolsDirectory 'openal-soft') -Offline:(!$Online) | ConvertFrom-Json
                $env:RC_OPENAL_SOFT_DLL=$dependency.library
                $dependency | ConvertTo-Json
            }
        }
        Step 'debug-tests' {Native cargo (@('test','--workspace','--lib')+$cargoOptions)}
        Step 'release-tests' {Native cargo (@('test','--workspace','--lib','--release')+$cargoOptions)}
        Step 'material-import-tests' {Native cargo (@('test','-p','rc-inspect','--bin','rc-scene-material-build')+$cargoOptions)}
        Step 'doc-tests' {Native cargo (@('test','--workspace','--doc')+$cargoOptions)}
        Step 'clippy' {Native cargo (@('clippy','--workspace','--all-targets')+$cargoOptions+@('--','-D','warnings'))}
        Step 'native-build' {Native cargo (@('build','-p','rc-native','--release')+$cargoOptions)}
        Step 'inspector-build' {Native cargo (@('build','-p','rc-inspect','--bin','rc-inspect','--bin','rc-squad-check','--bin','rc-squad-live-check','--bin','rc-demolition-interrupt-check','--bin','rc-map2-orders-check','--bin','rc-breach-orders-check','--bin','rc-marker-execution-check','--bin','rc-work-clips-export','--bin','rc-work-animation-check','--bin','rc-work-notify-audit','--bin','rc-work-equipment-export','--bin','rc-work-equipment-check','--bin','rc-map2-voice-audit','--bin','rc-map2-voice-check','--release')+$cargoOptions)}
        Step 'java-checks' {
            $jdk=Get-RcJdk $ToolsDirectory
            $classes=Join-Path $out 'java/classes';$cache=Join-Path $out 'java/cache'
            New-Item -ItemType Directory -Force -Path $classes,$cache | Out-Null
            $sources=@('MissionSoundtrack','CampaignRoutes','CheckpointStore','CheckpointPolicy','AudioTimeline','SoundOwnership','AmbientLoops','SpatialAudio','AudioPlayback','FramePass','SceneBytes','CampaignStore','TouchLayout') | ForEach-Object {Join-Path $root "android/app/src/main/java/org/rcport/diagnostic/$_.java";Join-Path $root "scripts/java/${_}Check.java"}
            $sources+=@('PcmPacket','PcmPump') | ForEach-Object {Join-Path $root "android/app/src/main/java/org/rcport/diagnostic/$_.java";Join-Path $root "scripts/java/${_}Check.java"}
            $sources+=@('PcmLiveProducer') | ForEach-Object {Join-Path $root "android/app/src/main/java/org/rcport/diagnostic/$_.java";Join-Path $root "scripts/java/${_}Check.java"}
            $sources+=@('SceneAudioMailbox') | ForEach-Object {Join-Path $root "android/app/src/main/java/org/rcport/diagnostic/$_.java";Join-Path $root "scripts/java/${_}Check.java"}
            $sources+=@('SceneResumeCache') | ForEach-Object {Join-Path $root "android/app/src/main/java/org/rcport/diagnostic/$_.java";Join-Path $root "scripts/java/${_}Check.java"}
            $sources+=@('SceneAudioProducer') | ForEach-Object {Join-Path $root "android/app/src/main/java/org/rcport/diagnostic/$_.java";Join-Path $root "scripts/java/${_}Check.java"}
            $sources+=@('MissionMix') | ForEach-Object {Join-Path $root "android/app/src/main/java/org/rcport/diagnostic/$_.java";Join-Path $root "scripts/java/${_}Check.java"}
            $sources+=@('ScenePoolStatus') | ForEach-Object {Join-Path $root "android/app/src/main/java/org/rcport/diagnostic/$_.java";Join-Path $root "scripts/java/${_}Check.java"}
            $sources+=@('SceneAudioCapture','AudioCheckpointStore') | ForEach-Object {Join-Path $root "android/app/src/main/java/org/rcport/diagnostic/$_.java";Join-Path $root "scripts/java/${_}Check.java"}
            Native (Join-Path $jdk "bin/$(Get-RcExecutable 'javac')") (@('-d',$classes)+$sources)
            $java=Join-Path $jdk "bin/$(Get-RcExecutable 'java')"
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.PcmPacketCheck',(Join-Path $root 'fixtures/audio-pcm-packet.bin'))
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.PcmPumpCheck')
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.PcmLiveProducerCheck')
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.SceneAudioMailboxCheck')
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.SceneResumeCacheCheck')
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.MissionMixCheck')
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.ScenePoolStatusCheck')
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.SceneAudioCaptureCheck')
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.AudioCheckpointStoreCheck','check',(Join-Path $out 'java/audio-store'))
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.SceneAudioProducerCheck',(Join-Path $out 'java/mission-pause-ingress.bin'))
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.CampaignRoutesCheck')
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.MissionSoundtrackCheck')
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.CheckpointStoreCheck',(Join-Path $out 'java/store'))
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.CampaignStoreCheck',(Join-Path $out 'java/campaign'))
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.TouchLayoutCheck')
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.CheckpointPolicyCheck')
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.AudioTimelineCheck')
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.SoundOwnershipCheck')
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.AmbientLoopsCheck')
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.SpatialAudioCheck')
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.AudioPlaybackCheck')
            Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.FramePassCheck')
            # Synthetic 64 MiB payload: the reader is format-agnostic. Two full
            # copies cannot coexist under this heap; no copyrighted scene needed.
            $fixture=Join-Path $out 'java/reader-fixture.bin'
            $stream=[IO.File]::Open($fixture,[IO.FileMode]::Create)
            try {$stream.SetLength(64MB)} finally {$stream.Dispose()}
            foreach($mode in @('known','unknown')) {Native $java @('-Xmx96m','-cp',$classes,'org.rcport.diagnostic.SceneBytesCheck',$mode,$fixture,$cache)}
            foreach($mode in @('spool-known','spool-unknown')) {Native $java @('-Xmx16m','-cp',$classes,'org.rcport.diagnostic.SceneBytesCheck',$mode,$fixture,$cache)}
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
        Step 'gpu-frame-contract' {
            & (Join-Path $root 'scripts/Test-GpuFrame.ps1') -ToolsDirectory $ToolsDirectory -OutputDirectory (Join-Path $out 'gpu-frame-java')
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
