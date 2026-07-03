# GAP: session status line (status-format[2]) — the "S:" session-status format
# that lists sessions, added in next-3.7. Depends on the unported session-status
# subsystem (session-status-style / session-status-current-style / #{session_alert}
# / #{S:...} loop). ztmux's status-format array has no index [2].
$TM show-options -g status-format[2]
