param(
    [string]$ToolsDirectory=(Join-Path (Split-Path $PSScriptRoot -Parent) 'tools'),
    [string]$FramesDirectory,
    [string]$WorldPacket,
    [string]$OutputDirectory=(Join-Path (Split-Path $PSScriptRoot -Parent) 'build/gpu-frame-java')
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'Host-Tools.ps1')
$root=Split-Path $PSScriptRoot -Parent
$tools=[IO.Path]::GetFullPath($ToolsDirectory)
$classes=[IO.Path]::GetFullPath($OutputDirectory)
$jdk=Get-RcJdk $tools
$androidJar=Join-Path $tools 'android-sdk/platforms/android-35/android.jar'
New-Item -ItemType Directory -Force -Path $classes | Out-Null
$sources=@('android/app/src/main/java/org/rcport/diagnostic/GpuSceneView.java','android/app/src/main/java/org/rcport/diagnostic/FramePass.java','scripts/java/GpuFrameCheck.java') | ForEach-Object {Join-Path $root $_}
& (Join-Path $jdk "bin/$(Get-RcExecutable 'javac')") -cp $androidJar -d $classes @sources
if($LASTEXITCODE -ne 0){throw 'GPU frame check compilation failed'}
$frames=if($FramesDirectory){@(Get-ChildItem -LiteralPath $FramesDirectory -Filter '*.frame' -File | Sort-Object Name | ForEach-Object FullName)}else{@()}
if($FramesDirectory -and !$frames.Count){throw 'No native player frames found'}
if($WorldPacket){$frames=@((Resolve-Path -LiteralPath $WorldPacket).Path)+$frames}
# Fix the collector for this constrained desktop heap. Android ART is covered
# independently by the GLES acceptance run.
& (Join-Path $jdk "bin/$(Get-RcExecutable 'java')") -Xmx96m -XX:+UseSerialGC -cp "$classes$([IO.Path]::PathSeparator)$androidJar" org.rcport.diagnostic.GpuFrameCheck @frames
if($LASTEXITCODE -ne 0){throw 'GPU frame reader check failed'}
