; Pulsar's Windows installer (NSIS), built by packaging/package.sh:
;
;   makensis -DVERSION=1.2.3 -DBINARY=pulsar.exe -DICON=icon.ico -DOUTFILE=setup.exe installer.nsi
;
; A per-user install to %LOCALAPPDATA%\Pulsar with the same files and
; registry keys as the installer of the Tauri versions, so each replaces the
; other in place. Flags (the Tauri updater runs "setup.exe /P /UPDATE"):
;
;   /P       passive: progress only, no questions
;   /S       silent
;   /UPDATE  updating a running install: start Pulsar again afterwards
;   /R       start Pulsar afterwards

Unicode true
ManifestDPIAware true
SetCompressor /SOLID lzma
RequestExecutionLevel user

!include MUI2.nsh
!include FileFunc.nsh
!include LogicLib.nsh

!define PRODUCT "Pulsar"
!define EXE "pulsar.exe"
!define PUBLISHER "petersen"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT}"
!define APP_KEY "Software\${PUBLISHER}\${PRODUCT}"
!define RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"

Name "${PRODUCT}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\${PRODUCT}"
BrandingText "${PRODUCT} ${VERSION}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${PRODUCT}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "${PRODUCT} installer"
VIAddVersionKey "CompanyName" "${PUBLISHER}"
VIAddVersionKey "LegalCopyright" "MIT License"

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"

Var Passive
Var StartAfter

!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipWhenPassive
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipWhenPassive
!define MUI_FINISHPAGE_RUN "$INSTDIR\${EXE}"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; The first is the default; the system language picks among them.
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "PortugueseBR"

!macro HasFlag FLAG VAR
  ClearErrors
  ${GetOptions} $0 "${FLAG}" $1
  ${IfNot} ${Errors}
    StrCpy ${VAR} 1
  ${EndIf}
!macroend

Function .onInit
  ${GetParameters} $0
  !insertmacro HasFlag "/P" $Passive
  !insertmacro HasFlag "/UPDATE" $StartAfter
  !insertmacro HasFlag "/R" $StartAfter
  ${If} $Passive == 1
    SetAutoClose true
  ${EndIf}
FunctionEnd

Function SkipWhenPassive
  ${If} $Passive == 1
    Abort
  ${EndIf}
FunctionEnd

; Waits for Pulsar to quit (an update starts this right before quitting),
; then closes it if it still runs.
Function CloseApp
  StrCpy $2 0
  ${DoWhile} ${FileExists} "$INSTDIR\${EXE}"
    ClearErrors
    Delete "$INSTDIR\${EXE}"
    ${IfNot} ${Errors}
      ${Break}
    ${EndIf}
    IntOp $2 $2 + 1
    ${If} $2 = 15
      nsExec::Exec 'taskkill /F /IM "${EXE}"'
      Pop $3
    ${EndIf}
    ${If} $2 > 40
      ${Break}
    ${EndIf}
    Sleep 200
  ${Loop}
FunctionEnd

Section "Install"
  SetOutPath "$INSTDIR"
  Call CloseApp
  File "/oname=${EXE}" "${BINARY}"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$SMPROGRAMS\${PRODUCT}.lnk" "$INSTDIR\${EXE}"

  WriteRegStr HKCU "${APP_KEY}" "" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$\"$INSTDIR\${EXE}$\""
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$\"$INSTDIR$\""
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegStr HKCU "${UNINSTALL_KEY}" "MainBinaryName" "${EXE}"
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" $0

  ; Left by the Tauri versions (their WebView's data); unused since 1.0.
  RMDir /r "$LOCALAPPDATA\com.petersen.pulsar\EBWebView"
SectionEnd

Function .onInstSuccess
  ${If} $StartAfter == 1
    Exec '"$INSTDIR\${EXE}"'
  ${EndIf}
FunctionEnd

Section "Uninstall"
  nsExec::Exec 'taskkill /F /IM "${EXE}"'
  Pop $0
  Sleep 500
  Delete "$INSTDIR\${EXE}"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${PRODUCT}.lnk"
  DeleteRegValue HKCU "${RUN_KEY}" "${PRODUCT}"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  DeleteRegKey HKCU "${APP_KEY}"
  DeleteRegKey /ifempty HKCU "Software\${PUBLISHER}"
SectionEnd
