#Requires -Version 7.0
param(
    [Parameter(Mandatory)][string]$Inputs,
    [string]$Stage='mission',
    [string]$GameData,
    [string]$OutputDirectory=(Join-Path (Split-Path $PSScriptRoot -Parent) 'build/scenes'),
    [switch]$Online
)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$manifestPath=(Resolve-Path -LiteralPath $Inputs).Path
$manifest=Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json -AsHashtable
$recipes=Get-Content -LiteralPath (Join-Path $root 'config/scene-builds.json') -Raw | ConvertFrom-Json -AsHashtable
if($manifest.schema -ne 1){throw 'Unsupported scene manifest schema'}
$stages=if($Stage -eq 'mission'){@($recipes.mission)}else{@($Stage)}
$out=[IO.Path]::GetFullPath($OutputDirectory)
$inputRoot=Split-Path $manifestPath -Parent
$resolved=@{};$available=@{};$required=[Collections.Generic.HashSet[string]]::new()
foreach($name in $stages){
    if(!$recipes.stages.ContainsKey($name)){throw "Unknown scene stage: $name"}
    foreach($key in $recipes.stages[$name].inputs){if(!$available.ContainsKey($key)){[void]$required.Add($key)}}
    $available[$recipes.stages[$name].output_key]=$true
}
if($Stage -eq 'mission'){foreach($key in @('map_a_replay','map_b_replay','map_c_replay')){[void]$required.Add($key)}}
# Preflight every external input before compiling or writing any scene outputs.
foreach($key in $required){
    if(!$manifest.inputs.ContainsKey($key)){throw "Missing scene input '$key' in $manifestPath"}
    $entry=$manifest.inputs[$key]
    if(!$entry.path -or $entry.sha256 -notmatch '^[a-fA-F0-9]{64}$'){throw "Unpinned scene input '$key'"}
    $path=if([IO.Path]::IsPathRooted($entry.path)){[IO.Path]::GetFullPath($entry.path)}else{[IO.Path]::GetFullPath((Join-Path $inputRoot $entry.path))}
    if(!(Test-Path -LiteralPath $path -PathType Leaf)){throw "Missing scene input '$key': $path"}
    if((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $entry.sha256){throw "SHA-256 mismatch for scene input '$key'"}
    $resolved[$key]=@{path=$path;sha256=$entry.sha256.ToLowerInvariant()}
}
if(!$GameData -or !(Test-Path -LiteralPath (Join-Path $GameData 'Maps') -PathType Container)){throw 'Provide -GameData with an installed Republic Commando GameData directory'}
New-Item -ItemType Directory -Force -Path $out | Out-Null
$active=Join-Path $out 'resolved-inputs.json'
$saved=@{}
foreach($name in @('RC_BUILD_INPUTS','RC_BUILD_OUTPUT','RC_GAME_DATA')){$saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
$results=[Collections.Generic.List[object]]::new()
$options=@('--release','--locked');if(!$Online){$options+='--offline'}
$targetRoot=if($env:CARGO_TARGET_DIR){[IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)}else{Join-Path $root 'target'}
Push-Location $root
try {
    foreach($name in $stages){
        $recipe=$recipes.stages[$name]
        $stageOut=Join-Path $out $name
        New-Item -ItemType Directory -Force -Path $stageOut | Out-Null
        @{schema=1;inputs=$resolved} | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $active -Encoding utf8NoBOM
        $env:RC_BUILD_INPUTS=$active;$env:RC_BUILD_OUTPUT=$stageOut;$env:RC_GAME_DATA=[IO.Path]::GetFullPath($GameData)
        $arguments=@(foreach($argument in $recipe.args){
            if($argument -eq '{game_data}'){$env:RC_GAME_DATA}
            elseif($argument -eq '{out}'){$stageOut}
            elseif($argument -match '^\{([a-z0-9_]+)\}$'){$resolved[$Matches[1]].path}
            else {throw "Invalid recipe argument $argument"}
        })
        Write-Host "Building scene stage $name"
        & cargo build -p rc-inspect --bin $recipe.binary @options
        if($LASTEXITCODE -ne 0){throw "Builder compilation failed: $name"}
        & (Join-Path $targetRoot "release/$($recipe.binary).exe") @arguments
        if($LASTEXITCODE -ne 0){throw "Scene stage failed: $name"}
        $product=Join-Path $stageOut $recipe.product
        if(!(Test-Path -LiteralPath $product -PathType Leaf)){throw "Scene stage produced no scene: $name"}
        $hash=(Get-FileHash -LiteralPath $product -Algorithm SHA256).Hash.ToLowerInvariant()
        $resolved[$recipe.output_key]=@{path=$product;sha256=$hash}
        $results.Add(@{stage=$name;path=$product;sha256=$hash;bytes=(Get-Item -LiteralPath $product).Length})
    }
    # Standalone stages must publish their produced identities too, so their
    # resolved manifest can be consumed by a subsequent stage or package build.
    @{schema=1;inputs=$resolved} | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $active -Encoding utf8NoBOM
    if($Stage -eq 'mission'){
        $env:RC_BUILD_OUTPUT=Join-Path $out 'runtime-validation'
        & cargo build -p rc-inspect --bin rc-checkpoint-check @options
        if($LASTEXITCODE -ne 0){throw 'Mission checkpoint validator compilation failed'}
        & (Join-Path $targetRoot 'release/rc-checkpoint-check.exe')
        if($LASTEXITCODE -ne 0){throw 'Rebuilt mission checkpoint validation failed'}
    }
    @{schema=1;passed=$true;source_manifest=$manifestPath;mission_runtime_checked=($Stage -eq 'mission');inputs=$resolved;results=$results} | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $out 'validation.json') -Encoding utf8
} finally {
    foreach($name in $saved.Keys){[Environment]::SetEnvironmentVariable($name,$saved[$name],'Process')}
    Pop-Location
}
