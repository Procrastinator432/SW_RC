param(
    [string]$Binary = 'engine.dll',
    [string]$Output = 'serializers.c',
    [string[]]$Addresses = @('10461c90','10411560','10527330')
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
. (Join-Path $PSScriptRoot 'Host-Tools.ps1')
$headless=if($IsWindows){'analyzeHeadless.bat'}else{'analyzeHeadless'}
$ghidra = Join-Path $projectRoot "tools/ghidra_12.1.4_PUBLIC/support/$headless"
$env:JAVA_HOME=Get-RcJdk (Join-Path $projectRoot 'tools')
$env:PATH=(Join-Path $env:JAVA_HOME 'bin')+[IO.Path]::PathSeparator+$env:PATH
$db = Join-Path $projectRoot 'analysis\ghidra-projects'
$out = Join-Path $projectRoot ('analysis\decompiled\' + [IO.Path]::GetFileName($Output))
& $ghidra $db 'RepublicCommando' -process $Binary -noanalysis -scriptPath (Join-Path $PSScriptRoot 'ghidra') -postScript ExportAddresses.java $out @Addresses
if ($LASTEXITCODE -ne 0) { throw 'Ghidra address export failed' }
