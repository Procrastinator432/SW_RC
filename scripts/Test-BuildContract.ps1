param([string]$ProjectRoot=(Split-Path $PSScriptRoot -Parent))
$ErrorActionPreference='Stop'
$recipes=Get-Content -LiteralPath (Join-Path $ProjectRoot 'config/scene-builds.json') -Raw | ConvertFrom-Json -AsHashtable
foreach($stage in $recipes.stages.Values){
    foreach($argument in $stage.args) {
        if($argument -in @('{game_data}','{out}') -or $argument -match '^geo_(01[abc]|03a)$'){continue}
        if($argument -match '^\{([a-z0-9_]+)\}$' -and $Matches[1] -in $stage.inputs){continue}
        throw "Invalid or undeclared recipe argument '$argument' in $($stage.binary)"
    }
    $source=Get-Content -LiteralPath (Join-Path $ProjectRoot "crates/rc-inspect/src/bin/$($stage.binary).rs") -Raw
    foreach($match in [regex]::Matches($source,'build_inputs::input\(\s*"([a-z0-9_]+)"')){
        if($match.Groups[1].Value -notin $stage.inputs){throw "Undeclared input $($match.Groups[1].Value) in $($stage.binary)"}
    }
}
# Prevent silently reintroducing implicit reads/writes to ignored deliveries.
foreach($file in Get-ChildItem (Join-Path $ProjectRoot 'crates/rc-inspect/src/bin') -Filter '*build.rs') {
    $source=Get-Content -LiteralPath $file.FullName -Raw
    if($source -match 'analysis[/\\]mission' -or $source -match 'D:/SteamLibrary') {throw "Implicit historical/machine path in builder $($file.Name)"}
}
foreach($file in @('build.ps1','scripts/Host-Tools.ps1','scripts/Build-Android.ps1','scripts/Build-Scenes.ps1','scripts/Build-CampaignPackage.ps1','scripts/Build-RevivalCheck.ps1','scripts/Test-PresentationAndroid.ps1','scripts/Test-GpuFrame.ps1','scripts/Analyze-Binary.ps1','scripts/Analyze-Addresses.ps1')) {
    $source=Get-Content -LiteralPath (Join-Path $ProjectRoot $file) -Raw
    if($source -match 'analysis[/\\]mission') {throw "Historical delivery dependency in $file"}
    $tokens=$null;$parseErrors=$null
    [void][System.Management.Automation.Language.Parser]::ParseFile((Join-Path $ProjectRoot $file),[ref]$tokens,[ref]$parseErrors)
    if($parseErrors.Count){throw "Invalid PowerShell syntax in ${file}: $($parseErrors[0].Message)"}
}
Write-Output 'Build contract passed: no implicit mission archive or machine-specific GameData paths in scene builders/central build.'
