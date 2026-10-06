param(
    [string]$GameData = 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData',
    [string]$Binary = 'System\ctgame.dll'
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$ghidra = Join-Path $projectRoot 'tools\ghidra_12.1.4_PUBLIC\support\analyzeHeadless.bat'
$portableJava = Get-ChildItem (Join-Path $projectRoot 'tools\jdk21') -Directory -ErrorAction SilentlyContinue | Select-Object -First 1
if ($portableJava) {
    $env:JAVA_HOME = $portableJava.FullName
    $env:PATH = (Join-Path $env:JAVA_HOME 'bin') + ';' + $env:PATH
    $properties = Join-Path $projectRoot 'tools\ghidra_12.1.4_PUBLIC\support\launch.properties'
    $content = Get-Content -LiteralPath $properties -Raw
    $content = $content -replace '(?m)^JAVA_HOME_OVERRIDE=.*$', ('JAVA_HOME_OVERRIDE=' + $portableJava.FullName.Replace('\','/'))
    Set-Content -LiteralPath $properties -Value $content -Encoding utf8
}
if (!(Test-Path -LiteralPath $ghidra)) { throw 'Ghidra 12.1.4 is missing from tools.' }
$inputFile = Join-Path $GameData $Binary
if (!(Test-Path -LiteralPath $inputFile)) { throw "Missing binary: $inputFile" }
$db = Join-Path $projectRoot 'analysis\ghidra-projects'
$output = Join-Path $projectRoot 'analysis\decompiled'
New-Item -ItemType Directory -Force -Path $db,$output | Out-Null
& $ghidra $db 'RepublicCommando' -import $inputFile -overwrite -analysisTimeoutPerFile 600 -max-cpu 4 -scriptPath (Join-Path $PSScriptRoot 'ghidra') -postScript ExportPortingFunctions.java $output
if ($LASTEXITCODE -ne 0) { throw "Ghidra failed: $LASTEXITCODE" }
