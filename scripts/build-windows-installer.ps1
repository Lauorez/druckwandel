[CmdletBinding()]
param(
    [switch]$SkipChecks,
    [switch]$AppOnly,
    [switch]$SkipBuild,
    [ValidateSet('Development','Production')][string]$SigningMode = 'Development',
    [string]$PrinterPackagePath = '',
    [string]$PrinterDependencyPath = '',
    [string]$PrinterCertificatePath = ''
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
Set-Location $workspace
function Invoke-Checked([string]$Command, [string[]]$Arguments) {
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Command failed ($LASTEXITCODE)" }
}
if ($SigningMode -eq 'Production' -and ($SkipChecks -or $AppOnly)) {throw 'Produktionsbuilds benötigen alle Prüfungen und das Druckerpaket.'}
$metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) {throw 'Cargo-Metadaten fehlen.'}
$version = ($metadata.packages | Where-Object name -eq 'druckwandel-native').version
$payload = Join-Path $workspace 'installer\windows\payload'
New-Item -ItemType Directory -Force $payload | Out-Null
if (-not $SkipChecks) {
    Invoke-Checked 'cargo' @('fmt','--all','--','--check')
    Invoke-Checked 'cargo' @('clippy','--all-targets','--locked','--','-D','warnings')
    Invoke-Checked 'cargo' @('test','--locked')
    Invoke-Checked 'cargo' @('test','--locked','--test','external_validators','--','--ignored')
}
if (-not $SkipBuild) {Invoke-Checked 'cargo' @('build','--release','--bin','druckwandel-native','--locked')}
Copy-Item (Join-Path $metadata.target_directory 'release\druckwandel-native.exe') (Join-Path $payload 'Druckwandel.exe') -Force
$validatorRoot = Join-Path $workspace 'resources\validators'
$manifest = Get-Content (Join-Path $validatorRoot 'manifest.json') -Raw | ConvertFrom-Json
foreach ($file in @('jre/bin/java.exe',$manifest.kosit.jar,$manifest.kosit.scenarios,$manifest.mustang.jar,$manifest.verapdf.jar)) {
    if (-not (Test-Path (Join-Path $validatorRoot $file) -PathType Leaf)) {throw "Prüfpaket fehlt: $file. Zuerst python scripts/fetch-validators.py auf Windows ausführen."}
}
$resourceTarget = Join-Path $payload 'resources'
New-Item -ItemType Directory -Force $resourceTarget | Out-Null
if (Test-Path (Join-Path $resourceTarget 'validators')) {Remove-Item (Join-Path $resourceTarget 'validators') -Recurse -Force}
Copy-Item $validatorRoot $resourceTarget -Recurse -Force
Copy-Item 'apps/native/assets/icon.ico' $payload -Force
foreach ($file in @('LICENSE','THIRD-PARTY-NOTICES.md')) {Copy-Item $file $payload -Force}
foreach ($file in @('InstallPrinter.ps1','RemovePrinter.ps1','AssertSessionAccount.ps1')) {
    $content = [IO.File]::ReadAllText((Join-Path $workspace "installer/windows/$file"))
    [IO.File]::WriteAllText((Join-Path $payload $file),$content,[Text.UTF8Encoding]::new($true))
}
$artifactRoot = Join-Path $workspace 'artifacts\windows'
New-Item -ItemType Directory -Force $artifactRoot | Out-Null
$suffix = if ($AppOnly) {'-AppOnly'} else {''}
$output = Join-Path $artifactRoot "Druckwandel-$version-x64$suffix-Setup.exe"
$arguments = @("/DVERSION=$version","/DPAYLOAD=$payload","/DOUTPUT=$output")
if (-not $AppOnly) {
    if (-not $PrinterPackagePath) {
        Invoke-Checked 'powershell.exe' @('-NoProfile','-ExecutionPolicy','Bypass','-File','scripts/printer-build.ps1')
        $PrinterPackagePath = (Get-ChildItem printer/artifacts/packages -Recurse -Filter 'CompanionApp_*.msix' | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1).FullName
    }
    if (-not $PrinterPackagePath -or -not $PrinterDependencyPath) {throw 'Druckerpaket und Windows-App-Runtime müssen vorhanden sein (-PrinterPackagePath und -PrinterDependencyPath).'}
    Copy-Item $PrinterPackagePath (Join-Path $payload 'Printer.msix') -Force
    Copy-Item $PrinterDependencyPath (Join-Path $payload 'WindowsAppRuntime.msix') -Force
    $validation = @('-NoProfile','-ExecutionPolicy','Bypass','-File', (Join-Path $payload 'InstallPrinter.ps1'),'-PackagePath',(Join-Path $payload 'Printer.msix'),'-DependencyPath',(Join-Path $payload 'WindowsAppRuntime.msix'),'-ValidateOnly')
    if ($SigningMode -eq 'Development') {
        if (-not $PrinterCertificatePath) {$PrinterCertificatePath = Join-Path $workspace 'printer/.cert/ERechnung.Dev.cer'}
        Copy-Item $PrinterCertificatePath (Join-Path $payload 'PrinterCertificate.cer') -Force
        $arguments += '/DDEV_CERTIFICATE'
        $validation += @('-CertificatePath',(Join-Path $payload 'PrinterCertificate.cer'))
    } else {
        $signature = Get-AuthenticodeSignature $PrinterPackagePath
        if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -eq $signature.SignerCertificate.Issuer) {throw 'Das Produktionsdruckerpaket benötigt eine öffentlich vertrauenswürdige Signatur.'}
    }
    Invoke-Checked 'powershell.exe' $validation
    $arguments += '/DWITH_PRINTER'
}
$arguments += (Join-Path $workspace 'installer/windows/druckwandel.nsi')
$compiler = Get-Command makensis.exe -ErrorAction SilentlyContinue
$makensis = if ($compiler) {$compiler.Source} else {Join-Path ${env:ProgramFiles(x86)} 'NSIS\makensis.exe'}
Invoke-Checked $makensis $arguments
if ($SigningMode -eq 'Production') {
    if (-not $env:ERECHNUNG_SIGNTOOL) {throw 'ERECHNUNG_SIGNTOOL fehlt.'}
    Invoke-Checked $env:ERECHNUNG_SIGNTOOL @('sign','/fd','SHA256','/td','SHA256','/tr','http://timestamp.digicert.com',$output)
    $signature = Get-AuthenticodeSignature $output
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -eq $signature.SignerCertificate.Issuer) {throw 'Der Installer benötigt eine öffentlich vertrauenswürdige Signatur.'}
}
$hash = (Get-FileHash $output -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText("$output.sha256", "$hash *$([IO.Path]::GetFileName($output))`r`n")
Write-Output $output
