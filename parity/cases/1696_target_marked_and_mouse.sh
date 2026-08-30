# ~ and {marked} name the marked pane (cmd-find.c:1055); with nothing marked
# they expand to nothing rather than erroring, and = / {mouse} do the same with
# no mouse event (cmd-find.c:1025).
#
# {active} and {current} are NOT exercised here: the vendored next-3.7 reference
# crashes its own server on `display-message -p -t {active}` with no client
# attached, so there is nothing to compare against.
$TM split-window -d
echo "before marking: [$($TM display-message -p -t '{marked}' '#{pane_index}' 2>&1)] rc=$?"
echo "no mouse:       [$($TM display-message -p -t '{mouse}' '#{pane_index}' 2>&1)] rc=$?"
echo "bare =:         [$($TM display-message -p -t '=' '#{pane_index}' 2>&1)] rc=$?"
$TM select-pane -m -t 1
echo "after marking:  [$($TM display-message -p -t '{marked}' '#{pane_index}' 2>&1)]"
echo "tilde:          [$($TM display-message -p -t '~' '#{pane_index}' 2>&1)]"
$TM list-panes -F '#{pane_index}:#{pane_marked}' | sort
$TM select-pane -M
echo "after clearing: [$($TM display-message -p -t '{marked}' '#{pane_index}' 2>&1)]"
