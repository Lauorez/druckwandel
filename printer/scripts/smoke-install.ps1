[CmdletBinding()]
param(
    [string]$PackageRoot,
    [string]$CertificatePath,
    [string]$SmokeArtifactRoot,
    [int]$QueueTimeoutSeconds = 30,
    [int]$PrintTimeoutSeconds = 45,
    [switch]$TestPrint,
    [switch]$TestMatrix
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
if (-not $SmokeArtifactRoot) {
    $SmokeArtifactRoot = Join-Path $repositoryRoot "artifacts\smoke"
}
$runPrintTest = $TestPrint -or $TestMatrix
if ($runPrintTest) {
    New-Item -ItemType Directory -Force -Path $SmokeArtifactRoot | Out-Null
}

$package = Get-ChildItem -Path $PackageRoot -Recurse -File -Filter "*.msix" |
    Where-Object { $_.FullName -notmatch "[\\/]Dependencies[\\/]" } |
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
        [int]$TimeoutSeconds,
        [string]$ExpectedDocumentName,
        [int]$ExpectedPages,
        [DateTimeOffset]$SubmittedAfter
    )

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $pdf = Get-ChildItem -Path $PrintJobsPath -File -Filter "*.pdf" -ErrorAction SilentlyContinue |
            Where-Object { $_.LastWriteTimeUtc -ge $SubmittedAfter.UtcDateTime } |
            Sort-Object LastWriteTimeUtc -Descending |
            Select-Object -First 20
        foreach ($candidate in $pdf) {
            $pdf = $candidate
            $jsonPath = [System.IO.Path]::ChangeExtension($pdf.FullName, ".json")
            if (Test-Path $jsonPath) {
                $metadata = Get-Content -Raw -Path $jsonPath | ConvertFrom-Json
                if ($metadata.documentName -ne $ExpectedDocumentName) {
                    continue
                }

                $logPath = Join-Path `
                    (Join-Path (Split-Path $PrintJobsPath -Parent) "Logs") `
                    "$($metadata.jobId).jsonl"
                $conversionSucceeded = (Test-Path $logPath) -and
                    ((Get-Content -Raw -Path $logPath) -match '"status":"pdfConversionSucceeded"')
                if ($conversionSucceeded) {
                    if ($metadata.pages -ne $ExpectedPages) {
                        throw "Der Job '$ExpectedDocumentName' enthält $($metadata.pages) statt $ExpectedPages Seiten."
                    }

                    $header = [System.IO.File]::ReadAllBytes($pdf.FullName) | Select-Object -First 5
                    $headerText = [System.Text.Encoding]::ASCII.GetString([byte[]]$header)
                    if ($headerText -ne "%PDF-") {
                        throw "Die erzeugte Datei besitzt keinen gültigen PDF-Header: $($pdf.FullName)"
                    }

                    return $pdf
                }
            }
        }

        Start-Sleep -Seconds 1
    } while ([DateTimeOffset]::UtcNow -lt $deadline)

    throw "Innerhalb von $TimeoutSeconds Sekunden wurde kein PDF/JSON-Dateipaar für '$ExpectedDocumentName' erzeugt."
}

$trustedCertificate = $null
$removeTrustedCertificate = $false
$installedPackage = $null
$spoolerWasRunning = $null
$localStateRoot = $null
$smokeStartedAt = [DateTimeOffset]::UtcNow

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

    $dependencyArchitecture = if ($package.Name -match "arm64") { "arm64" } else { "x64" }
    $dependencyPath = Join-Path $package.DirectoryName "Dependencies\$dependencyArchitecture"
    $dependencies = @(
        Get-ChildItem -Path $dependencyPath -File -Filter "*.msix" -ErrorAction SilentlyContinue |
            Select-Object -ExpandProperty FullName
    )
    $installParameters = @{
        Path                     = $package.FullName
        ForceApplicationShutdown = $true
    }
    if ($dependencies.Count -gt 0) {
        $installParameters.DependencyPath = $dependencies
    }

    Add-AppxPackage @installParameters
    $installedPackage = Get-AppxPackage -Name "ERechnung.VirtualPrinter.PoC"
    if (-not $installedPackage) {
        throw "Add-AppxPackage meldete keinen Fehler, das Paket ist aber nicht registriert."
    }
    $localStateRoot = Join-Path `
        $env:LOCALAPPDATA `
        "Packages\$($installedPackage.PackageFamilyName)\LocalState"

    $printer = Wait-ForPrinter -ShouldExist $true -TimeoutSeconds $QueueTimeoutSeconds
    $workflowServices = @(Get-Service -Name "PrintWorkflowUserSvc*" -ErrorAction SilentlyContinue)
    if ($workflowServices.Count -eq 0) {
        throw "Windows PrintWorkflowUserSvc wurde nicht gefunden."
    }
    foreach ($workflowService in $workflowServices) {
        if ($workflowService.Status -eq "Running") {
            Restart-Service -InputObject $workflowService -Force
        } else {
            Start-Service -InputObject $workflowService
        }
    }
    if (Get-Service -Name "PrintWorkflowUserSvc*" | Where-Object Status -ne "Running") {
        throw "Windows PrintWorkflowUserSvc läuft nach der Installation nicht."
    }

    Write-Host "Package installed: $($installedPackage.PackageFullName)"
    Write-Host "Printer registered: $($printer.Name)"

    if ($runPrintTest) {
        $printJobsPath = Join-Path $localStateRoot "ERechnung\PrintJobs"
        $submittedAfter = [DateTimeOffset]::UtcNow
        & dotnet run `
            --project (Join-Path $repositoryRoot "tests\PrintSmokeSender\PrintSmokeSender.csproj") `
            --configuration Release
        if ($LASTEXITCODE -ne 0) {
            throw "Der automatisierte Testdruck konnte nicht gesendet werden."
        }

        $pdf = Wait-ForPrintArtifact `
            -PrintJobsPath $printJobsPath `
            -TimeoutSeconds $PrintTimeoutSeconds `
            -ExpectedDocumentName "E-Rechnung automated smoke test" `
            -ExpectedPages 1 `
            -SubmittedAfter $submittedAfter
        Write-Host "Print pipeline created PDF: $($pdf.FullName)"

        if ($TestMatrix) {
            $scenarios = @(
                @{ Name = "multipage"; DocumentName = "E-Rechnung smoke multipage"; Pages = 3 },
                @{ Name = "landscape"; DocumentName = "E-Rechnung smoke landscape"; Pages = 1 },
                @{ Name = "visual"; DocumentName = "E-Rechnung smoke visual content"; Pages = 1 }
            )
            foreach ($scenario in $scenarios) {
                $submittedAfter = [DateTimeOffset]::UtcNow
                & dotnet run `
                    --project (Join-Path $repositoryRoot "tests\PrintSmokeSender\PrintSmokeSender.csproj") `
                    --configuration Release `
                    -- `
                    --scenario $scenario.Name
                if ($LASTEXITCODE -ne 0) {
                    throw "Das Druckszenario '$($scenario.Name)' konnte nicht gesendet werden."
                }

                $scenarioPdf = Wait-ForPrintArtifact `
                    -PrintJobsPath $printJobsPath `
                    -TimeoutSeconds $PrintTimeoutSeconds `
                    -ExpectedDocumentName $scenario.DocumentName `
                    -ExpectedPages $scenario.Pages `
                    -SubmittedAfter $submittedAfter
                Write-Host "Scenario '$($scenario.Name)' created PDF: $($scenarioPdf.FullName)"
            }

            $parallelStartedAt = [DateTimeOffset]::UtcNow
            $senderProject = Join-Path $repositoryRoot "tests\PrintSmokeSender\PrintSmokeSender.csproj"
            $parallelJobs = @()
            $parallelJobs += Start-Process dotnet -WindowStyle Hidden -PassThru -ArgumentList @(
                "run", "--project", $senderProject, "--configuration", "Release", "--no-build", "--", "--scenario", "multipage")
            $parallelJobs += Start-Process dotnet -WindowStyle Hidden -PassThru -ArgumentList @(
                "run", "--project", $senderProject, "--configuration", "Release", "--no-build", "--", "--scenario", "landscape")
            $parallelJobs | Wait-Process
            if ($parallelJobs | Where-Object { $_.ExitCode -ne 0 }) {
                throw "Mindestens ein paralleler Testsender ist fehlgeschlagen."
            }

            $parallelPdfs = @(
                Wait-ForPrintArtifact -PrintJobsPath $printJobsPath -TimeoutSeconds $PrintTimeoutSeconds `
                    -ExpectedDocumentName "E-Rechnung smoke multipage" -ExpectedPages 3 -SubmittedAfter $parallelStartedAt
                Wait-ForPrintArtifact -PrintJobsPath $printJobsPath -TimeoutSeconds $PrintTimeoutSeconds `
                    -ExpectedDocumentName "E-Rechnung smoke landscape" -ExpectedPages 1 -SubmittedAfter $parallelStartedAt
            )
            if (@($parallelPdfs | Select-Object -ExpandProperty BaseName -Unique).Count -ne 2) {
                throw "Die parallelen Druckjobs wurden nicht als zwei getrennte UUID-Dateien gespeichert."
            }
            Write-Host "Parallel scenario created two independent PDFs."
        }
    }
} finally {
    try {
        if (-not $installedPackage) {
            $installedPackage = Get-AppxPackage -Name "ERechnung.VirtualPrinter.PoC"
        }
        if ($installedPackage) {
            if ($runPrintTest) {
                $summary = [ordered]@{
                    startedAt        = $smokeStartedAt
                    collectedAt      = [DateTimeOffset]::UtcNow
                    packageFullName  = $installedPackage.PackageFullName
                    packageFamilyName = $installedPackage.PackageFamilyName
                    printerPresent   = [bool](Get-Printer -Name "E-Rechnung" -ErrorAction SilentlyContinue)
                    localStateRoot   = $localStateRoot
                }
                $summary | ConvertTo-Json | Set-Content `
                    -Path (Join-Path $SmokeArtifactRoot "summary.json")

                $productDataPath = if ($localStateRoot) {
                    Join-Path $localStateRoot "ERechnung"
                }
                if ($productDataPath -and (Test-Path $productDataPath)) {
                    Copy-Item -Path $productDataPath -Destination $SmokeArtifactRoot -Recurse -Force
                }
            }

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
