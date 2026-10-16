[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
# NSIS can be started with a reduced PSModulePath.
Import-Module (Join-Path $PSHOME "Modules\CimCmdlets\CimCmdlets.psd1") -Force

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Output "Die Einrichtung benötigt Administratorrechte."
    exit 1
}

# Printer package, application data and update snapshot belong to the account that runs
# the installer. If an administrator confirmed UAC with a different account, they would
# end up with that account instead of the person signed in.
$sessionId = (Get-Process -Id $PID).SessionId
$shell = Get-CimInstance -ClassName Win32_Process -Filter "Name = 'explorer.exe' AND SessionId = $sessionId" |
    Select-Object -First 1
if (-not $shell) {
    exit 0
}
$owner = Invoke-CimMethod -InputObject $shell -MethodName GetOwnerSid
if ($owner.Sid -and $owner.Sid -ne $identity.User.Value) {
    Write-Output "Die Administratorfreigabe stammt vom Konto $($identity.Name), angemeldet ist aber ein anderes Konto. Drucker und Daten würden sonst beim falschen Konto eingerichtet. Bitte melden Sie sich mit dem Konto an, das Druckwandel nutzen soll und Administratorrechte hat, und starten Sie die Einrichtung erneut."
    exit 1
}
exit 0
