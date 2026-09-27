[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$PackagePath,
    [Parameter(Mandatory = $true)][string]$CertificatePath
)

$ErrorActionPreference = "Stop"
Import-Module (Join-Path $PSHOME "Modules\Microsoft.PowerShell.Security\Microsoft.PowerShell.Security.psd1") -Force

$principal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "Zum Freigeben des Druckerzertifikats sind Administratorrechte erforderlich."
}

$certificate = [Security.Cryptography.X509Certificates.X509Certificate2]::new($CertificatePath)
$signature = Get-AuthenticodeSignature -LiteralPath $PackagePath
if ($signature.Status -eq "HashMismatch" -or -not $signature.SignerCertificate) {
    throw "Die Signatur des Druckerpakets ist ungültig."
}
if ($certificate.Thumbprint -ne $signature.SignerCertificate.Thumbprint -or
    $certificate.Subject -ne "CN=ERechnung Development" -or
    $certificate.HasPrivateKey -or
    $certificate.NotAfter -le (Get-Date)) {
    throw "Das mitgelieferte Entwicklungszertifikat passt nicht zum Druckerpaket."
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [IO.Compression.ZipFile]::OpenRead($PackagePath)
try {
    $entry = $archive.GetEntry("AppxManifest.xml")
    if (-not $entry) { throw "Das Druckerpaket enthält kein AppxManifest.xml." }
    $reader = [IO.StreamReader]::new($entry.Open())
    try { [xml]$manifest = $reader.ReadToEnd() } finally { $reader.Dispose() }
    if ($manifest.Package.Identity.Name -ne "ERechnung.VirtualPrinter.PoC" -or
        $manifest.Package.Identity.Publisher -ne $certificate.Subject) {
        throw "Die Paketidentität passt nicht zum Entwicklungszertifikat."
    }
} finally {
    $archive.Dispose()
}

Import-Certificate -FilePath $CertificatePath -CertStoreLocation "Cert:\LocalMachine\TrustedPeople" | Out-Null
if (-not (Test-Path -LiteralPath ("Cert:\LocalMachine\TrustedPeople\" + $certificate.Thumbprint))) {
    throw "Windows hat das Druckerzertifikat nicht im Computerspeicher hinterlegt."
}
