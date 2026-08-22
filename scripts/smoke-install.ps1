[CmdletBinding()]
param(
    [string]$PackageRoot,
    [string]$CertificatePath,
    [int]$QueueTimeoutSeconds = 30,
    [int]$PrintTimeoutSeconds = 45,
    [switch]$TestPrint
)

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path $PSScriptRoot -Parent

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "Der Installations-Smoke-Test muss als Administrator ausgeführt werden."
}

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

function Wait-ForPrintArtifact {
    param(
        [string]$PrintJobsPath,
        [int]$TimeoutSeconds
    )

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $pdf = Get-ChildItem -Path $PrintJobsPath -File -Filter "*.pdf" -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTimeUtc -Descending |
            Select-Object -First 1
        if ($pdf) {
            $jsonPath = [System.IO.Path]::ChangeExtension($pdf.FullName, ".json")
            if (Test-Path $jsonPath) {
                $header = [System.IO.File]::ReadAllBytes($pdf.FullName) | Select-Object -First 5
                $headerText = [System.Text.Encoding]::ASCII.GetString([byte[]]$header)
                if ($headerText -ne "%PDF-") {
                    throw "Die erzeugte Datei besitzt keinen gültigen PDF-Header: $($pdf.FullName)"
                }

                $metadata = Get-Content -Raw -Path $jsonPath | ConvertFrom-Json
                if ($metadata.documentName -ne "E-Rechnung automated smoke test") {
                    throw "Der erzeugte Job enthält einen unerwarteten Dokumentnamen: $($metadata.documentName)"
                }

                return $pdf
            }
        }

        Start-Sleep -Seconds 1
    } while ([DateTimeOffset]::UtcNow -lt $deadline)

    throw "Innerhalb von $TimeoutSeconds Sekunden wurde kein PDF/JSON-Dateipaar erzeugt."
}

$trustedCertificate = $null
$removeTrustedCertificate = $false
$installedPackage = $null
$spoolerWasRunning = $null

try {
    $spooler = Get-Service -Name "Spooler"
    $spoolerWasRunning = $spooler.Status -eq "Running"
    if (-not $spoolerWasRunning) {
        Start-Service -Name "Spooler"
    }

    $certificateInfo = Get-PfxCertificate -FilePath $CertificatePath
    $trustedCertificatePath = "Cert:\LocalMachine\TrustedPeople\$($certificateInfo.Thumbprint)"
    if (-not (Test-Path $trustedCertificatePath)) {
        $trustedCertificate = Import-Certificate `
            -FilePath $CertificatePath `
            -CertStoreLocation "Cert:\LocalMachine\TrustedPeople"
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

    if ($TestPrint) {
        $printJobsPath = Join-Path `
            $env:LOCALAPPDATA `
            "Packages\$($installedPackage.PackageFamilyName)\LocalState\ERechnung\PrintJobs"
        & dotnet run `
            --project (Join-Path $repositoryRoot "tests\PrintSmokeSender\PrintSmokeSender.csproj") `
            --configuration Release
        if ($LASTEXITCODE -ne 0) {
            throw "Der automatisierte Testdruck konnte nicht gesendet werden."
        }

        $pdf = Wait-ForPrintArtifact -PrintJobsPath $printJobsPath -TimeoutSeconds $PrintTimeoutSeconds
        Write-Host "Print pipeline created PDF: $($pdf.FullName)"
    }
} finally {
    try {
        if (-not $installedPackage) {
            $installedPackage = Get-AppxPackage -Name "ERechnung.VirtualPrinter.PoC"
        }
        if ($installedPackage) {
            Get-Process -Name "CompanionApp" -ErrorAction SilentlyContinue | Stop-Process -Force
            Remove-AppxPackage -Package $installedPackage.PackageFullName
            Wait-ForPrinter -ShouldExist $false -TimeoutSeconds $QueueTimeoutSeconds | Out-Null
            Write-Host "Package removed and printer queue disappeared."
        }
    } finally {
        try {
            if ($removeTrustedCertificate -and $trustedCertificate) {
                Remove-Item -Path "Cert:\LocalMachine\TrustedPeople\$($trustedCertificate.Thumbprint)" -Force
            }
        } finally {
            if ($spoolerWasRunning -eq $false) {
                Stop-Service -Name "Spooler"
            }
        }
    }
}
