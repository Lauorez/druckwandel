[CmdletBinding()]
param(
    [string]$PackageRoot,
    [string]$CertificatePath,
    [int]$QueueTimeoutSeconds = 30
)

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path $PSScriptRoot -Parent

if (-not $PackageRoot) {
    $PackageRoot = Join-Path $repositoryRoot "artifacts\packages"
}
if (-not $CertificatePath) {
    $CertificatePath = Join-Path $repositoryRoot ".cert\ERechnung.Dev.cer"
}

$package = Get-ChildItem -Path $PackageRoot -Recurse -File -Filter "*.msix" |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1
if (-not $package) {
    throw "Kein MSIX-Paket unter $PackageRoot gefunden."
}
if (-not (Test-Path $CertificatePath)) {
    throw "Development-Zertifikat nicht gefunden: $CertificatePath"
}

if (Get-AppxPackage -Name "ERechnung.VirtualPrinter.PoC") {
    throw "Das Paket ist bereits installiert. Der Smoke-Test verändert keine bestehende Installation."
}
if (Get-Printer -Name "E-Rechnung" -ErrorAction SilentlyContinue) {
    throw "Die Queue 'E-Rechnung' existiert bereits. Der Smoke-Test verändert keine bestehende Queue."
}

function Wait-ForPrinter {
    param(
        [bool]$ShouldExist,
        [int]$TimeoutSeconds
    )

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $printer = Get-Printer -Name "E-Rechnung" -ErrorAction SilentlyContinue
        if (($ShouldExist -and $printer) -or (-not $ShouldExist -and -not $printer)) {
            return $printer
        }

        Start-Sleep -Seconds 1
    } while ([DateTimeOffset]::UtcNow -lt $deadline)

    if ($ShouldExist) {
        throw "Die Druckerqueue 'E-Rechnung' erschien nicht innerhalb von $TimeoutSeconds Sekunden."
    }

    throw "Die Druckerqueue 'E-Rechnung' verschwand nicht innerhalb von $TimeoutSeconds Sekunden."
}

$trustedCertificate = $null
$removeTrustedCertificate = $false
$installedPackage = $null

try {
    $spooler = Get-Service -Name "Spooler"
    if ($spooler.Status -ne "Running") {
        Start-Service -Name "Spooler"
    }

    $certificateInfo = Get-PfxCertificate -FilePath $CertificatePath
    $trustedCertificatePath = "Cert:\CurrentUser\TrustedPeople\$($certificateInfo.Thumbprint)"
    if (-not (Test-Path $trustedCertificatePath)) {
        $trustedCertificate = Import-Certificate `
            -FilePath $CertificatePath `
            -CertStoreLocation "Cert:\CurrentUser\TrustedPeople"
        $removeTrustedCertificate = $true
    }

    Add-AppxPackage -Path $package.FullName -ForceApplicationShutdown
    $installedPackage = Get-AppxPackage -Name "ERechnung.VirtualPrinter.PoC"
    if (-not $installedPackage) {
        throw "Add-AppxPackage meldete keinen Fehler, das Paket ist aber nicht registriert."
    }

    $printer = Wait-ForPrinter -ShouldExist $true -TimeoutSeconds $QueueTimeoutSeconds
    Write-Host "Package installed: $($installedPackage.PackageFullName)"
    Write-Host "Printer registered: $($printer.Name)"
} finally {
    if (-not $installedPackage) {
        $installedPackage = Get-AppxPackage -Name "ERechnung.VirtualPrinter.PoC"
    }
    if ($installedPackage) {
        Remove-AppxPackage -Package $installedPackage.PackageFullName
        Wait-ForPrinter -ShouldExist $false -TimeoutSeconds $QueueTimeoutSeconds | Out-Null
        Write-Host "Package removed and printer queue disappeared."
    }

    if ($removeTrustedCertificate -and $trustedCertificate) {
        Remove-Item -Path "Cert:\CurrentUser\TrustedPeople\$($trustedCertificate.Thumbprint)" -Force
    }
}
