# Shared host paths for the existing PowerShell build and analysis entry points.
function Get-RcExecutable([string]$Name) {
    if($IsWindows){return "$Name.exe"}
    return $Name
}
function Get-RcJdk([string]$ToolsDirectory) {
    $javac=Get-RcExecutable 'javac'
    if($env:JAVA_HOME -and (Test-Path (Join-Path $env:JAVA_HOME "bin/$javac"))){return $env:JAVA_HOME}
    $portable=Get-ChildItem (Join-Path $ToolsDirectory 'jdk21') -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name | Where-Object {Test-Path (Join-Path $_.FullName "bin/$javac")} | Select-Object -First 1
    if($portable){return $portable.FullName}
    throw 'JDK missing: set JAVA_HOME or install tools/jdk21 (see docs/BUILD.md)'
}
function Get-RcNdkHost {
    if($IsWindows){return 'windows-x86_64'}
    if($IsLinux){return 'linux-x86_64'}
    throw 'The Android builder currently supports Windows and Linux x86_64 hosts'
}
