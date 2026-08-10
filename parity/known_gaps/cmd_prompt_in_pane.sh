# GAP: command-prompt -P (the in-pane prompt).
#
# next-3.7 moved the prompt off the status line and into the pane for the copy-mode
# bindings that ask for input -- search, goto-line and the jump keys. The flag is
# -P, and 32 of the default bindings carry it. ztmux has no in-pane prompt: struct
# window_pane has no prompt fields, so there is nothing for PROMPT_ISPANE to set,
# and the default table was written without the flag to match what the port can do.
#
# The divergence is visible without a client, because the flag is part of the
# binding string that list-keys prints back.
$TM list-keys -T copy-mode g
$TM list-keys -T copy-mode-vi :
$TM list-keys -T copy-mode C-s
$TM list-keys -T copy-mode-vi /
# And the flag itself is rejected outright.
$TM command-prompt -P -p '(x)' 'display-message hi' 2>&1
