!include LogicLib.nsh

; NSIS resolves __FILEDIR__ to this hook's source directory. The generated
; payload therefore stays machine-independent and next to the Tauri project.
!define ERECHNUNG_INSTALLER_PAYLOAD "${__FILEDIR__}\..\installer\windows\payload"

!macro NSIS_HOOK_PREINSTALL
  ${If} ${RunningX64}
    StrCpy $R8 "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
  ${Else}
    StrCpy $R8 "$SYSDIR\WindowsPowerShell\v1.0\powershell.exe"
  ${EndIf}

  ReadRegStr $0 HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion" "CurrentBuildNumber"
  ${If} $0 == ""
    MessageBox MB_ICONSTOP|MB_OK "Die Windows-Version konnte nicht geprüft werden. Die Einrichtung wird beendet."
    Abort
  ${EndIf}
  ${If} $0 < 26100
    MessageBox MB_ICONSTOP|MB_OK "Der E-Rechnungsdrucker benötigt Windows 11 mit allen aktuellen Updates (Version 24H2 oder neuer). Bitte aktualisieren Sie Windows und starten Sie die Einrichtung danach erneut."
    Abort
  ${EndIf}

  InitPluginsDir
  CreateDirectory "$PLUGINSDIR\ERechnungsAssistent"
  SetOutPath "$PLUGINSDIR\ERechnungsAssistent"
  File /oname=InstallPrinter.ps1 "${ERECHNUNG_INSTALLER_PAYLOAD}\InstallPrinter.ps1"
  File /oname=TrustPrinterCertificate.ps1 "${ERECHNUNG_INSTALLER_PAYLOAD}\TrustPrinterCertificate.ps1"
  File /oname=UpdateGuard.ps1 "${ERECHNUNG_INSTALLER_PAYLOAD}\UpdateGuard.ps1"
  File /oname=Printer.msix "${ERECHNUNG_INSTALLER_PAYLOAD}\Printer.msix"
  File /oname=WindowsAppRuntime.msix "${ERECHNUNG_INSTALLER_PAYLOAD}\WindowsAppRuntime.msix"

  ; Bis 0.3.4 hieß die Anwendung "E-Rechnungs-Assistent". Tauri leitet Installationsordner
  ; und Deinstallationsschlüssel aus dem Produktnamen ab, deshalb wird die alte
  ; Installation hier wie bei einem Update ersetzt. Daten, Archiv und Drucker bleiben.
  ReadRegStr $R7 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\E-Rechnungs-Assistent" "UninstallString"

  ${If} $UpdateMode = 1
  ${OrIf} $R7 != ""
    DetailPrint "Sicherungsstand vor der Aktualisierung ..."
    nsExec::ExecToStack /TIMEOUT=120000 '"$R8" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\ERechnungsAssistent\UpdateGuard.ps1" -Action PrepareUpdate -IncomingVersion "${VERSION}"'
    Pop $0
    Pop $1
    ${If} $0 != 0
      DetailPrint "$1"
      MessageBox MB_ICONSTOP|MB_OK "Die Aktualisierung wurde abgebrochen, bevor Dateien ersetzt wurden. Der bisherige Datenbestand bleibt unverändert.$\r$\n$\r$\n$1" /SD IDOK
      SetErrorLevel 1
      Abort
    ${EndIf}
  ${EndIf}

  ${If} $R7 != ""
    ReadRegStr $R6 HKCU "Software\${MANUFACTURER}\E-Rechnungs-Assistent" ""
    ${If} $R6 == ""
      StrCpy $R6 "$LOCALAPPDATA\E-Rechnungs-Assistent"
    ${EndIf}
    DetailPrint "Die bisherige Installation E-Rechnungs-Assistent wird ersetzt ..."
    ClearErrors
    ; /UPDATE lässt den Drucker stehen, _?= hält den Deinstaller im alten Ordner,
    ; damit ExecWait auf ihn warten kann.
    ExecWait '$R7 /S /UPDATE _?=$R6' $0
    ${If} ${Errors}
    ${OrIf} $0 <> 0
      MessageBox MB_ICONSTOP|MB_OK "Die bisherige Installation E-Rechnungs-Assistent konnte nicht ersetzt werden. Bitte schließen Sie die Anwendung und starten Sie die Einrichtung erneut. Ihre Daten bleiben unverändert." /SD IDOK
      SetErrorLevel 1
      Abort
    ${EndIf}
    Delete "$R6\uninstall.exe"
    RMDir "$R6"
    Delete "$SMPROGRAMS\E-Rechnungs-Assistent.lnk"
    Delete "$DESKTOP\E-Rechnungs-Assistent.lnk"
    DeleteRegKey HKCU "Software\${MANUFACTURER}\E-Rechnungs-Assistent"
  ${EndIf}

  !if /FileExists "${ERECHNUNG_INSTALLER_PAYLOAD}\PrinterCertificate.cer"
    File /oname=PrinterCertificate.cer "${ERECHNUNG_INSTALLER_PAYLOAD}\PrinterCertificate.cer"
    DetailPrint "Der E-Rechnungsdrucker wird eingerichtet ..."
    nsExec::ExecToStack /TIMEOUT=300000 '"$R8" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\ERechnungsAssistent\InstallPrinter.ps1" -PackagePath "$PLUGINSDIR\ERechnungsAssistent\Printer.msix" -DependencyPath "$PLUGINSDIR\ERechnungsAssistent\WindowsAppRuntime.msix" -CertificatePath "$PLUGINSDIR\ERechnungsAssistent\PrinterCertificate.cer" -CertificateTrustScriptPath "$PLUGINSDIR\ERechnungsAssistent\TrustPrinterCertificate.ps1"'
  !else
    DetailPrint "Der E-Rechnungsdrucker wird eingerichtet ..."
    nsExec::ExecToStack /TIMEOUT=300000 '"$R8" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\ERechnungsAssistent\InstallPrinter.ps1" -PackagePath "$PLUGINSDIR\ERechnungsAssistent\Printer.msix" -DependencyPath "$PLUGINSDIR\ERechnungsAssistent\WindowsAppRuntime.msix"'
  !endif

  Pop $0
  Pop $1
  ${If} $0 != 0
    DetailPrint "$1"
    MessageBox MB_ICONSTOP|MB_OK "Der E-Rechnungsdrucker konnte nicht eingerichtet werden. Die vollständige Installation wird abgebrochen.$\r$\n$\r$\nWeitere Informationen stehen in:$\r$\n$TEMP\Druckwandel-Installation.log" /SD IDOK
    SetErrorLevel 1
    Abort
  ${Else}
    DetailPrint "Der E-Rechnungsdrucker ist bereit."
  ${EndIf}
  ; File changes NSIS' output directory. Restore Tauri's application directory
  ; before the generated installer copies its executable and resources.
  SetOutPath $INSTDIR
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ${If} ${RunningX64}
    StrCpy $R8 "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
  ${Else}
    StrCpy $R8 "$SYSDIR\WindowsPowerShell\v1.0\powershell.exe"
  ${EndIf}
  InitPluginsDir
  CreateDirectory "$PLUGINSDIR\ERechnungsAssistent"
  SetOutPath "$PLUGINSDIR\ERechnungsAssistent"
  File /oname=UpdateGuard.ps1 "${ERECHNUNG_INSTALLER_PAYLOAD}\UpdateGuard.ps1"
  DetailPrint "Installierte Version wird festgehalten ..."
  nsExec::ExecToStack /TIMEOUT=60000 '"$R8" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\ERechnungsAssistent\UpdateGuard.ps1" -Action RecordInstalledVersion -IncomingVersion "${VERSION}"'
  Pop $0
  Pop $1
  ${If} $0 != 0
    DetailPrint "$1"
  ${EndIf}
  SetOutPath $INSTDIR
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Tauri invokes the old uninstaller with /UPDATE during an application update.
  ; The printer must survive that internal uninstall/install hand-over.
  ${If} $UpdateMode <> 1
    ${If} ${RunningX64}
      StrCpy $R8 "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
    ${Else}
      StrCpy $R8 "$SYSDIR\WindowsPowerShell\v1.0\powershell.exe"
    ${EndIf}
    InitPluginsDir
    CreateDirectory "$PLUGINSDIR\ERechnungsAssistent"
    SetOutPath "$PLUGINSDIR\ERechnungsAssistent"
    File /oname=RemovePrinter.ps1 "${ERECHNUNG_INSTALLER_PAYLOAD}\RemovePrinter.ps1"
    DetailPrint "Der E-Rechnungsdrucker wird entfernt ..."
    nsExec::ExecToStack /TIMEOUT=120000 '"$R8" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\ERechnungsAssistent\RemovePrinter.ps1"'
    Pop $0
    Pop $1
    ${If} $0 != 0
      DetailPrint "$1"
      MessageBox MB_ICONEXCLAMATION|MB_OK "Der E-Rechnungsdrucker konnte nicht vollständig entfernt werden. Die übrige Anwendung wird trotzdem deinstalliert.$\r$\n$\r$\nWeitere Informationen stehen in:$\r$\n$TEMP\Druckwandel-Deinstallation.log"
    ${EndIf}
    SetOutPath $INSTDIR
  ${EndIf}
!macroend
