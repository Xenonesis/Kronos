!macro NSIS_HOOK_PREINSTALL
  ; Terminate running Kronos desktop app and embedded kernel processes before file extraction
  DetailPrint "Stopping running Kronos processes..."
  nsExec::Exec 'taskkill /F /IM paseo-desktop.exe /T'
  nsExec::Exec 'powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-CimInstance Win32_Process | Where-Object { $$_.ExecutablePath -like ''*Kronos*'' -or $$_.ExecutablePath -like ''*paseo*'' } | ForEach-Object { Stop-Process -Id $$_.ProcessId -Force -ErrorAction SilentlyContinue }"'
  Sleep 500
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Register Windows App Execution Alias (App Paths) for kronos.exe and paseo.exe
  WriteRegStr SHCTX "Software\Microsoft\Windows\CurrentVersion\App Paths\kronos.exe" "" "$INSTDIR\paseo-desktop.exe"
  WriteRegStr SHCTX "Software\Microsoft\Windows\CurrentVersion\App Paths\kronos.exe" "Path" "$INSTDIR"
  WriteRegStr SHCTX "Software\Microsoft\Windows\CurrentVersion\App Paths\paseo.exe" "" "$INSTDIR\paseo-desktop.exe"
  WriteRegStr SHCTX "Software\Microsoft\Windows\CurrentVersion\App Paths\paseo.exe" "Path" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\kronos.exe" "" "$INSTDIR\paseo-desktop.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\kronos.exe" "Path" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\paseo.exe" "" "$INSTDIR\paseo-desktop.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\paseo.exe" "Path" "$INSTDIR"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Terminate running Kronos desktop app and embedded kernel processes before uninstallation
  DetailPrint "Stopping running Kronos processes..."
  nsExec::Exec 'taskkill /F /IM paseo-desktop.exe /T'
  nsExec::Exec 'powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-CimInstance Win32_Process | Where-Object { $$_.ExecutablePath -like ''*Kronos*'' -or $$_.ExecutablePath -like ''*paseo*'' } | ForEach-Object { Stop-Process -Id $$_.ProcessId -Force -ErrorAction SilentlyContinue }"'
  Sleep 500
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Clean up App Paths registry entries
  DeleteRegKey SHCTX "Software\Microsoft\Windows\CurrentVersion\App Paths\kronos.exe"
  DeleteRegKey SHCTX "Software\Microsoft\Windows\CurrentVersion\App Paths\paseo.exe"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\kronos.exe"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\paseo.exe"
  ; Delete application data on uninstallation
  RMDir /r "$LOCALAPPDATA\sh.paseo.desktop.tauri"
  RMDir /r "$PROFILE\.paseo"
!macroend
