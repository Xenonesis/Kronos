!macro NSIS_HOOK_PREINSTALL
  ; Terminate running desktop app and background daemon before file extraction
  DetailPrint "Stopping running Kronos processes..."
  nsExec::Exec 'taskkill /F /IM paseo-desktop.exe /T'
  nsExec::Exec 'powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-CimInstance Win32_Process | Where-Object { $$_.ExecutablePath -like ''*Kronos*'' -or $$_.ExecutablePath -like ''*paseo*'' } | ForEach-Object { Stop-Process -Id $$_.ProcessId -Force -ErrorAction SilentlyContinue }"'
  Sleep 500
!macroend

!macro NSIS_HOOK_POSTINSTALL
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Terminate running desktop app and background daemon before uninstallation
  DetailPrint "Stopping running Kronos processes..."
  nsExec::Exec 'taskkill /F /IM paseo-desktop.exe /T'
  nsExec::Exec 'powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-CimInstance Win32_Process | Where-Object { $$_.ExecutablePath -like ''*Kronos*'' -or $$_.ExecutablePath -like ''*paseo*'' } | ForEach-Object { Stop-Process -Id $$_.ProcessId -Force -ErrorAction SilentlyContinue }"'
  Sleep 500
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Delete application data on uninstallation
  RMDir /r "$LOCALAPPDATA\sh.paseo.desktop.tauri"
  RMDir /r "$PROFILE\.paseo"
!macroend
