[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$minimumBuild = 26100
$currentBuild = [Environment]::OSVersion.Version.Build

if (-not $IsWindows -and $PSVersionTable.PSEdition -eq "Core") {
    throw "Dieses Projekt kann nur unter Windows gebaut und installiert werden."
}

if ($currentBuild -lt $minimumBuild) {
    throw "Windows Build $minimumBuild oder neuer ist erforderlich. Gefunden: $currentBuild"
}

$dotnetVersion = & dotnet --version
if ($LASTEXITCODE -ne 0 -or -not $dotnetVersion.StartsWith("10.")) {
    throw ".NET SDK 10 ist erforderlich. Gefunden: $dotnetVersion"
}

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path $vswhere)) {
    throw "Visual Studio Installer bzw. vswhere.exe wurde nicht gefunden. Installiere Visual Studio 2026 mit WinUI/MSIX-Tools."
}

$msbuild = & $vswhere -latest -products * -requires Microsoft.Component.MSBuild -find "MSBuild\**\Bin\MSBuild.exe" |
    Select-Object -First 1
if (-not $msbuild) {
    throw "MSBuild wurde nicht gefunden."
}

Write-Host "Windows build: $currentBuild"
Write-Host ".NET SDK:      $dotnetVersion"
Write-Host "MSBuild:      $msbuild"
