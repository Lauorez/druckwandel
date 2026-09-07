[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path $PSScriptRoot -Parent
$cerPath = Join-Path $repositoryRoot ".cert\ERechnung.Dev.cer"

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
$isAdministrator = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdministrator) {
    throw "Das Development-Zertifikat muss in LocalMachine\TrustedPeople importiert werden. Starte PowerShell als Administrator."
}

if (-not (Test-Path $cerPath)) {
    & (Join-Path $PSScriptRoot "create-dev-cert.ps1")
}

Import-Certificate -FilePath $cerPath -CertStoreLocation "Cert:\LocalMachine\TrustedPeople" | Out-Null
Write-Host "Development certificate trusted in LocalMachine\TrustedPeople."
