#Requires -Version 7.0
param([Parameter(Mandatory)][string]$Inputs,[string]$OutputDirectory=(Join-Path (Split-Path $PSScriptRoot -Parent) 'build/artifacts'))
$ErrorActionPreference='Stop'
$manifest=(Resolve-Path -LiteralPath $Inputs).Path
$doc=Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json -AsHashtable
if($doc.schema -ne 1){throw 'Unsupported scene input manifest'}
$maps=[ordered]@{GEO_01A='map_a_scene';GEO_01B='map_b_scene';GEO_01C='map_c_scene';GEO_03A='factory_scene'}
$files=@{};$text="format=RCCAMPAIGN1`n"
foreach($map in $maps.Keys){
    $key=$maps[$map];if(!$doc.inputs.ContainsKey($key)){throw "Missing package input: $key"}
    $entry=$doc.inputs[$key]
    $file=if([IO.Path]::IsPathRooted($entry.path)){$entry.path}else{Join-Path (Split-Path $manifest -Parent) $entry.path}
    $file=(Resolve-Path -LiteralPath $file).Path;$size=(Get-Item -LiteralPath $file).Length
    if($size -lt 12 -or $size -gt 112MB -or (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -ne $entry.sha256){throw "Invalid size/SHA-256: $map"}
    $filename=$map.ToLowerInvariant()+'.rcplay';$files[$filename]=$file
    $text+="$map.file=$filename`n$map.bytes=$size`n$map.sha256=$($entry.sha256.ToLowerInvariant())`n"
}
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$output=Join-Path ([IO.Path]::GetFullPath($OutputDirectory)) 'mission1-scenes.rcpack.zip';$temporary=$output+'.tmp'
try {
    $stream=[IO.File]::Open($temporary,[IO.FileMode]::Create)
    try {
        $zip=[IO.Compression.ZipArchive]::new($stream,[IO.Compression.ZipArchiveMode]::Create,$true)
        try {
            $entry=$zip.CreateEntry('campaign.properties');$writer=[IO.StreamWriter]::new($entry.Open(),[Text.Encoding]::ASCII)
            try {$writer.Write($text)}finally{$writer.Dispose()}
            foreach($name in $files.Keys){
                $entry=$zip.CreateEntry($name,[IO.Compression.CompressionLevel]::Optimal);$target=$entry.Open();$source=[IO.File]::OpenRead($files[$name])
                try {$source.CopyTo($target)}finally{$source.Dispose();$target.Dispose()}
            }
        }finally{$zip.Dispose()}
        $stream.Flush($true)
    }finally{$stream.Dispose()}
    [IO.File]::Move($temporary,$output,$true)
}finally{if(Test-Path -LiteralPath $temporary){Remove-Item -LiteralPath $temporary}}
@{schema=1;passed=$true;format='RCCAMPAIGN1';maps=@($maps.Keys);path=$output;sha256=(Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash.ToLowerInvariant();bytes=(Get-Item -LiteralPath $output).Length} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputDirectory 'campaign-package.json') -Encoding utf8
Write-Output $output
