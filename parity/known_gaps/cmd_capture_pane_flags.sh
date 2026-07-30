# GAP: capture-pane is missing four of next-3.7's flags. The port's arg string
# is "ab:CeE:JNpPqS:Tt:" against the C's "ab:CeE:FHJLMNpPqS:Tt:"
# (cmd-capture-pane.c:45), so -F (per-line grid flags), -H (hyperlink URIs,
# cmd_capture_pane_hyperlinks at cmd-capture-pane.c:111), -L (line numbers) and
# -M (capture the mode's screen rather than the pane's) are all rejected as
# unknown flags.
#
# -H needs more than the flag: the port has no GRID_LINE_HYPERLINK (tmux.h:804,
# set at grid.c:189) for the fast path to test, and -M needs the mode's
# get_screen callback (tmux.h:1180), which struct window_mode does not carry
# here. -F needs the same line-flag set.
$TM new-window -d -n hl 'printf "\0033]8;;https://example.com/one\0033\0134first\0033]8;;\0033\0134 plain\n"; sleep 300'
sleep 1
$TM capture-pane -pH -S 0 -E 1 -t hl 2>&1 | perl -pe 's/\e/<ESC>/g' | perl -pe "s{^(.*)\$}{[\$1]}"
$TM capture-pane -pF -S 0 -E 1 -t hl 2>&1 | perl -pe "s{^(.*)\$}{[\$1]}"
$TM capture-pane -pL -S 0 -E 1 -t hl 2>&1 | perl -pe "s{^(.*)\$}{[\$1]}"
$TM copy-mode -t hl
$TM capture-pane -pM -S 0 -E 1 -t hl 2>&1 | perl -pe "s{^(.*)\$}{[\$1]}"
