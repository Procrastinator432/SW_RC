param(
    [Parameter(Mandatory)][string]$PackageFile,
    [Parameter(Mandatory)][string]$ReportFile,
    [Parameter(Mandatory)][string]$ObjectPath
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$report = Get-Content -LiteralPath $ReportFile -Raw | ConvertFrom-Json
$matches = @($report.exports | Where-Object object_path -EQ $ObjectPath)
if ($matches.Count -ne 1) { throw 'Object path must identify exactly one export' }
$object = $matches[0]
$bytes = [IO.File]::ReadAllBytes((Resolve-Path -LiteralPath $PackageFile))
if ($bytes.Length -lt ([long]$object.serial_offset + [long]$object.serial_size)) { throw 'Object payload exceeds file' }
$out = Join-Path $projectRoot 'analysis\objects'
New-Item -ItemType Directory -Force -Path $out | Out-Null
$stem = [IO.Path]::GetFileNameWithoutExtension($PackageFile)
$safePath = $ObjectPath -replace '[^a-zA-Z0-9_.-]', '_'
$destination = Join-Path $out ($stem + '__' + $safePath + '.bin')
$payload = New-Object byte[] $object.serial_size
[Array]::Copy($bytes, [long]$object.serial_offset, $payload, 0, [long]$object.serial_size)
[IO.File]::WriteAllBytes($destination, $payload)
[ordered]@{
    source = (Resolve-Path -LiteralPath $PackageFile).Path
    source_sha256 = (Get-FileHash -LiteralPath $PackageFile -Algorithm SHA256).Hash
    object_path = $ObjectPath
    class_path = $object.class_path
    serial_offset = $object.serial_offset
    serial_size = $object.serial_size
    payload_sha256 = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
    status = 'Raw serialized object; properties, function metadata and bytecode are not yet separated.'
} | ConvertTo-Json | Set-Content -LiteralPath ($destination + '.json') -Encoding utf8
Write-Output $destination
