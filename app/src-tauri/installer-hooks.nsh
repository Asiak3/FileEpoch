; FileEpoch: extra steps for the Windows installer (NSIS), see tauri.conf.json.
;
; The standard uninstaller already removes the "start with Windows" entry and, when the user ticks
; "Delete the application data", the settings folder. The saved password lives in the Windows
; Credential Manager, so the app itself is asked to delete it. Not done when updating.

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --forget-credentials'
  ${EndIf}
!macroend
