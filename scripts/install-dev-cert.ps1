[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path $PSScriptRoot -Parent
$cerPath = Join-Path $repositoryRoot ".cert\ERechnung.Dev.cer"

if (-not (Test-Path $cerPath)) {
    & (Join-Path $PSScriptRoot "create-dev-cert.ps1")
}

Import-Certificate -FilePath $cerPath -CertStoreLocation "Cert:\CurrentUser\TrustedPeople" | Out-Null
Write-Host "Development certificate trusted for the current user."
