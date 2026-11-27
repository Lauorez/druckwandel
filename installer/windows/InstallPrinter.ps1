[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$PackagePath,
    [Parameter(Mandatory = $true)]
    [string]$DependencyPath,
    [string]$CertificatePath = ""
)

$ErrorActionPreference = "Stop"

try {
    # NSIS can be started with a reduced PSModulePath.
    Import-Module (Join-Path $PSHOME "Modules\Appx\Appx.psd1") -Force
    Import-Module (Join-Path $PSHOME "Modules\PrintManagement\PrintManagement.psd1") -Force

    if (@(Get-PrintJob -PrinterName "E-Rechnung" -ErrorAction SilentlyContinue).Count -gt 0) {
        throw "Aktiver Druckauftrag läuft. Bitte schließen oder brechen Sie den Druckauftrag ab und versuchen Sie es erneut."
    }

    if ($CertificatePath) {
        $cert = [Security.Cryptography.X509Certificates.X509Certificate2]::new($CertificatePath)
        $store = [Security.Cryptography.X509Certificates.X509Store]::new("TrustedPeople", "LocalMachine")
        $store.Open("ReadWrite")
        try {
            $existing = @($store.Certificates | Where-Object { $_.Thumbprint -eq $cert.Thumbprint })
            if ($existing.Count -eq 0) {
                $store.Add($cert)
            }
            # Remove older self-signed development certificates from previous releases so updates stay clean.
            if ($cert.Subject -eq "CN=ERechnung Development" -and $cert.Issuer -eq "CN=ERechnung Development") {
                $stale = @($store.Certificates | Where-Object {
                    $_.Subject -eq "CN=ERechnung Development" -and
                    $_.Issuer -eq "CN=ERechnung Development" -and
                    $_.Thumbprint -ne $cert.Thumbprint
                })
                foreach ($oldCert in $stale) {
                    $store.Remove($oldCert)
                }
            }
        }
        finally {
            $store.Close()
        }
    }

    # Remove any already installed printer package before reinstalling so updates with the same
    # MSIX version or a freshly generated development certificate do not fail with 0x80073CFB.
    $packageNames = @("Druckwandel.VirtualPrinter", "ERechnung.VirtualPrinter.PoC")
    $hadExisting = $false
    foreach ($name in $packageNames) {
        foreach ($pkg in @(Get-AppxPackage -AllUsers -Name $name -ErrorAction SilentlyContinue | Where-Object { $_ })) {
            $hadExisting = $true
            try {
                Remove-AppxPackage -Package $pkg.PackageFullName -AllUsers -ErrorAction Stop
            }
            catch {
                Remove-AppxPackage -Package $pkg.PackageFullName -ErrorAction SilentlyContinue
            }
        }
        foreach ($pkg in @(Get-AppxPackage -Name $name -ErrorAction SilentlyContinue | Where-Object { $_ })) {
            $hadExisting = $true
            Remove-AppxPackage -Package $pkg.PackageFullName -ErrorAction Stop
        }
    }

    if ($hadExisting) {
        $removeDeadline = (Get-Date).AddSeconds(30)
        do {
            $remainingPkgs = @(
                foreach ($name in $packageNames) {
                    Get-AppxPackage -AllUsers -Name $name -ErrorAction SilentlyContinue | Where-Object { $_ }
                    Get-AppxPackage -Name $name -ErrorAction SilentlyContinue | Where-Object { $_ }
                }
            )
            $remainingPrinter = Get-Printer -Name "E-Rechnung" -ErrorAction SilentlyContinue
            if ($remainingPkgs.Count -eq 0 -and -not $remainingPrinter) {
                break
            }
            Start-Sleep -Milliseconds 500
        } while ((Get-Date) -lt $removeDeadline)

        $stillInstalled = @(
            foreach ($name in $packageNames) {
                Get-AppxPackage -AllUsers -Name $name -ErrorAction SilentlyContinue | Where-Object { $_ }
                Get-AppxPackage -Name $name -ErrorAction SilentlyContinue | Where-Object { $_ }
            }
        )
        if ($stillInstalled.Count -gt 0) {
            throw "Das bisherige Druckerpaket konnte nicht vollständig entfernt werden."
        }
    }

    Add-AppxPackage `
        -Path $PackagePath `
        -DependencyPath $DependencyPath `
        -ForceApplicationShutdown `
        -ForceUpdateFromAnyVersion

    $readyDeadline = (Get-Date).AddSeconds(45)
    while (-not (Get-Printer -Name "E-Rechnung" -ErrorAction SilentlyContinue) -and (Get-Date) -lt $readyDeadline) {
        Start-Sleep -Milliseconds 500
    }
    if (-not (Get-Printer -Name "E-Rechnung" -ErrorAction SilentlyContinue)) {
        throw "Drucker E-Rechnung wurde nicht rechtzeitig bereitgestellt."
    }

    foreach ($svc in @(Get-Service -Name "PrintWorkflowUserSvc*" -ErrorAction SilentlyContinue)) {
        try {
            if ($svc.Status -eq "Running") {
                Restart-Service -InputObject $svc -Force -ErrorAction Stop
            }
            else {
                Start-Service -InputObject $svc -ErrorAction Stop
            }
        }
        catch {
            # Best-effort: printer queue is already registered.
        }
    }

    exit 0
}
catch {
    Write-Output $_.Exception.Message
    exit 1
}
