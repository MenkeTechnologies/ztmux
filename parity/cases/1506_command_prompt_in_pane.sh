# command-prompt -P, the in-pane prompt.
#
# next-3.7 moved the prompt off the status line and into the pane for the
# copy-mode bindings that ask for input -- search, goto-line and the jump keys.
# 32 of the default bindings carry the flag. This was a known gap until the pane
# prompt was ported: window_pane gained prompt/prompt_data/prompt_cx,
# window.c's five pane-prompt functions were ported, the key routing and the
# draw were wired, and the 32 bindings got their -P back.
#
# The flag is part of the binding string list-keys prints back, so most of this
# is checkable without a client; the rendering itself is covered by case 1507.
$TM list-keys -T copy-mode g
$TM list-keys -T copy-mode-vi :
$TM list-keys -T copy-mode C-s
$TM list-keys -T copy-mode-vi /
# And the flag is accepted rather than rejected. Without a client there is no
# pane to own the prompt, so both binaries report the same missing client.
$TM command-prompt -P -p '(x)' 'display-message hi' 2>&1
echo "rc=$?"
# -P with -b does not wait on the queue either.
$TM command-prompt -b -P -p '(x)' 'display-message hi' 2>&1
echo "rc=$?"
# The remaining 28 keys the C gives the flag.
for k in C-r F T f t M-1 M-9; do $TM list-keys -T copy-mode "$k"; done
for k in 1 9 ? F T f t; do $TM list-keys -T copy-mode-vi "$k"; done
