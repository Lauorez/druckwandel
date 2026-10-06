[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path $PSScriptRoot -Parent
$cerPath = Join-Path $repositoryRoot ".cert\ERechnung.Dev.cer"

if (-not (Test-Path $cerPath)) {
    & (Join-Path $PSScriptRoot "create-dev-cert.ps1")
}

Import-Certificate -FilePath $cerPath -CertStoreLocation "Cert:\CurrentUser\TrustedPeople" | Out-Null
Import-Certificate -FilePath $cerPath -CertStoreLocation "Cert:\CurrentUser\Root" | Out-Null

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
$isAdministrator = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if ($isAdministrator) {
    Import-Certificate -FilePath $cerPath -CertStoreLocation "Cert:\LocalMachine\TrustedPeople" | Out-Null
    Write-Host "Development certificate trusted in CurrentUser and LocalMachine\TrustedPeople."
} else {
    Write-Host "Development certificate trusted in CurrentUser\TrustedPeople (ohne Administrator)."
}
