param(
    [string]$GameData = $env:RC_GAME_DATA,
    [string]$Binary = 'System/ctgame.dll'
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Host-Tools.ps1')
$projectRoot = Split-Path $PSScriptRoot -Parent
$headless=if($IsWindows){'analyzeHeadless.bat'}else{'analyzeHeadless'}
$ghidra = Join-Path $projectRoot "tools/ghidra_12.1.4_PUBLIC/support/$headless"
$env:JAVA_HOME = Get-RcJdk (Join-Path $projectRoot 'tools')
$env:PATH = (Join-Path $env:JAVA_HOME 'bin') + [IO.Path]::PathSeparator + $env:PATH
if (!(Test-Path -LiteralPath $ghidra)) { throw 'Ghidra 12.1.4 is missing from tools.' }
if(!$GameData){throw 'Provide -GameData or set RC_GAME_DATA to the installed GameData directory'}
$inputFile = Join-Path $GameData $Binary
if (!(Test-Path -LiteralPath $inputFile)) { throw "Missing binary: $inputFile" }
$db = Join-Path $projectRoot 'analysis\ghidra-projects'
$output = Join-Path $projectRoot 'analysis\decompiled'
New-Item -ItemType Directory -Force -Path $db,$output | Out-Null
& $ghidra $db 'RepublicCommando' -import $inputFile -overwrite -analysisTimeoutPerFile 600 -max-cpu 4 -scriptPath (Join-Path $PSScriptRoot 'ghidra') -postScript ExportPortingFunctions.java $output
if ($LASTEXITCODE -ne 0) { throw "Ghidra failed: $LASTEXITCODE" }
