[CmdletBinding()]
param(
    [switch]$SkipChecks,
    [switch]$SkipPrinterBuild,

    [ValidateSet("Development", "Production")]
    [string]$SigningMode = "Development",

    [string]$PrinterPackagePath = "",
    [string]$PrinterDependencyPath = "",
    [string]$PrinterCertificatePath = ""
)

$ErrorActionPreference = "Stop"
$workspace = Split-Path -Parent $PSScriptRoot
$tauriRoot = Join-Path $workspace "apps\desktop\src-tauri"
$payloadRoot = Join-Path $tauriRoot "installer\windows\payload"
$printerPackagesRoot = Join-Path $workspace "drucker\src\CompanionApp\AppPackages"
$defaultCertificate = Join-Path $workspace "drucker\.cert\ERechnung.Dev.cer"

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "'$Command $($Arguments -join ' ')' ist fehlgeschlagen (Fehlercode $LASTEXITCODE)."
    }
}

if (-not $SkipChecks) {
    Invoke-Checked -Command "npm.cmd" -Arguments @("run", "check")
}

if (-not $PrinterPackagePath) {
    if (-not $SkipPrinterBuild) {
        Invoke-Checked -Command "powershell.exe" -Arguments @(
            "-NoProfile",
            "-ExecutionPolicy", "Bypass",
            "-File", (Join-Path $workspace "scripts\wp5-build-reference.ps1")
        )
    }

    $printerPackage = Get-ChildItem -Path $printerPackagesRoot -Recurse -File -Filter "CompanionApp_*.msix" -ErrorAction SilentlyContinue |
        Where-Object FullName -NotMatch "[\\/]Dependencies[\\/]" |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1
    if (-not $printerPackage) {
        throw "Kein Druckerpaket gefunden. Führe den Build ohne -SkipPrinterBuild aus."
    }
    $PrinterPackagePath = $printerPackage.FullName
}

$PrinterPackagePath = (Resolve-Path -LiteralPath $PrinterPackagePath).Path
$packageDirectory = Split-Path -Parent $PrinterPackagePath

if (-not $PrinterDependencyPath) {
    $dependencyDirectory = Join-Path $packageDirectory "Dependencies\x64"
    $dependency = Get-ChildItem -LiteralPath $dependencyDirectory -File -Filter "Microsoft.WindowsAppRuntime*.msix" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1
    if (-not $dependency) {
        throw "Die x64-Windows-App-Runtime des Druckerpakets wurde nicht gefunden."
    }
    $PrinterDependencyPath = $dependency.FullName
}
$PrinterDependencyPath = (Resolve-Path -LiteralPath $PrinterDependencyPath).Path

if ($SigningMode -eq "Development") {
    if (-not $PrinterCertificatePath) {
        $PrinterCertificatePath = $defaultCertificate
    }
    $PrinterCertificatePath = (Resolve-Path -LiteralPath $PrinterCertificatePath).Path
} else {
    $signature = Get-AuthenticodeSignature -LiteralPath $PrinterPackagePath
    if ($signature.Status -ne "Valid") {
        throw "Das Produktionspaket besitzt keine unter Windows gültige Signatur."
    }
    if ($signature.SignerCertificate.Subject -eq $signature.SignerCertificate.Issuer) {
        throw "Ein selbstsigniertes Zertifikat darf nicht für den Produktionsinstaller verwendet werden."
    }
}

New-Item -ItemType Directory -Force -Path $payloadRoot | Out-Null
$stagedPackage = Join-Path $payloadRoot "Printer.msix"
$stagedDependency = Join-Path $payloadRoot "WindowsAppRuntime.msix"
$stagedCertificate = Join-Path $payloadRoot "PrinterCertificate.cer"
Copy-Item -LiteralPath $PrinterPackagePath -Destination $stagedPackage -Force
Copy-Item -LiteralPath $PrinterDependencyPath -Destination $stagedDependency -Force
if ($SigningMode -eq "Development") {
    Copy-Item -LiteralPath $PrinterCertificatePath -Destination $stagedCertificate -Force
} elseif (Test-Path -LiteralPath $stagedCertificate) {
    Remove-Item -LiteralPath $stagedCertificate -Force
}
$utf8WithBom = [System.Text.UTF8Encoding]::new($true)
$utf8WithoutBom = [System.Text.UTF8Encoding]::new($false)
$installScriptContent = [System.IO.File]::ReadAllText((Join-Path $tauriRoot "installer\windows\InstallPrinter.ps1"), $utf8WithoutBom)
$removeScriptContent = [System.IO.File]::ReadAllText((Join-Path $tauriRoot "installer\windows\RemovePrinter.ps1"), $utf8WithoutBom)
[System.IO.File]::WriteAllText((Join-Path $payloadRoot "InstallPrinter.ps1"), $installScriptContent, $utf8WithBom)
[System.IO.File]::WriteAllText((Join-Path $payloadRoot "RemovePrinter.ps1"), $removeScriptContent, $utf8WithBom)

$validationArguments = @(
    "-NoProfile",
    "-ExecutionPolicy", "Bypass",
    "-File", (Join-Path $payloadRoot "InstallPrinter.ps1"),
    "-PackagePath", $stagedPackage,
    "-DependencyPath", $stagedDependency,
    "-ValidateOnly"
)
if ($SigningMode -eq "Development") {
    $validationArguments += @("-CertificatePath", $stagedCertificate)
}
Invoke-Checked -Command "powershell.exe" -Arguments $validationArguments

Push-Location $workspace
try {
    Invoke-Checked -Command "npx.cmd" -Arguments @(
        "tauri", "build",
        "--config", "apps/desktop/src-tauri/tauri.conf.json",
        "--bundles", "nsis"
    )
} finally {
    Pop-Location
}

$tauriConfig = Get-Content -LiteralPath (Join-Path $tauriRoot "tauri.conf.json") -Raw | ConvertFrom-Json
$version = [string]$tauriConfig.version
$builtInstaller = Get-ChildItem -LiteralPath (Join-Path $tauriRoot "target\release\bundle\nsis") -File -Filter "*.exe" |
    Where-Object LastWriteTimeUtc -GE (Get-Date).ToUniversalTime().AddMinutes(-15) |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1
if (-not $builtInstaller) {
    throw "Tauri meldete Erfolg, aber der neue Windows-Installer wurde nicht gefunden."
}

$artifactDirectory = Join-Path $workspace "artifacts\windows"
New-Item -ItemType Directory -Force -Path $artifactDirectory | Out-Null
$artifactPath = Join-Path $artifactDirectory "E-Rechnungs-Assistent-$version-x64-Setup.exe"
Copy-Item -LiteralPath $builtInstaller.FullName -Destination $artifactPath -Force
$installerSignature = Get-AuthenticodeSignature -LiteralPath $artifactPath
if ($SigningMode -eq "Production" -and $installerSignature.Status -ne "Valid") {
    throw "Der Produktionsinstaller ist nicht gültig signiert. Konfiguriere vor dem Produktionsbuild Tauri bundle.windows.signCommand."
}
$hash = (Get-FileHash -LiteralPath $artifactPath -Algorithm SHA256).Hash.ToLowerInvariant()
$checksumPath = "$artifactPath.sha256"
[System.IO.File]::WriteAllText($checksumPath, "$hash *$([System.IO.Path]::GetFileName($artifactPath))`r`n", [System.Text.UTF8Encoding]::new($false))

Write-Host ""
Write-Host "Gemeinsamer Windows-Installer:"
Write-Host $artifactPath
Write-Host "SHA-256: $hash"
if ($SigningMode -eq "Development") {
    Write-Warning "Das Setup enthält das lokale Testzertifikat und ist nur für Entwicklungstests bestimmt."
}
