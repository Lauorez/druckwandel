[CmdletBinding()]
param(
    [string]$InstallerPath = ""
)

$ErrorActionPreference = "Stop"
$workspace = Split-Path -Parent $PSScriptRoot
if (-not $InstallerPath) {
    $InstallerPath = Join-Path $workspace "artifacts\windows"
    $found = Get-ChildItem -LiteralPath $InstallerPath -Filter "E-Rechnungs-Assistent-*-x64-Setup.exe" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1
    if (-not $found) {
        throw "Kein Windows-Installer unter artifacts/windows. Zuerst npm run installer:windows -SigningMode Production."
    }
    $InstallerPath = $found.FullName
}

function Assert-TrustedSignature {
    param([string]$Path, [string]$Role)
    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($signature.Status -ne "Valid") {
        throw "$Role ist nicht gültig signiert ($($signature.Status))."
    }
    if ($signature.SignerCertificate.Subject -eq $signature.SignerCertificate.Issuer) {
        throw "$Role verwendet ein Test- oder selbstsigniertes Zertifikat. Das ist kein Release."
    }
}

Assert-TrustedSignature -Path $InstallerPath -Role "Der Setup"
$payloadHint = Join-Path $workspace "apps\desktop\src-tauri\installer\windows\payload\Printer.msix"
if (Test-Path -LiteralPath $payloadHint) {
    Assert-TrustedSignature -Path $payloadHint -Role "Das Druckerpaket"
}
Write-Output "Produktionssignaturen sind gültig: $InstallerPath"
