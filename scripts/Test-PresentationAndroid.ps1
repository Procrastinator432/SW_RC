#Requires -Version 7.0
param(
    [Parameter(Mandatory)][string]$Inputs,
    [Parameter(Mandatory)][string]$PresentationReport,
    [string]$OutputDirectory=(Join-Path (Split-Path $PSScriptRoot -Parent) 'build/presentation-android'),
    [ValidatePattern('^[a-zA-Z0-9_.:-]+$')][string]$Serial='emulator-5554'
)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$out=[IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $out,(Join-Path $out 'native') | Out-Null
@{schema=1;passed=$false;started_utc=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $out 'validation.json') -Encoding utf8
$inputsPath=(Resolve-Path -LiteralPath $Inputs).Path
$doc=Get-Content -LiteralPath $inputsPath -Raw | ConvertFrom-Json -AsHashtable
$presentation=Get-Content -LiteralPath $PresentationReport -Raw | ConvertFrom-Json
if(!$presentation.passed){throw 'A passed contiguous desktop presentation report is required'}
$files=@{}
foreach($key in @('map_a_scene','map_a_replay','map_b_scene')) {
    $entry=$doc.inputs[$key]
    if(!$entry){throw "Missing input $key"}
    $path=if([IO.Path]::IsPathRooted($entry.path)){$entry.path}else{Join-Path (Split-Path $inputsPath -Parent) $entry.path}
    $path=(Resolve-Path -LiteralPath $path).Path
    if((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $entry.sha256){throw "Input SHA-256 mismatch: $key"}
    $files[$key]=$path
}
$adb=Join-Path $root 'tools/android-sdk/platform-tools/adb.exe'
function Adb([string[]]$Arguments) {& $adb -s $Serial @Arguments;if($LASTEXITCODE -ne 0){throw "ADB failed: $($Arguments[0])"}}
function PullAppImage([string]$Remote,[string]$Local) {
    # Export private captures as the debug application's UID.
    $start=[Diagnostics.ProcessStartInfo]::new()
    $start.FileName=$adb;$start.UseShellExecute=$false
    $start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
    foreach($argument in @('-s',$Serial,'exec-out','run-as','org.rcport.diagnostic','cat',$Remote)) {
        [void]$start.ArgumentList.Add($argument)
    }
    $process=[Diagnostics.Process]::Start($start)
    try {
        $file=[IO.File]::Create($Local)
        try {$process.StandardOutput.BaseStream.CopyTo($file)}finally{$file.Dispose()}
        $errorText=$process.StandardError.ReadToEnd();$process.WaitForExit()
        if($process.ExitCode -ne 0){throw "App capture export failed: $errorText"}
        $header=[IO.File]::ReadAllBytes($Local)
        if($header.Length -lt 8 -or [BitConverter]::ToString($header,0,8) -ne '89-50-4E-47-0D-0A-1A-0A') {
            throw "Invalid exported PNG: $Remote"
        }
    } finally {$process.Dispose()}
}
$abi=(Adb @('shell','getprop','ro.product.cpu.abi')).Trim()
$target=switch($abi){'x86_64'{'x86_64-linux-android'}'arm64-v8a'{'aarch64-linux-android'}default{throw "Unsupported probe ABI: $abi"}}
$library=Join-Path $root "android/app/src/main/jniLibs/$abi/librc_native.so"
$compiler=Join-Path $root "tools/android-sdk/ndk/27.2.12479018/toolchains/llvm/prebuilt/windows-x86_64/bin/${target}26-clang.cmd"
$saved=@{}
foreach($key in @('JAVA_HOME','ANDROID_HOME','GRADLE_USER_HOME','TEMP','TMP')){$saved[$key]=[Environment]::GetEnvironmentVariable($key,'Process')}
try {
    $env:JAVA_HOME=(Get-ChildItem (Join-Path $root 'tools/jdk21') -Directory | Select-Object -First 1).FullName
    $env:ANDROID_HOME=Join-Path $root 'tools/android-sdk'
    $env:GRADLE_USER_HOME=Join-Path $root 'tools/gradle-home'
    $env:TEMP=Join-Path $out 'compiler-temp';$env:TMP=$env:TEMP
    New-Item -ItemType Directory -Force -Path $env:TEMP | Out-Null
    & $compiler -O2 -Wall -Wextra -Werror (Join-Path $root 'scripts/android-presentation-replay.c') "-L$(Split-Path $library -Parent)" -lrc_native '-Wl,-z,max-page-size=16384' -o (Join-Path $out 'android-presentation-replay')
    if($LASTEXITCODE -ne 0){throw 'Android-native presentation probe compilation failed'}
    $probeSources=Join-Path $root 'scripts/android-presentation-probe'
    & (Join-Path $root 'tools/gradle-8.11.1/bin/gradle.bat') -p (Join-Path $root 'android') "-PrcAbis=$abi" "-PrcAcceptanceSources=$probeSources" --offline --no-daemon assembleDebug
    if($LASTEXITCODE -ne 0){throw 'Optional acceptance APK compilation failed'}
    $apk=Join-Path $out 'presentation-probe.apk'
    Copy-Item -LiteralPath (Join-Path $root 'android/app/build/outputs/apk/debug/app-debug.apk') -Destination $apk
    function Tick([string]$Label) {
        $match=@($presentation.views | Where-Object label -eq $Label)
        if($match.Count -ne 1){throw "Missing unique presentation view: $Label"}
        return [int]$match[0].tick
    }
    $barrier=Tick 'barriers-falling'
    $timings=@(0,((Tick 'effect-823')-39),((Tick 'bacta-Loading')-31),($barrier-42),$barrier,((Tick 'effect-951')-43),((Tick 'effect-1137')-43),((Tick 'door-start')-28))
    $timings -join ' ' | Set-Content -LiteralPath (Join-Path $out 'capture-ticks.txt') -Encoding ascii
    $remote='/data/local/tmp/rc-presentation-acceptance'
    Adb @('shell','mkdir','-p',$remote)
    foreach($pair in @(@((Join-Path $out 'android-presentation-replay'),'replay'),@($library,'librc_native.so'),@($files.map_a_scene,'geo_01a.rcplay'),@($files.map_a_replay,'map_a_replay.bin'),@($files.map_b_scene,'geo_01b.rcplay'),@((Join-Path $out 'capture-ticks.txt'),'capture-ticks.txt'))) {
        Adb @('push',$pair[0],"$remote/$($pair[1])")
    }
    $result=Adb @('shell',"cd $remote && chmod 700 replay && LD_LIBRARY_PATH=. ./replay geo_01a.rcplay map_a_replay.bin geo_01b.rcplay capture-ticks.txt")
    $native=($result -join "`n") | ConvertFrom-Json
    if(!$native.passed){throw 'Contiguous Android-native mission replay failed'}
    $native | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $out 'native/validation.json') -Encoding utf8
    Adb @('install','-r',$apk)
    $app='org.rcport.diagnostic';$external="/sdcard/Android/data/$app/files/acceptance"
    Adb @('shell','mkdir','-p',$external)
    Adb @('push',$files.map_a_scene,"$external/geo_01a.rcplay")
    Adb @('push',$files.map_a_replay,"$external/map_a_replay.bin")
    foreach($name in @('intro','injury','bacta','barriers','falling','bunker','demolition','door')) {
        $local=Join-Path $out "native/$name.save"
        Adb @('pull',"$remote/$name.save",$local)
        Adb @('push',$local,"$external/$name.save")
    }
    $runs=[Collections.Generic.List[object]]::new()
    foreach($probe in @(@('intro',300),@('injury',150),@('bacta',360),@('barriers',240),@('falling',12),@('bunker',180),@('demolition',180),@('door',110))) {
        $name=[string]$probe[0]
        Adb @('shell','am','force-stop',$app);Adb @('logcat','-c')
        Adb @('shell','rm','-f',"$external/$name-result.json")
        Adb @('shell',"run-as $app sh -c 'rm -f files/$name-*.png'")
        Adb @('shell','am','start','-W','-n',"$app/.PresentationProbeActivity",'--es','probe',$name,'--ei','ticks',[string]$probe[1])
        $deadline=[DateTime]::UtcNow.AddMinutes(4)
        do {
            Start-Sleep -Milliseconds 500
            $ready=Adb @('shell',"if [ -f $external/$name-result.json ]; then echo ready; fi")
            if([DateTime]::UtcNow -gt $deadline){throw "Android GLES/audio probe timed out: $name"}
        } while($ready -notcontains 'ready')
        Adb @('pull',"$external/$name-result.json",(Join-Path $out "$name-result.json"))
        $run=Get-Content -LiteralPath (Join-Path $out "$name-result.json") -Raw | ConvertFrom-Json
        $log=Adb @('logcat','-d','-s','RCProbe:I','RCGpu:I','RCAudio:I','AndroidRuntime:E','*:S')
        $log | Set-Content -LiteralPath (Join-Path $out "$name.log") -Encoding utf8
        if(!$run.passed){throw "Android GLES/audio probe failed: $name $($run.error)"}
        if(($log -join "`n") -match 'FATAL EXCEPTION|Graphics failure|Probe failure|play failed|sample load failed'){throw "Android log failure: $name"}
        if(($log -join "`n") -notmatch '"type":"draw"' -or ($log -join "`n") -notmatch 'bank sounds='){throw "Missing Android driver/audio evidence: $name"}
        $events=@($log | Where-Object {$_ -match 'event sequence=.*played_ms='}).Count
        $loops=@($log | Where-Object {$_ -match 'ambient owner=.*stream='}).Count
        if($name -in @('intro','injury','bacta','bunker','demolition','door') -and $events -eq 0){throw "No SoundPool event playback: $name"}
        if($name -eq 'bacta' -and $loops -eq 0){throw 'No actual Bacta ambient stream started'}
        $run | Add-Member -NotePropertyName soundpool_events -NotePropertyValue $events
        $run | Add-Member -NotePropertyName ambient_starts -NotePropertyValue $loops
        $runs.Add($run)
        $captures=Join-Path $out 'apk-captures'
        New-Item -ItemType Directory -Force -Path $captures | Out-Null
        $images=@(Adb @('shell','run-as',$app,'ls','files') | Where-Object {$_ -match "^$name-\d+\.png$"})
        if(@($images).Count -ne 2){throw "Expected two actual GLES captures: $name"}
        foreach($image in $images){PullAppImage "files/$($image.Trim())" (Join-Path $captures $image.Trim())}
    }
    @{schema=1;passed=$true;serial=$Serial;abi=$abi;native=$native;apk_sha256=(Get-FileHash -LiteralPath $apk).Hash.ToLowerInvariant();native_sha256=(Get-FileHash -LiteralPath $library).Hash.ToLowerInvariant();inputs=$doc.inputs;runs=$runs;limits=@('Optional local acceptance APK; ordinary delivery APK has no probe activity','Emulator GLES/SoundPool execution does not certify subjective hearing or touch comfort on a physical phone')} | ConvertTo-Json -Depth 14 | Set-Content -LiteralPath (Join-Path $out 'validation.json') -Encoding utf8
} finally {foreach($key in $saved.Keys){[Environment]::SetEnvironmentVariable($key,$saved[$key],'Process')}}
