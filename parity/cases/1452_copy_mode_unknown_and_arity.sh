# send-keys -X dispatches through a table keyed by command name, with a
# per-entry argument count. Unknown names, wrong arity and commands that need a
# mode when none is open all produce specific errors, and getting any of them
# wrong is how a port ends up silently accepting nonsense — the failure mode
# that made the broken search look like it worked.
$TM new-window -d -n arity 'printf "text\n"; sleep 300'
sleep 1
# No mode open yet: -X commands are rejected rather than crashing.
$TM send-keys -X -t arity cursor-down 2>&1
$TM copy-mode -t arity
# Unknown command name.
$TM send-keys -X -t arity no-such-copy-command 2>&1
# Too many arguments to a zero-argument command.
$TM send-keys -X -t arity cursor-down extra 2>&1
# goto-line and the jumps require exactly one argument.
$TM send-keys -X -t arity goto-line 2>&1
$TM send-keys -X -t arity jump-forward 2>&1
$TM send-keys -X -t arity search-forward 2>&1
# A repeat count is accepted by -N and applies to the motion.
$TM send-keys -X -t arity history-top
$TM send-keys -N 3 -X -t arity cursor-right 2>&1
$TM display-message -p -t arity "repeat #{copy_cursor_y},#{copy_cursor_x}"
# The mode survived every one of those errors.
$TM display-message -p -t arity "alive #{pane_mode} #{pane_in_mode}"
