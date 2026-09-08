[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$PackagePath,

    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$DependencyPath,

    [string]$CertificatePath = "",

    [ValidateRange(10, 180)]
    [int]$QueueTimeoutSeconds = 45,

    [switch]$ValidateOnly
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$logPath = Join-Path $env:TEMP "E-Rechnungs-Assistent-Installation.log"
$packageName = "ERechnung.VirtualPrinter.PoC"
$printerName = "E-Rechnung"
$installedDuringThisRun = $false
$packageExistedBeforeRun = $false

function Write-SetupLog {
    param([Parameter(Mandatory = $true)][string]$Message)

    $line = "{0}  {1}" -f (Get-Date).ToString("o"), $Message
    Add-Content -LiteralPath $logPath -Value $line -Encoding UTF8
    Write-Output $Message
}

function Get-PackageIdentity {
    param([Parameter(Mandatory = $true)][string]$Path)

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [System.IO.Compression.ZipFile]::OpenRead($Path)
    try {
        $manifestEntry = $archive.GetEntry("AppxManifest.xml")
        if (-not $manifestEntry) {
            throw "Das Druckerpaket enthält kein AppxManifest.xml."
        }

        $reader = [System.IO.StreamReader]::new($manifestEntry.Open())
        try {
            [xml]$manifest = $reader.ReadToEnd()
        } finally {
            $reader.Dispose()
        }

        return [pscustomobject]@{
            Name      = [string]$manifest.Package.Identity.Name
            Publisher = [string]$manifest.Package.Identity.Publisher
            Version   = [version]([string]$manifest.Package.Identity.Version)
        }
    } finally {
        $archive.Dispose()
    }
}

function Test-CertificateTrusted {
    param([Parameter(Mandatory = $true)][string]$Thumbprint)

    $stores = @(
        "Cert:\CurrentUser\TrustedPeople",
        "Cert:\CurrentUser\Root",
        "Cert:\LocalMachine\TrustedPeople"
    )
    foreach ($store in $stores) {
        $match = Get-ChildItem -Path $store -ErrorAction SilentlyContinue |
            Where-Object Thumbprint -EQ $Thumbprint |
            Select-Object -First 1
        if ($match) {
            return $true
        }
    }
    return $false
}

function Install-DevelopmentCertificate {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Thumbprint
    )

    if (Test-CertificateTrusted -Thumbprint $Thumbprint) {
        return
    }

    Write-SetupLog "Das Testzertifikat wird nur für das aktuelle Benutzerkonto übernommen."
    Import-Certificate -FilePath $Path -CertStoreLocation "Cert:\CurrentUser\TrustedPeople" | Out-Null
    Import-Certificate -FilePath $Path -CertStoreLocation "Cert:\CurrentUser\Root" | Out-Null
    if (-not (Test-CertificateTrusted -Thumbprint $Thumbprint)) {
        throw "Das Testzertifikat konnte nicht im Benutzer-Zertifikatspeicher abgelegt werden."
    }
}

function Wait-ForPrinter {
    param([Parameter(Mandatory = $true)][int]$TimeoutSeconds)

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $printer = Get-Printer -Name $printerName -ErrorAction SilentlyContinue
        if ($printer) {
            return $printer
        }
        Start-Sleep -Milliseconds 500
    } while ([DateTimeOffset]::UtcNow -lt $deadline)

    return $null
}

try {
    Set-Content -LiteralPath $logPath -Value "E-Rechnungs-Assistent – Einrichtung" -Encoding UTF8
    # NSIS can be started with a reduced PSModulePath. Use the modules belonging
    # to this exact process instead of relying on automatic module discovery.
    Import-Module (Join-Path $PSHOME "Modules\Microsoft.PowerShell.Security\Microsoft.PowerShell.Security.psd1") -Force
    Import-Module (Join-Path $PSHOME "Modules\Appx\Appx.psd1") -Force
    Import-Module (Join-Path $PSHOME "Modules\PrintManagement\PrintManagement.psd1") -Force
    Write-SetupLog "Das Druckerpaket wird geprüft."

    $currentBuild = [int](Get-ItemPropertyValue -Path "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion" -Name CurrentBuildNumber)
    if ($currentBuild -lt 26100) {
        throw "Windows 11 Version 24H2 oder neuer ist erforderlich. Gefundener Build: $currentBuild."
    }
    if (-not (Test-Path -LiteralPath $PackagePath -PathType Leaf)) {
        throw "Das Druckerpaket fehlt."
    }
    if (-not (Test-Path -LiteralPath $DependencyPath -PathType Leaf)) {
        throw "Eine für den Drucker benötigte Windows-Komponente fehlt."
    }

    $identity = Get-PackageIdentity -Path $PackagePath
    if ($identity.Name -ne $packageName) {
        throw "Unerwartetes Druckerpaket: $($identity.Name)."
    }

    $packageSignature = Get-AuthenticodeSignature -LiteralPath $PackagePath
    if (-not $packageSignature.SignerCertificate -or $packageSignature.Status -eq "HashMismatch") {
        throw "Die Signatur des Druckerpakets ist ungültig."
    }
    if ($packageSignature.SignerCertificate.Subject -ne $identity.Publisher) {
        throw "Herausgeber und Paketsignatur stimmen nicht überein."
    }

    $dependencySignature = Get-AuthenticodeSignature -LiteralPath $DependencyPath
    if ($dependencySignature.Status -ne "Valid") {
        throw "Die Signatur der benötigten Windows-Komponente ist ungültig: $($dependencySignature.Status)."
    }

    if ($CertificatePath) {
        if (-not (Test-Path -LiteralPath $CertificatePath -PathType Leaf)) {
            throw "Das Testzertifikat fehlt."
        }
        $certificate = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new($CertificatePath)
        if ($certificate.Thumbprint -ne $packageSignature.SignerCertificate.Thumbprint) {
            throw "Testzertifikat und Druckerpaket gehören nicht zusammen."
        }
        if (-not $ValidateOnly) {
            Install-DevelopmentCertificate -Path $CertificatePath -Thumbprint $certificate.Thumbprint
            $packageSignature = Get-AuthenticodeSignature -LiteralPath $PackagePath
        }
    }

    if (-not $ValidateOnly -and $packageSignature.Status -ne "Valid") {
        Write-SetupLog "Windows bewertet die Paketsignatur als $($packageSignature.Status). Die Anmeldung wird trotzdem für den aktuellen Benutzer versucht."
    }
    if ($ValidateOnly) {
        Write-SetupLog "Druckerpaket, Abhängigkeit und Signaturen sind vollständig."
        exit 0
    }

    $installedPackages = @(Get-AppxPackage -Name $packageName | Sort-Object Version -Descending)
    $installedPackage = $installedPackages | Select-Object -First 1
    $packageExistedBeforeRun = [bool]$installedPackage
    $printer = Get-Printer -Name $printerName -ErrorAction SilentlyContinue

    if ($installedPackage -and [version]$installedPackage.Version -gt $identity.Version) {
        if (-not $printer) {
            throw "Eine neuere Druckerversion ist vorhanden, aber der Drucker fehlt. Bitte reparieren Sie zuerst die vorhandene Installation."
        }
        Write-SetupLog "Eine neuere Druckerversion ist bereits eingerichtet."
        exit 0
    }

    if ($installedPackage -and [version]$installedPackage.Version -eq $identity.Version -and $printer) {
        Write-SetupLog "Der E-Rechnungsdrucker ist bereits eingerichtet."
        exit 0
    }

    $activeJobs = @()
    if ($printer) {
        $activeJobs = @(Get-PrintJob -PrinterName $printerName -ErrorAction SilentlyContinue)
    }
    if ($activeJobs.Count -gt 0) {
        throw "Die Einrichtung kann nicht fortgesetzt werden, solange noch ein Druckauftrag läuft."
    }

    if ($installedPackage -and [version]$installedPackage.Version -eq $identity.Version) {
        Write-SetupLog "Eine unvollständige Druckerregistrierung wird repariert."
        Remove-AppxPackage -Package $installedPackage.PackageFullName
        $deadline = [DateTimeOffset]::UtcNow.AddSeconds(30)
        do {
            Start-Sleep -Milliseconds 500
        } while ((Get-AppxPackage -Name $packageName) -and [DateTimeOffset]::UtcNow -lt $deadline)
        if (Get-AppxPackage -Name $packageName) {
            throw "Die beschädigte Druckerregistrierung konnte nicht entfernt werden."
        }
    }

    Write-SetupLog "Der E-Rechnungsdrucker wird bei Windows angemeldet."
    Add-AppxPackage `
        -Path $PackagePath `
        -DependencyPath $DependencyPath `
        -ForceApplicationShutdown
    $installedDuringThisRun = $true

    $printer = Wait-ForPrinter -TimeoutSeconds $QueueTimeoutSeconds
    if (-not $printer) {
        throw "Windows hat den Drucker nicht innerhalb von $QueueTimeoutSeconds Sekunden bereitgestellt."
    }

    $workflowServices = @(Get-Service -Name "PrintWorkflowUserSvc*" -ErrorAction SilentlyContinue)
    if ($workflowServices.Count -eq 0) {
        throw "Der Windows-Druckdienst wurde nicht gefunden."
    }
    foreach ($workflowService in $workflowServices) {
        try {
            if ($workflowService.Status -eq "Running") {
                Restart-Service -InputObject $workflowService -Force -ErrorAction Stop
            } else {
                Start-Service -InputObject $workflowService -ErrorAction Stop
            }
        } catch {
            Write-SetupLog "Der Druckdienst $($workflowService.Name) konnte ohne erhöhte Rechte nicht neu gestartet werden."
        }
    }
    $stoppedServices = @(Get-Service -Name "PrintWorkflowUserSvc*" | Where-Object Status -NE "Running")
    if ($stoppedServices.Count -gt 0 -and -not (Get-Printer -Name $printerName -ErrorAction SilentlyContinue)) {
        throw "Der Windows-Druckdienst konnte nicht gestartet werden."
    }

    Write-SetupLog "Der E-Rechnungsdrucker ist bereit."
    exit 0
} catch {
    $message = $_.Exception.Message
    try {
        Write-SetupLog "FEHLER: $message"
        if ($installedDuringThisRun -and -not $packageExistedBeforeRun) {
            Get-AppxPackage -Name $packageName -ErrorAction SilentlyContinue |
                Remove-AppxPackage -ErrorAction SilentlyContinue
            Write-SetupLog "Die unvollständige Druckerinstallation wurde zurückgenommen."
        }
    } catch {
        # Preserve the original installation error even if logging or rollback fails.
    }
    Write-Error $message
    exit 1
}
