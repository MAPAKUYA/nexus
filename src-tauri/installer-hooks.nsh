; NEXUS installer hooks (Tauri NSIS bundler).
; Everything else (Start menu shortcut, uninstall entry, removing the autostart entry on uninstall,
; the "delete app data" checkbox) is done by Tauri's standard template.

; Remember the language chosen in the installer. NEXUS reads it on its FIRST launch only
; (afterwards the language lives in the app's own settings). $LANGUAGE is a Windows LCID.
!macro NSIS_HOOK_POSTINSTALL
  Push $R8
  StrCpy $R8 "en"
  ${If} $LANGUAGE == 1049
    StrCpy $R8 "ru"
  ${ElseIf} $LANGUAGE == 1034
  ${OrIf} $LANGUAGE == 3082
    StrCpy $R8 "es"
  ${ElseIf} $LANGUAGE == 1031
    StrCpy $R8 "de"
  ${ElseIf} $LANGUAGE == 1036
    StrCpy $R8 "fr"
  ${ElseIf} $LANGUAGE == 1046
    StrCpy $R8 "pt"
  ${ElseIf} $LANGUAGE == 2052
    StrCpy $R8 "zh"
  ${ElseIf} $LANGUAGE == 1041
    StrCpy $R8 "ja"
  ${EndIf}
  WriteRegStr HKCU "Software\NEXUS" "Language" "$R8"
  Pop $R8
!macroend

; Remove our value on a real uninstall (not on an update, which runs the uninstaller silently).
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\NEXUS" "Language"
    DeleteRegKey /ifempty HKCU "Software\NEXUS"
  ${EndIf}
!macroend
