Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "x64.nsh"
Name "Druckwandel"
OutFile "${OUTPUT}"
InstallDir "$PROGRAMFILES64\Druckwandel"
InstallDirRegKey HKLM "Software\Druckwandel" "InstallLocation"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "German"
Var PowerShell
Var IsUpdate

Function .onInit
  SetRegView 64
  SetShellVarContext current
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "Druckwandel benötigt Windows x64."
    Abort
  ${EndIf}
  StrCpy $PowerShell "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  File "${PAYLOAD}\AssertSessionAccount.ps1"
  nsExec::ExecToStack /TIMEOUT=60000 '"$PowerShell" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\AssertSessionAccount.ps1"'
  Pop $0
  Pop $1
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "$1" /SD IDOK
    SetErrorLevel 1
    Abort
  ${EndIf}
!ifdef WITH_PRINTER
  ReadRegStr $0 HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion" "CurrentBuildNumber"
  ${If} $0 < 26100
    MessageBox MB_ICONSTOP "Der E-Rechnungsdrucker benötigt Windows 11 24H2 (Build 26100) oder neuer."
    Abort
  ${EndIf}
!endif
FunctionEnd

Section "Druckwandel" SEC_APP
  SetRegView 64
  ; Replace a previous installer before writing new files. /UPDATE retains the
  ; installed printer. The immutable legacy branch contains the old implementation.
  ReadRegStr $R7 HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Druckwandel" "UninstallString"
  ${If} $R7 == ""
    ReadRegStr $R7 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Druckwandel" "UninstallString"
  ${EndIf}
  ${If} $R7 != ""
    ExecWait '$R7 /S /UPDATE _?=$INSTDIR' $0
    ${If} $0 != 0
      MessageBox MB_ICONSTOP "Die bisherige Installation konnte nicht entfernt werden. Bitte Druckwandel schließen und erneut versuchen." /SD IDOK
      SetErrorLevel 1
      Abort
    ${EndIf}
  ${EndIf}
  SetOutPath "$INSTDIR"
  File "${PAYLOAD}\Druckwandel.exe"
  File "${PAYLOAD}\icon.ico"
  File "${PAYLOAD}\LICENSE"
  File "${PAYLOAD}\THIRD-PARTY-NOTICES.md"
  File "${PAYLOAD}\AssertSessionAccount.ps1"
  File "${PAYLOAD}\RemovePrinter.ps1"
  SetOutPath "$INSTDIR\resources\validators"
  File /r "${PAYLOAD}\resources\validators\*"
!ifdef WITH_PRINTER
  SetOutPath "$PLUGINSDIR\printer"
  File "${PAYLOAD}\InstallPrinter.ps1"
  File "${PAYLOAD}\Printer.msix"
  File "${PAYLOAD}\WindowsAppRuntime.msix"
!ifdef DEV_CERTIFICATE
  File "${PAYLOAD}\PrinterCertificate.cer"
  nsExec::ExecToStack /TIMEOUT=300000 '"$PowerShell" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\printer\InstallPrinter.ps1" -PackagePath "$PLUGINSDIR\printer\Printer.msix" -DependencyPath "$PLUGINSDIR\printer\WindowsAppRuntime.msix" -CertificatePath "$PLUGINSDIR\printer\PrinterCertificate.cer"'
!else
  nsExec::ExecToStack /TIMEOUT=300000 '"$PowerShell" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\printer\InstallPrinter.ps1" -PackagePath "$PLUGINSDIR\printer\Printer.msix" -DependencyPath "$PLUGINSDIR\printer\WindowsAppRuntime.msix"'
!endif
  Pop $0
  Pop $1
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Der Drucker konnte nicht eingerichtet werden. Details: $TEMP\Druckwandel-Installation.log$\r$\n$1" /SD IDOK
    SetErrorLevel 1
    Abort
  ${EndIf}
!endif
  ; Same URI scheme and UUID format as the installed print companion.
  WriteRegStr HKCU "Software\Classes\erechnung-review" "" "URL:Druckwandel Druckauftrag"
  WriteRegStr HKCU "Software\Classes\erechnung-review" "URL Protocol" ""
  WriteRegStr HKCU "Software\Classes\erechnung-review\DefaultIcon" "" '"$INSTDIR\Druckwandel.exe",0'
  WriteRegStr HKCU "Software\Classes\erechnung-review\shell\open\command" "" '"$INSTDIR\Druckwandel.exe" "%1"'
  WriteRegStr HKLM "Software\Druckwandel" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Druckwandel" "DisplayName" "Druckwandel"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Druckwandel" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Druckwandel" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Druckwandel" "UninstallString" '"$INSTDIR\uninstall.exe"'
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$SMPROGRAMS\Druckwandel.lnk" "$INSTDIR\Druckwandel.exe"
SectionEnd

Function un.onInit
  SetRegView 64
  SetShellVarContext current
  StrCpy $PowerShell "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/UPDATE" $1
  StrCpy $IsUpdate 0
  ${IfNot} ${Errors}
    StrCpy $IsUpdate 1
  ${EndIf}
  nsExec::ExecToStack /TIMEOUT=60000 '"$PowerShell" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\AssertSessionAccount.ps1"'
  Pop $0
  Pop $1
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "$1" /SD IDOK
    SetErrorLevel 1
    Abort
  ${EndIf}
FunctionEnd

Section "Uninstall"
!ifdef WITH_PRINTER
  ${If} $IsUpdate != 1
    nsExec::ExecToStack /TIMEOUT=120000 '"$PowerShell" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\RemovePrinter.ps1"'
    Pop $0
    Pop $1
    ${If} $0 != 0
      MessageBox MB_ICONSTOP "Der Drucker konnte nicht entfernt werden.$\r$\n$1" /SD IDOK
      SetErrorLevel 1
      Abort
    ${EndIf}
  ${EndIf}
!endif
  ReadRegStr $0 HKCU "Software\Classes\erechnung-review\shell\open\command" ""
  ${If} $0 == '"$INSTDIR\Druckwandel.exe" "%1"'
    DeleteRegKey HKCU "Software\Classes\erechnung-review"
  ${EndIf}
  DeleteRegKey HKLM "Software\Druckwandel"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Druckwandel"
  Delete "$SMPROGRAMS\Druckwandel.lnk"
  Delete "$INSTDIR\Druckwandel.exe"
  Delete "$INSTDIR\icon.ico"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\THIRD-PARTY-NOTICES.md"
  Delete "$INSTDIR\AssertSessionAccount.ps1"
  Delete "$INSTDIR\RemovePrinter.ps1"
  RMDir /r "$INSTDIR\resources\validators"
  RMDir "$INSTDIR\resources"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
SectionEnd
