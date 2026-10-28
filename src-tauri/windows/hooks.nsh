; VARden: extra steps for the Windows installer (see tauri.conf.json,
; bundle > windows > nsis > installerHooks).

; Before uninstalling: take VARden's terminal-sync lines out of your shell
; profiles (each profile is backed up first), so no shell keeps loading a
; hook that no longer exists. Skipped when the uninstaller only runs as part
; of an update.
!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    DetailPrint "Removing VARden's terminal sync from shell profiles..."
    nsExec::ExecToLog '"$INSTDIR\${MAINBINARYNAME}.exe" --uninstall-cleanup'
    Pop $0
  ${EndIf}
!macroend

; After uninstalling: if you ticked "Delete the application data", also
; remove VARden's own folder (backups, sync files, logs). Your variables
; are never removed.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    SetShellVarContext current
    RmDir /r "$LOCALAPPDATA\VARden"
  ${EndIf}
!macroend
