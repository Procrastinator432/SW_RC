param(
    [string]$Binary = 'engine.dll',
    [string]$Output = 'serializers.c',
    [string[]]$Addresses = @('10461c90','10411560','10527330')
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$ghidra = Join-Path $projectRoot 'tools\ghidra_12.1.4_PUBLIC\support\analyzeHeadless.bat'
$db = Join-Path $projectRoot 'analysis\ghidra-projects'
$out = Join-Path $projectRoot ('analysis\decompiled\' + [IO.Path]::GetFileName($Output))
& $ghidra $db 'RepublicCommando' -process $Binary -noanalysis -scriptPath (Join-Path $PSScriptRoot 'ghidra') -postScript ExportAddresses.java $out @Addresses
if ($LASTEXITCODE -ne 0) { throw 'Ghidra address export failed' }
