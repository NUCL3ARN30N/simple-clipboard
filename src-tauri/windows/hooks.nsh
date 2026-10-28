; Restore the built-in Windows Win+V shortcut if Simple Clipboard disabled it.
!macro NSIS_HOOK_POSTUNINSTALL
  ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced" "DisabledHotkeys"
  StrCmp $0 "V" 0 +2
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced" "DisabledHotkeys"
!macroend
