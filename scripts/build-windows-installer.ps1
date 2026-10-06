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
# npm/PowerShell 7 can pass a module search path that does not belong to the
# Windows PowerShell process running this build. Bind security commands to it.
Import-Module (Join-Path $PSHOME "Modules\Microsoft.PowerShell.Security\Microsoft.PowerShell.Security.psd1") -Force
Import-Module (Join-Path $PSHOME "Modules\Microsoft.PowerShell.Utility\Microsoft.PowerShell.Utility.psd1") -Force
$workspace = Split-Path -Parent $PSScriptRoot
$tauriRoot = Join-Path $workspace "apps\desktop\src-tauri"
$payloadRoot = Join-Path $tauriRoot "installer\windows\payload"
$printerPackageRoots = @(
    (Join-Path $workspace "drucker\artifacts\packages")
)
$defaultCertificate = Join-Path $workspace "drucker\.cert\ERechnung.Dev.cer"

$validatorRoot = Join-Path $tauriRoot "resources\validators"
$validatorManifest = Get-Content -LiteralPath (Join-Path $validatorRoot "manifest.json") -Raw | ConvertFrom-Json
foreach ($resource in @("jre/bin/java.exe", $validatorManifest.kosit.jar, $validatorManifest.kosit.scenarios, $validatorManifest.mustang.jar, $validatorManifest.verapdf.jar)) {
    if (-not (Test-Path -LiteralPath (Join-Path $validatorRoot $resource) -PathType Leaf)) {
        throw "Das vollständige Windows-Prüfpaket fehlt ($resource). Zuerst npm run validators:fetch auf Windows ausführen."
    }
}

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
    Invoke-Checked -Command "npm.cmd" -Arguments @("run", "release:gate")
} elseif ($SigningMode -eq "Production") {
    throw "Ein Produktionsbuild darf das Release-Gate nicht überspringen."
}

if (-not $PrinterPackagePath) {
    if (-not $SkipPrinterBuild) {
        Invoke-Checked -Command "powershell.exe" -Arguments @(
            "-NoProfile",
            "-ExecutionPolicy", "Bypass",
            "-File", (Join-Path $workspace "scripts\printer-build.ps1")
        )
    }

    $printerPackage = $printerPackageRoots | ForEach-Object {
        if (Test-Path -LiteralPath $_) {
            Get-ChildItem -Path $_ -Recurse -File -Filter "CompanionApp_*.msix" -ErrorAction SilentlyContinue |
                Where-Object FullName -NotMatch "[\\/]Dependencies[\\/]"
        }
    } | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
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
$trustScriptContent = [System.IO.File]::ReadAllText((Join-Path $tauriRoot "installer\windows\TrustPrinterCertificate.ps1"), $utf8WithoutBom)
[System.IO.File]::WriteAllText((Join-Path $payloadRoot "TrustPrinterCertificate.ps1"), $trustScriptContent, $utf8WithBom)
[System.IO.File]::WriteAllText((Join-Path $payloadRoot "RemovePrinter.ps1"), $removeScriptContent, $utf8WithBom)
$updateGuardContent = [System.IO.File]::ReadAllText((Join-Path $tauriRoot "installer\windows\UpdateGuard.ps1"), $utf8WithoutBom)
[System.IO.File]::WriteAllText((Join-Path $payloadRoot "UpdateGuard.ps1"), $updateGuardContent, $utf8WithBom)

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
$nsisDirectories = @(
    (Join-Path $tauriRoot "target\release\bundle\nsis")
)
if ($env:CARGO_TARGET_DIR) {
    $nsisDirectories = @((Join-Path $env:CARGO_TARGET_DIR "release\bundle\nsis")) + $nsisDirectories
}
$builtInstaller = $nsisDirectories | ForEach-Object {
    if (Test-Path -LiteralPath $_) {
        Get-ChildItem -LiteralPath $_ -File -Filter "*.exe" |
            Where-Object LastWriteTimeUtc -GE (Get-Date).ToUniversalTime().AddMinutes(-30)
    }
} | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
if (-not $builtInstaller) {
    throw "Tauri meldete Erfolg, aber der neue Windows-Installer wurde nicht gefunden."
}

$artifactDirectory = Join-Path $workspace "artifacts\windows"
New-Item -ItemType Directory -Force -Path $artifactDirectory | Out-Null
$artifactPath = Join-Path $artifactDirectory "Druckwandel-$version-x64-Setup.exe"
Copy-Item -LiteralPath $builtInstaller.FullName -Destination $artifactPath -Force
if ($env:ERECHNUNG_SIGNTOOL -and $SigningMode -eq "Production") {
    Invoke-Checked -Command $env:ERECHNUNG_SIGNTOOL -Arguments @("sign", "/fd", "SHA256", "/td", "SHA256", "/tr", "http://timestamp.digicert.com", $artifactPath)
}
$installerSignature = Get-AuthenticodeSignature -LiteralPath $artifactPath
if ($SigningMode -eq "Production") {
    if ($installerSignature.Status -ne "Valid") {
        throw "Der Produktionsinstaller ist nicht gültig signiert. Setze ERECHNUNG_SIGNTOOL oder Tauri bundle.windows.signCommand."
    }
    if ($installerSignature.SignerCertificate.Subject -eq $installerSignature.SignerCertificate.Issuer) {
        throw "Ein Test- oder selbstsigniertes Zertifikat darf den Produktionsinstaller nicht signieren."
    }
}
$hash = (Get-FileHash -LiteralPath $artifactPath -Algorithm SHA256).Hash.ToLowerInvariant()
$checksumPath = "$artifactPath.sha256"
[System.IO.File]::WriteAllText($checksumPath, "$hash *$([System.IO.Path]::GetFileName($artifactPath))`r`n", [System.Text.UTF8Encoding]::new($false))

Write-Host ""
Write-Host "Gemeinsamer Windows-Installer:"
Write-Host $artifactPath
Write-Host "SHA-256: $hash"
if ($SigningMode -eq "Development") {
    Write-Warning "Vorführbuild: Auf einem fremden Rechner muss das Entwicklungszertifikat bereits von der IT freigegeben sein. Ohne diese Vorbedingung ist für die Installation ohne Admin eine öffentlich vertrauenswürdige Signatur erforderlich."
}
