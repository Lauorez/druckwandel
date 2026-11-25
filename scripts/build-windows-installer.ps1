[CmdletBinding()]
param(
    [switch]$SkipChecks,
    [switch]$AppOnly,
    [switch]$SkipBuild,
    [ValidateSet('Development','Production')][string]$SigningMode = 'Development',
    [string]$PrinterPackagePath = '',
    [string]$PrinterDependencyPath = '',
    [string]$PrinterCertificatePath = '',
    [switch]$Uncompressed
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
Set-Location $workspace
function Invoke-Checked([string]$Command, [string[]]$Arguments) {
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Command failed ($LASTEXITCODE)" }
}
if ($SigningMode -eq 'Production' -and ($SkipChecks -or $SkipBuild -or $AppOnly -or $Uncompressed)) {
    throw 'Produktionsbuilds benötigen alle Prüfungen, einen frischen Release-Build, Komprimierung und das Druckerpaket.'
}
$metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) { throw 'Cargo-Metadaten fehlen.' }
$version = ($metadata.packages | Where-Object name -eq 'druckwandel-native').version
$payload = Join-Path $workspace 'installer\windows\payload'
if (Test-Path $payload) { Remove-Item $payload -Recurse -Force }
New-Item -ItemType Directory -Force $payload | Out-Null
if (-not $SkipChecks) {
    Invoke-Checked 'cargo' @('fmt','--all','--','--check')
    Invoke-Checked 'cargo' @('clippy','--all-targets','--locked','--','-D','warnings')
    Invoke-Checked 'cargo' @('test','--locked')
    Invoke-Checked 'cargo' @('test','--locked','--test','ocr','--','--ignored')
    Invoke-Checked 'cargo' @('test','--locked','--test','external_validators','--','--ignored')
}
if (-not $SkipBuild) {
    Invoke-Checked 'cargo' @('build','--release','--bin','druckwandel-native','--locked')
}
Copy-Item (Join-Path $metadata.target_directory 'release\druckwandel-native.exe') (Join-Path $payload 'Druckwandel.exe') -Force
$validatorRoot = Join-Path $workspace 'resources\validators'
$manifest = Get-Content (Join-Path $validatorRoot 'manifest.json') -Raw | ConvertFrom-Json
foreach ($file in @('jre/bin/java.exe',$manifest.kosit.jar,$manifest.kosit.scenarios,$manifest.mustang.jar,$manifest.verapdf.jar)) {
    if (-not (Test-Path (Join-Path $validatorRoot $file) -PathType Leaf)) {
        throw "Prüfpaket fehlt: $file. Zuerst python scripts/fetch-validators.py auf Windows ausführen."
    }
}
$resourceTarget = Join-Path $payload 'resources'
New-Item -ItemType Directory -Force $resourceTarget | Out-Null
Copy-Item $validatorRoot $resourceTarget -Recurse -Force
$ocrRoot = Join-Path $workspace 'resources\ocr'
foreach ($file in @('tesseract.exe','tessdata/deu.traineddata','tessdata/eng.traineddata','doc/LICENSE')) {
    if (-not (Test-Path (Join-Path $ocrRoot $file) -PathType Leaf)) {
        throw "OCR-Paket fehlt: $file. Zuerst python scripts/fetch-ocr.py ausführen."
    }
}
Copy-Item $ocrRoot $resourceTarget -Recurse -Force
Copy-Item 'apps/native/assets/icon.ico' $payload -Force
foreach ($file in @('LICENSE','THIRD-PARTY-NOTICES.md')) { Copy-Item $file $payload -Force }
$sessionScript = [IO.File]::ReadAllText((Join-Path $workspace 'installer/windows/AssertSessionAccount.ps1'))
[IO.File]::WriteAllText((Join-Path $payload 'AssertSessionAccount.ps1'), $sessionScript, [Text.UTF8Encoding]::new($true))
$artifactRoot = Join-Path $workspace 'artifacts\windows'
New-Item -ItemType Directory -Force $artifactRoot | Out-Null
$suffix = if ($AppOnly) { '-AppOnly' } else { '' }
$output = Join-Path $artifactRoot "Druckwandel-$version-x64$suffix-Setup.exe"
$arguments = @("/DVERSION=$version","/DPAYLOAD=$payload","/DOUTPUT=$output")
if ($Uncompressed) { $arguments += '/DUNCOMPRESSED' }
if (-not $AppOnly) {
    if (-not $PrinterPackagePath) {
        & (Join-Path $workspace 'printer\scripts\build.ps1') -Platform x64 -Configuration Release
        if ($LASTEXITCODE -ne 0) { throw "Drucker-Build fehlgeschlagen ($LASTEXITCODE)." }
        $builtPackage = Get-ChildItem (Join-Path $workspace 'printer\artifacts\packages') -Recurse -File -Filter 'CompanionApp_*.msix' |
            Where-Object { $_.FullName -notmatch '[\\/]Dependencies[\\/]' } |
            Sort-Object LastWriteTimeUtc -Descending |
            Select-Object -First 1
        if (-not $builtPackage) { throw 'Gebautes Drucker-MSIX wurde nicht gefunden.' }
        $PrinterPackagePath = $builtPackage.FullName
    }
    if (-not $PrinterDependencyPath) {
        $depDir = Join-Path (Split-Path -Parent $PrinterPackagePath) 'Dependencies\x64'
        $dependency = Get-ChildItem $depDir -File -Filter '*.msix' -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($dependency) { $PrinterDependencyPath = $dependency.FullName }
    }
    if (-not (Test-Path -LiteralPath $PrinterPackagePath -PathType Leaf) -or -not (Test-Path -LiteralPath $PrinterDependencyPath -PathType Leaf)) {
        throw 'Druckerpaket (Printer.msix) oder WindowsAppRuntime.msix fehlt.'
    }
    $depSignature = Get-AuthenticodeSignature -LiteralPath $PrinterDependencyPath
    if ($depSignature.Status -ne 'Valid') {
        throw "Die Signatur der Windows-App-Runtime ist ungültig: $($depSignature.Status)."
    }
    Copy-Item -LiteralPath $PrinterPackagePath (Join-Path $payload 'Printer.msix') -Force
    Copy-Item -LiteralPath $PrinterDependencyPath (Join-Path $payload 'WindowsAppRuntime.msix') -Force
    if ($SigningMode -eq 'Development') {
        if (-not $PrinterCertificatePath) { $PrinterCertificatePath = Join-Path $workspace 'printer\.cert\ERechnung.Dev.cer' }
        if (-not (Test-Path -LiteralPath $PrinterCertificatePath -PathType Leaf)) {
            throw "Entwicklungszertifikat fehlt: $PrinterCertificatePath"
        }
        Copy-Item -LiteralPath $PrinterCertificatePath (Join-Path $payload 'PrinterCertificate.cer') -Force
        $arguments += '/DDEV_CERTIFICATE'
    } else {
        $signature = Get-AuthenticodeSignature -LiteralPath $PrinterPackagePath
        if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -eq $signature.SignerCertificate.Issuer) {
            throw 'Das Produktionsdruckerpaket benötigt eine öffentlich vertrauenswürdige Signatur.'
        }
    }
    $arguments += '/DWITH_PRINTER'
}
$arguments += (Join-Path $workspace 'installer/windows/druckwandel.nsi')
$compiler = Get-Command makensis.exe -ErrorAction SilentlyContinue
$makensis = if ($compiler) { $compiler.Source } else { Join-Path ${env:ProgramFiles(x86)} 'NSIS\makensis.exe' }
Invoke-Checked $makensis $arguments
if ($SigningMode -eq 'Production') {
    if (-not $env:ERECHNUNG_SIGNTOOL) { throw 'ERECHNUNG_SIGNTOOL fehlt.' }
    Invoke-Checked $env:ERECHNUNG_SIGNTOOL @('sign','/fd','SHA256','/td','SHA256','/tr','https://timestamp.digicert.com',$output)
    $signature = Get-AuthenticodeSignature $output
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -eq $signature.SignerCertificate.Issuer) {
        throw 'Der Installer benötigt eine öffentlich vertrauenswürdige Signatur.'
    }
}
$hash = (Get-FileHash $output -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText("$output.sha256", "$hash *$([IO.Path]::GetFileName($output))`r`n")
Write-Output $output
