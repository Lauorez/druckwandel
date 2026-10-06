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

& (Join-Path $PSScriptRoot "check-version.ps1")
& (Join-Path $PSScriptRoot "check-environment.ps1")
& (Join-Path $PSScriptRoot "create-dev-cert.ps1") -Password $CertificatePassword
& (Join-Path $PSScriptRoot "build-native-task.ps1") -Platform $Platform -Configuration $Configuration
# Der Test muss in Windows PowerShell (.NET Framework) laufen: Er belegt, dass der
# native Task kein CoreCLR lädt, und pwsh hat CoreCLR immer schon geladen.
$windowsPowerShell = Join-Path $env:SystemRoot "System32\WindowsPowerShell\v1.0\powershell.exe"
& $windowsPowerShell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "test-native-task.ps1")
if ($LASTEXITCODE -ne 0) {
    throw "Der Test des nativen Background-Tasks ist fehlgeschlagen."
}

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
    Where-Object { $_.FullName -notmatch "[\\/]Dependencies[\\/]" } |
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
    $packageEntries = $archive.Entries | ForEach-Object { $_.FullName.Replace("\", "/") }
    $requiredEntries = @(
        "AppxManifest.xml",
        "Config/PrinterPdc.xml",
        "CompanionApp.exe",
        "CompanionApp.runtimeconfig.json",
        "coreclr.dll",
        "hostfxr.dll",
        "System.Private.CoreLib.dll",
        "ERechnung.VirtualPrinter.Native.dll",
        "ERechnung.VirtualPrinter.Tasks.winmd",
        "resources.pri",
        "AppxSignature.p7x"
    )
    $forbiddenEntries = @(
        "WinRT.Host.dll",
        "WinRT.Host.runtimeconfig.json",
        "ERechnung.VirtualPrinter.Tasks.dll"
    )

    $missingEntries = $requiredEntries | Where-Object { $_ -notin $packageEntries }
    if ($missingEntries) {
        throw "Dem MSIX-Paket fehlen erforderliche Dateien: $($missingEntries -join ', ')"
    }

    $unexpectedEntries = $forbiddenEntries | Where-Object { $_ -in $packageEntries }
    if ($unexpectedEntries) {
        throw "Das MSIX-Paket enthält einen nicht eigenständigen Task-Host: $($unexpectedEntries -join ', ')"
    }

    $runtimeEntry = $archive.GetEntry("CompanionApp.runtimeconfig.json")
    $runtimeReader = [System.IO.StreamReader]::new($runtimeEntry.Open())
    try {
        $runtime = $runtimeReader.ReadToEnd() | ConvertFrom-Json
    } finally {
        $runtimeReader.Dispose()
    }
    if ($runtime.runtimeOptions.framework -or $runtime.runtimeOptions.frameworks) {
        throw "Die Druckoberfläche benötigt eine externe .NET-Installation und ist nicht vollständig."
    }

    $manifestEntry = $archive.GetEntry("AppxManifest.xml")
    $reader = [System.IO.StreamReader]::new($manifestEntry.Open())
    try {
        $manifestContent = $reader.ReadToEnd()
    } finally {
        $reader.Dispose()
    }

    if ($manifestContent -notmatch "windows\.printSupportVirtualPrinterWorkflow") {
        throw "Das fertige MSIX-Manifest registriert keinen Print Support Virtual Printer."
    }

    if ($manifestContent -notmatch "<Path>ERechnung\.VirtualPrinter\.Native\.dll</Path>") {
        throw "Das fertige MSIX-Manifest aktiviert den Drucker-Task nicht über ERechnung.VirtualPrinter.Native.dll."
    }

    if ($manifestContent -match "OutputFileTypes") {
        throw "Das fertige MSIX-Manifest enthält OutputFileTypes und würde einen Speichern-unter-Dialog aktivieren."
    }
} finally {
    $archive.Dispose()
}

Write-Host "Signed package: $($package.FullName)"
Write-Host "Package output: $packageDirectory"
