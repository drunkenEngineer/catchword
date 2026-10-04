; Catchword's additions to Tauri's NSIS installer (REL-2).
;
; Uninstalling removes the index and the logs (PRIV-7): they hold text and
; names from the user's documents. An update (/UPDATE) keeps them (REL-5).
; Settings, which hold only the folder list and choices, stay unless the
; user ticks the box on the uninstall page (see English.nsh); with it ticked,
; Tauri removes the whole data folder. A reinstall rebuilds the index from
; the user's files by itself.

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    SetShellVarContext current
    RmDir /r "$LOCALAPPDATA\${BUNDLEID}\data"
    RmDir /r "$LOCALAPPDATA\${BUNDLEID}\logs"
    RmDir /r "$LOCALAPPDATA\${BUNDLEID}\EBWebView"
  ${EndIf}
!macroend
