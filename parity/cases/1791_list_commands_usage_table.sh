# The WHOLE command table's usage lines, diffed against next-3.7's.
#
# This is the same idea as case 1498 for the default bindings: a usage string is
# DATA, so the anti-drift gate (which compares function names) never looked at
# it, and nothing else in the suite compared more than a handful. Transcribing
# ~90 of them by hand is exactly the sort of thing that drifts, and it had: the
# choose-tree family had lost -k (and -h/-i), break-pane described -x/-y/-X/-Y
# with the wrong words, command-prompt was missing -F/-N/-P, display-menu and
# display-popup had lost a space, send-keys and send-prefix showed -t as
# required, server-access had dropped its -t altogether, and bind-key,
# new-session, respawn-pane, respawn-window, set-buffer and show-hooks each
# ended with the wrong optional argument.
#
# The six lines that are SUPPOSED to differ are excluded by name: ztmux's five
# list-* commands document their structured-output flags, and znative exists
# only here. Everything else must match byte for byte.
$TM list-commands \
  | grep -vE '^(list-buffers|list-clients|list-panes|list-sessions|list-windows|znative) ' \
  | sort
echo "== the excluded ones are still there =="
$TM list-commands -F '#{command_list_name}' \
  | grep -cE '^(list-buffers|list-clients|list-panes|list-sessions|list-windows)$'
echo "== and every command still has a usage line =="
$TM list-commands -F '#{?#{==:#{command_list_usage},},MISSING-USAGE:#{command_list_name},}' | grep -c MISSING-USAGE
