[CmdletBinding()]
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",

    [ValidateSet("x64", "ARM64")]
    [string]$Platform = "x64",

    [string]$CertificatePassword = "ERechnung-Dev-Only"
)

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path $PSScriptRoot -Parent

& (Join-Path $PSScriptRoot "check-environment.ps1")
& (Join-Path $PSScriptRoot "create-dev-cert.ps1") -Password $CertificatePassword

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
$msbuild = & $vswhere -latest -products * -requires Microsoft.Component.MSBuild -find "MSBuild\**\Bin\MSBuild.exe" |
    Select-Object -First 1

$project = Join-Path $repositoryRoot "src\CompanionApp\CompanionApp.csproj"
$certificate = Join-Path $repositoryRoot ".cert\ERechnung.Dev.pfx"
$packageDirectory = Join-Path $repositoryRoot "artifacts\packages\"
$runtimeIdentifier = if ($Platform -eq "ARM64") { "win-arm64" } else { "win-x64" }

New-Item -ItemType Directory -Force -Path $packageDirectory | Out-Null

& $msbuild $project `
    /restore `
    /m `
    /p:Configuration=$Configuration `
    /p:Platform=$Platform `
    /p:RuntimeIdentifier=$runtimeIdentifier `
    /p:GenerateAppxPackageOnBuild=true `
    /p:AppxPackageSigningEnabled=false `
    "/p:AppxPackageDir=$packageDirectory"

if ($LASTEXITCODE -ne 0) {
    throw "MSBuild failed with exit code $LASTEXITCODE."
}

$package = Get-ChildItem -Path $packageDirectory -Recurse -File -Filter "*.msix" |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1
if (-not $package) {
    throw "MSBuild meldete Erfolg, hat aber kein MSIX-Paket erzeugt."
}

$nugetPackages = if ($env:NUGET_PACKAGES) {
    $env:NUGET_PACKAGES
} else {
    Join-Path $env:USERPROFILE ".nuget\packages"
}

$signTool = Get-ChildItem `
    -Path (Join-Path $nugetPackages "microsoft.windows.sdk.buildtools") `
    -Recurse `
    -File `
    -Filter "signtool.exe" |
    Where-Object { $_.DirectoryName -match "[\\/]x64$" } |
    Sort-Object FullName -Descending |
    Select-Object -First 1
if (-not $signTool) {
    throw "SignTool wurde in den restaurierten Windows SDK BuildTools nicht gefunden."
}

& $signTool.FullName sign /fd SHA256 /f $certificate /p $CertificatePassword $package.FullName
if ($LASTEXITCODE -ne 0) {
    throw "SignTool failed with exit code $LASTEXITCODE."
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::OpenRead($package.FullName)
try {
    $signature = $archive.Entries | Where-Object { $_.FullName -eq "AppxSignature.p7x" }
    if (-not $signature) {
        throw "Das MSIX-Paket wurde erzeugt, enthält aber keine Signatur."
    }
} finally {
    $archive.Dispose()
}

Write-Host "Signed package: $($package.FullName)"
Write-Host "Package output: $packageDirectory"
