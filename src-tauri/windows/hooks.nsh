; Installer hooks for Dock (wired up in tauri.conf.json, bundle.windows.nsis.installerHooks).
;
; The dock hides the Windows taskbar while it runs. Killing it outright would leave the
; taskbar hidden, so before files are replaced or removed the dock is asked to quit properly:
; `--quit` reaches a running dock through single-instance and it exits through its normal
; path, which puts the taskbar back. With no dock running, `--quit` restores a taskbar that a
; crashed dock left hidden, then exits.

!macro DOCK_QUIT_RUNNING
  IfFileExists "$INSTDIR\${MAINBINARYNAME}.exe" 0 +3
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --quit'
    Sleep 1500
!macroend

!macro NSIS_HOOK_PREINSTALL
  ; Upgrading over a running dock.
  !insertmacro DOCK_QUIT_RUNNING
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro DOCK_QUIT_RUNNING
  ; "Launch at startup" (tauri-plugin-autostart) and the on/off switch Windows keeps for it.
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "${PRODUCTNAME}"
!macroend
