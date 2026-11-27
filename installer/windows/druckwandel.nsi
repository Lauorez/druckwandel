Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "Sections.nsh"
!include "x64.nsh"
Name "Druckwandel"
OutFile "${OUTPUT}"
InstallDir "$PROGRAMFILES64\Druckwandel"
InstallDirRegKey HKLM "Software\Druckwandel" "InstallLocation"
RequestExecutionLevel admin
!ifdef UNCOMPRESSED
SetCompress off
!else
SetCompressor /SOLID lzma
!endif
!define MUI_ABORTWARNING
!define MUI_ICON "${PAYLOAD}\icon.ico"
!define MUI_UNICON "${PAYLOAD}\icon.ico"
!insertmacro MUI_PAGE_WELCOME
!define MUI_COMPONENTSPAGE_NODESC
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "Druckwandel jetzt starten"
!define MUI_FINISHPAGE_RUN_FUNCTION LaunchAppAsUser
!define MUI_PAGE_CUSTOMFUNCTION_SHOW FinishPageShow
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "German"
Var PowerShell
Var IsUpdate
Var AutoUpdate
Var WaitPid

Function .onInit
  SetRegView 64
  SetShellVarContext current
  StrCpy $AutoUpdate 0
  StrCpy $WaitPid ""
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/AUTOUPDATE" $1
  ${IfNot} ${Errors}
    StrCpy $AutoUpdate 1
  ${EndIf}
  ClearErrors
  ${GetOptions} $0 "/WAITPID=" $1
  ${IfNot} ${Errors}
    StrCpy $WaitPid $1
  ${EndIf}
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
  Call RestoreDesktopShortcutSelection
FunctionEnd

Section "Druckwandel" SEC_APP
  SectionIn RO
  SetRegView 64
  Call WaitForAppExit
  ; Replace a previous machine-wide installer before writing new files. /UPDATE
  ; retains the installed printer. Never execute an unprivileged HKCU uninstall
  ; string while elevated as administrator.
  ReadRegStr $R7 HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Druckwandel" "UninstallString"
  ReadRegStr $R6 HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Druckwandel" "InstallLocation"
  ${If} $R7 == ""
    ReadRegStr $R5 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Druckwandel" "UninstallString"
    ${If} $R5 != ""
      MessageBox MB_ICONSTOP "Es wurde eine ältere benutzerbezogene Installation gefunden. Bitte entfernen Sie diese zuerst über die Windows-Apps-Einstellungen und starten Sie die Einrichtung erneut." /SD IDOK
      SetErrorLevel 1
      Abort
    ${EndIf}
  ${EndIf}
  ${If} $R6 == ""
    StrCpy $R6 $INSTDIR
  ${EndIf}
  ${If} $R7 != ""
    ExecWait '$R7 /S /UPDATE _?=$R6' $0
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
  SetOutPath "$INSTDIR\resources\validators"
  File /r "${PAYLOAD}\resources\validators\*"
  SetOutPath "$INSTDIR\resources\ocr"
  File /r "${PAYLOAD}\resources\ocr\*"
!ifdef WITH_PRINTER
  SetOutPath "$PLUGINSDIR\printer"
  File "${PAYLOAD}\Printer.msix"
  File "${PAYLOAD}\WindowsAppRuntime.msix"
  File "${PAYLOAD}\InstallPrinter.ps1"
!ifdef DEV_CERTIFICATE
  File "${PAYLOAD}\PrinterCertificate.cer"
  nsExec::ExecToStack /TIMEOUT=300000 '"$PowerShell" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\printer\InstallPrinter.ps1" -PackagePath "$PLUGINSDIR\printer\Printer.msix" -DependencyPath "$PLUGINSDIR\printer\WindowsAppRuntime.msix" -CertificatePath "$PLUGINSDIR\printer\PrinterCertificate.cer"'
!else
  nsExec::ExecToStack /TIMEOUT=300000 '"$PowerShell" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\printer\InstallPrinter.ps1" -PackagePath "$PLUGINSDIR\printer\Printer.msix" -DependencyPath "$PLUGINSDIR\printer\WindowsAppRuntime.msix"'
!endif
  Pop $0
  Pop $1
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Der Drucker konnte nicht eingerichtet werden.$\r$\n$1" /SD IDOK
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
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Druckwandel" "DisplayIcon" "$INSTDIR\Druckwandel.exe,0"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Druckwandel" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Druckwandel" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Druckwandel" "UninstallString" '"$INSTDIR\uninstall.exe"'
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$SMPROGRAMS\Druckwandel.lnk" "$INSTDIR\Druckwandel.exe" "" "$INSTDIR\Druckwandel.exe" 0
SectionEnd

Section /o "Desktopverknüpfung erstellen" SEC_DESKTOP
  ClearErrors
  CreateShortcut "$DESKTOP\Druckwandel.lnk" "$INSTDIR\Druckwandel.exe" "" "$INSTDIR\Druckwandel.exe" 0
  ${IfNot} ${Errors}
    WriteRegDWORD HKCU "Software\Druckwandel" "DesktopShortcut" 1
  ${EndIf}
SectionEnd

Function RestoreDesktopShortcutSelection
  ReadRegDWORD $0 HKCU "Software\Druckwandel" "DesktopShortcut"
  ${If} $0 == 1
  ${AndIf} ${FileExists} "$DESKTOP\Druckwandel.lnk"
    !insertmacro SelectSection ${SEC_DESKTOP}
  ${EndIf}
FunctionEnd

Function WaitForAppExit
  ${If} $WaitPid != ""
    IntCmp $WaitPid 0 wait_done wait_done
    ; OpenProcess(SYNCHRONIZE = 0x00100000, bInheritHandle = 0, dwProcessId = $WaitPid)
    System::Call 'kernel32::OpenProcess(i 0x00100000, i 0, i $WaitPid) p .r0'
    ${If} $0 != 0
      DetailPrint "Warte auf das Beenden von Druckwandel (PID $WaitPid) ..."
      System::Call 'kernel32::WaitForSingleObject(p r0, i 15000) i .r1'
      System::Call 'kernel32::CloseHandle(p r0)'
      Sleep 300
    ${EndIf}
  ${EndIf}
  wait_done:
FunctionEnd

Function LaunchAppAsUser
  ; Relaunch through explorer.exe so Druckwandel runs in the normal (non-elevated) user context.
  ${If} ${FileExists} "$INSTDIR\Druckwandel.exe"
    Exec '"$WINDIR\explorer.exe" "$INSTDIR\Druckwandel.exe"'
  ${EndIf}
FunctionEnd

Function FinishPageShow
  ; Keep normal manual installation identical; only show & pre-select restart during /AUTOUPDATE.
  ${If} $AutoUpdate != 1
    ${NSD_Uncheck} $mui.FinishPage.Run
    ShowWindow $mui.FinishPage.Run 0 ; SW_HIDE
  ${EndIf}
FunctionEnd

Function .onInstSuccess
  ; In silent /AUTOUPDATE runs the finish page is skipped, so relaunch here.
  ${If} ${Silent}
  ${AndIf} $AutoUpdate == 1
    Call LaunchAppAsUser
  ${EndIf}
FunctionEnd

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
    nsExec::ExecToStack /TIMEOUT=120000 `"$PowerShell" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$ErrorActionPreference='Stop';Import-Module (Join-Path $$PSHOME 'Modules\Appx\Appx.psd1') -Force;Import-Module (Join-Path $$PSHOME 'Modules\PrintManagement\PrintManagement.psd1') -Force;if(@(Get-PrintJob -PrinterName 'E-Rechnung' -EA SilentlyContinue).Count -gt 0){throw 'Aktiver Druckauftrag läuft.'};foreach($$n in 'Druckwandel.VirtualPrinter','ERechnung.VirtualPrinter.PoC'){Get-AppxPackage -Name $$n|Remove-AppxPackage}"`
    Pop $0
    Pop $1
    ${If} $0 != 0
      MessageBox MB_ICONSTOP "Der Drucker konnte nicht entfernt werden.$\r$\n$1" /SD IDOK
      SetErrorLevel 1
      Abort
    ${EndIf}
    nsExec::ExecToStack /TIMEOUT=60000 `"$PowerShell" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$s=[Security.Cryptography.X509Certificates.X509Store]::new('TrustedPeople','LocalMachine');$$s.Open('ReadWrite');try{$$s.Certificates|Where-Object{$$_.Subject -eq 'CN=ERechnung Development' -and $$_.Issuer -eq 'CN=ERechnung Development'}|ForEach-Object{$$s.Remove($$_)}}finally{$$s.Close()}"`
    Pop $0
    Pop $1
  ${EndIf}
!endif
  ReadRegStr $0 HKCU "Software\Classes\erechnung-review\shell\open\command" ""
  ${If} $0 == '"$INSTDIR\Druckwandel.exe" "%1"'
    DeleteRegKey HKCU "Software\Classes\erechnung-review"
  ${EndIf}
  DeleteRegKey HKLM "Software\Druckwandel"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Druckwandel"
  Delete "$SMPROGRAMS\Druckwandel.lnk"
  ReadRegDWORD $0 HKCU "Software\Druckwandel" "DesktopShortcut"
  ${If} $0 == 1
    Delete "$DESKTOP\Druckwandel.lnk"
    DeleteRegValue HKCU "Software\Druckwandel" "DesktopShortcut"
    DeleteRegKey /ifempty HKCU "Software\Druckwandel"
  ${EndIf}
  Delete "$INSTDIR\Druckwandel.exe"
  Delete "$INSTDIR\icon.ico"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\THIRD-PARTY-NOTICES.md"
  Delete "$INSTDIR\AssertSessionAccount.ps1"
  RMDir /r "$INSTDIR\resources\validators"
  RMDir /r "$INSTDIR\resources\ocr"
  RMDir "$INSTDIR\resources"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
SectionEnd
