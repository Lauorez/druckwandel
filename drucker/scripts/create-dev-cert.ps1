[CmdletBinding()]
param(
    [string]$Password = "ERechnung-Dev-Only"
)

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path $PSScriptRoot -Parent
$certificateDirectory = Join-Path $repositoryRoot ".cert"
$pfxPath = Join-Path $certificateDirectory "ERechnung.Dev.pfx"
$cerPath = Join-Path $certificateDirectory "ERechnung.Dev.cer"

New-Item -ItemType Directory -Force -Path $certificateDirectory | Out-Null

if ((Test-Path $pfxPath) -and (Test-Path $cerPath)) {
    Write-Host "Development certificate already exists: $pfxPath"
    return
}

$certificate = New-SelfSignedCertificate `
    -Type Custom `
    -Subject "CN=ERechnung Development" `
    -FriendlyName "E-Rechnung Virtual Printer Development" `
    -CertStoreLocation "Cert:\CurrentUser\My" `
    -KeyAlgorithm RSA `
    -KeyLength 2048 `
    -HashAlgorithm SHA256 `
    -KeyUsage DigitalSignature `
    -TextExtension @(
        "2.5.29.37={text}1.3.6.1.5.5.7.3.3",
        "2.5.29.19={text}"
    )

$securePassword = ConvertTo-SecureString -String $Password -Force -AsPlainText
Export-PfxCertificate -Cert $certificate -FilePath $pfxPath -Password $securePassword | Out-Null
Export-Certificate -Cert $certificate -FilePath $cerPath | Out-Null

Write-Host "Created development certificate: $pfxPath"
