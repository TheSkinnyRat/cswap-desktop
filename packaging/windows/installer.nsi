; Claude Swap v1 installer (NSIS 3, Unicode, per-user). Built by build.ps1:
;   makensis /DVERSION=1.0.0 /DEXE=<claude-swap.exe> /DOUTFILE=<setup.exe> installer.nsi
;
; It installs where 0.1.x (electron-builder, per-user) lived, under the same names, so
; the Start-menu shortcut, the HKCU Run value ("electron.app.Claude Swap") and anything
; else pointing at the exe keep working without being touched:
;   %LOCALAPPDATA%\Programs\cswap\Claude Swap.exe
;   %LOCALAPPDATA%\Programs\cswap\Uninstall Claude Swap.exe
; and it takes over 0.1.x's uninstall entry (same HKCU key) instead of sitting beside it.
;
; TAKEOVER FROM 0.1.x. The Electron uninstaller is NEVER run: it would remove the
; shortcut and the entry this installer just took over, and it is ~100 MB of Electron
; that is gone anyway. The 0.1.x app is stopped (by exact path), its files are deleted by
; name, and %APPDATA%\cswap-desktop (settings.json, plans.json) is left alone: v1 reads it.
;
; Arguments:
;   /S            silent (v1's updater and 0.1.x's electron-updater both pass it)
;   /D=<dir>      install folder (NSIS built-in; must be last)
;   --force-run   start the app when done; the finish page is skipped then
;   --updated     passed by electron-updater and by v1's helper; ignored
;
; Test seam: /DAPP_GUID=<x> /DSHORTCUT=<name> /DRUN_VALUE=<name> /DDEFAULT_DIR=<dir> make a
; build that cannot touch a real install's entry, shortcut or login item.

Unicode true
ManifestDPIAware true
RequestExecutionLevel user
SetCompressor /SOLID lzma

!ifndef VERSION
  !error "pass /DVERSION=x.y.z"
!endif
!ifndef EXE
  !error "pass /DEXE=<path to the built claude-swap.exe>"
!endif
!ifndef OUTFILE
  !define OUTFILE "claude-swap-desktop-win-x64-setup.exe"
!endif

!define APP_NAME "Claude Swap"
!define EXE_NAME "Claude Swap.exe"
!define UNINSTALLER "Uninstall Claude Swap.exe"
; electron-builder's key for appId id.my.reinahatsuki.cswap-desktop (a UUID v5 of the
; appId), read off the PC: HKCU\...\Uninstall\<this> and HKCU\Software\<this>.
!ifndef APP_GUID
  !define APP_GUID "70c76982-575b-5e2d-80a5-0b90f967d60f"
!endif
!ifndef SHORTCUT
  !define SHORTCUT "Claude Swap"
!endif
; Electron's login item for 0.1.x (apps/cswap-desktop/src/autostart.rs).
!ifndef RUN_VALUE
  !define RUN_VALUE "electron.app.Claude Swap"
!endif
!ifndef DEFAULT_DIR
  !define DEFAULT_DIR "$LOCALAPPDATA\Programs\cswap"
!endif
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_GUID}"
!define INSTALL_KEY "Software\${APP_GUID}"
!define RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"
!define REPO_URL "https://github.com/TheSkinnyRat/cswap-desktop"

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "x64.nsh"

Name "${APP_NAME}"
OutFile "${OUTFILE}"
InstallDir "${DEFAULT_DIR}"
InstallDirRegKey HKCU "${INSTALL_KEY}" "InstallLocation"
BrandingText "${APP_NAME} ${VERSION}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${APP_NAME}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "${APP_NAME} Setup"
VIAddVersionKey "CompanyName" "Purwa Sabrang Ramadhan"
VIAddVersionKey "LegalCopyright" "Copyright (c) 2026 Purwa Sabrang Ramadhan"

!define MUI_ICON "..\..\resources\icon.ico"
!define MUI_UNICON "..\..\resources\icon.ico"
!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "Run ${APP_NAME}"
!define MUI_FINISHPAGE_RUN_FUNCTION LaunchApp
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipFinishWhenForced
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Var ForceRun

; Stops the copies running from DIR (stop-app.ps1: exact path, then PID).
!macro StopAppIn DIR
  DetailPrint "Stopping ${APP_NAME} in ${DIR}"
  nsExec::ExecToLog '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\stop-app.ps1" -Dir "${DIR}"'
  Pop $1
  ${If} $1 != 0
    DetailPrint "Some ${APP_NAME} processes could not be stopped (exit $1)"
  ${EndIf}
!macroend

; The files electron-builder put in DIR for 0.1.x, measured on the PC, by name: never a
; wildcard over a folder the user chose.
!macro RemoveElectronFiles DIR
  DetailPrint "Removing the 0.1.x (Electron) files from ${DIR}"
  RMDir /r "${DIR}\locales"
  RMDir /r "${DIR}\resources"
  Delete "${DIR}\chrome_100_percent.pak"
  Delete "${DIR}\chrome_200_percent.pak"
  Delete "${DIR}\resources.pak"
  Delete "${DIR}\d3dcompiler_47.dll"
  Delete "${DIR}\dxcompiler.dll"
  Delete "${DIR}\dxil.dll"
  Delete "${DIR}\ffmpeg.dll"
  Delete "${DIR}\libEGL.dll"
  Delete "${DIR}\libGLESv2.dll"
  Delete "${DIR}\vk_swiftshader.dll"
  Delete "${DIR}\vk_swiftshader_icd.json"
  Delete "${DIR}\vulkan-1.dll"
  Delete "${DIR}\icudtl.dat"
  Delete "${DIR}\snapshot_blob.bin"
  Delete "${DIR}\v8_context_snapshot.bin"
  Delete "${DIR}\LICENSE.electron.txt"
  Delete "${DIR}\LICENSES.chromium.html"
  Delete "${DIR}\uninstallerIcon.ico"
  ; Same name as v1's: deleted unrun, rewritten below.
  Delete "${DIR}\${UNINSTALLER}"
!macroend

Function LaunchApp
  Exec '"$INSTDIR\${EXE_NAME}"'
FunctionEnd

Function SkipFinishWhenForced
  ${If} $ForceRun == "1"
    Abort
  ${EndIf}
FunctionEnd

Function .onInit
  SetShellVarContext current
  StrCpy $ForceRun "0"
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "--force-run" $1
  ${IfNot} ${Errors}
    StrCpy $ForceRun "1"
  ${EndIf}
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "${APP_NAME} needs 64-bit Windows." /SD IDOK
    Abort
  ${EndIf}
FunctionEnd

Section "Install"
  InitPluginsDir
  File "/oname=$PLUGINSDIR\stop-app.ps1" "stop-app.ps1"
  !insertmacro StopAppIn "$INSTDIR"

  ${If} ${FileExists} "$INSTDIR\LICENSE.electron.txt"
  ${OrIf} ${FileExists} "$INSTDIR\resources\app.asar"
    !insertmacro RemoveElectronFiles "$INSTDIR"
  ${EndIf}

  SetOutPath "$INSTDIR"
  ; try: a locked file sets the error flag instead of an Abort/Retry dialog (which a
  ; silent update would have nobody to answer).
  SetOverwrite try
  ClearErrors
  File "/oname=${EXE_NAME}" "${EXE}"
  SetOverwrite on
  ${If} ${Errors}
    ; Still locked (an antivirus scan, a copy that would not stop): say so, do not
    ; register a half-installed app.
    MessageBox MB_ICONSTOP "Could not write $INSTDIR\${EXE_NAME}. Close ${APP_NAME} and run the installer again." /SD IDOK
    SetErrorLevel 2
    Abort
  ${EndIf}
  WriteUninstaller "$INSTDIR\${UNINSTALLER}"

  ; The values electron-builder wrote, under the same keys (replacing the 0.1.x entry).
  WriteRegStr HKCU "${INSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${INSTALL_KEY}" "KeepShortcuts" "true"
  WriteRegStr HKCU "${INSTALL_KEY}" "ShortcutName" "${SHORTCUT}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${APP_NAME} ${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "Purwa Sabrang Ramadhan"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Comments" "Desktop app for claude-swap (cswap): manage and switch Claude Code accounts without the terminal."
  WriteRegStr HKCU "${UNINSTALL_KEY}" "HelpLink" "${REPO_URL}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "URLInfoAbout" "${REPO_URL}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "URLUpdateInfo" "${REPO_URL}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Readme" "${REPO_URL}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\${EXE_NAME},0"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\${UNINSTALLER}"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\${UNINSTALLER}" /S'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" "$0"

  ; Where 0.1.x put it (per-user Start menu). Rewritten so its icon is the new exe's.
  CreateShortCut "$SMPROGRAMS\${SHORTCUT}.lnk" "$INSTDIR\${EXE_NAME}" "" "$INSTDIR\${EXE_NAME}" 0

  ; electron-updater's download cache: ~230 MB of old installers nobody will run again.
  RMDir /r "$LOCALAPPDATA\cswap-desktop-updater"

  ${If} $ForceRun == "1"
    Call LaunchApp
  ${EndIf}
SectionEnd

Function un.onInit
  SetShellVarContext current
FunctionEnd

Section "Uninstall"
  InitPluginsDir
  File "/oname=$PLUGINSDIR\stop-app.ps1" "stop-app.ps1"
  !insertmacro StopAppIn "$INSTDIR"

  Delete "$INSTDIR\${EXE_NAME}"
  Delete "$INSTDIR\${UNINSTALLER}"
  ; Not /r: whatever else is in the folder is not ours to delete.
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${SHORTCUT}.lnk"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  DeleteRegKey HKCU "${INSTALL_KEY}"
  ; The login item would start a missing exe at every logon.
  DeleteRegValue HKCU "${RUN_KEY}" "${RUN_VALUE}"
  ; Settings (%APPDATA%\cswap-desktop) stay, as with 0.1.x: a reinstall picks them up.
SectionEnd
