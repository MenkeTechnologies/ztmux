# The #{W:...} window loop installs six variables that exist NOWHERE else, so
# nothing outside the loop can reach them and no other case has ever expanded
# one (format.c:4825-4908):
#
#   window_after_active   i > 0     && l[i-1] == curw   -- the entry AFTER the
#                                                          current one, i.e. the
#                                                          current window is the
#                                                          PREVIOUS entry
#   window_before_active  i+1 < n   && l[i+1] == curw
#   next_window_index / next_window_active   from l[i+1], for all but the last
#   prev_window_index / prev_window_active   from l[i-1], for all but the first
#
# plus every `@`-prefixed window option of the neighbour, re-added under a
# `next_`/`prev_` prefix (format.c:4840-4850) -- the one place in tmux where a
# user option is renamed rather than read straight through.
#
# The edges are the whole point: the first entry has no prev_* and the last has
# no next_*, and an unset name expands to empty, so a port that adds the
# neighbours unconditionally (or that reads l[i+1] for the last entry) diverges
# here and matches everywhere else.
$TM new-window -d -n two
$TM new-window -d -n three
$TM new-window -d -n four
$TM select-window -t :2

echo "== windows, with the current one marked =="
$TM display-message -p '#{W:#{window_index}#{?window_active,*, } ,}'

echo "== window_after_active / window_before_active =="
$TM display-message -p '#{W:#{window_index}:a=#{window_after_active}b=#{window_before_active} ,}'

echo "== neighbour index and active flag, both directions =="
$TM display-message -p '#{W:[#{window_index} prev=#{prev_window_index}/#{prev_window_active} next=#{next_window_index}/#{next_window_active}] ,}'

echo "== unset at the edges: first has no prev, last has no next =="
$TM display-message -p '#{W:#{window_index}=#{?#{==:#{prev_window_index},},NOPREV,has-prev}/#{?#{==:#{next_window_index},},NONEXT,has-next} ,}'

echo "== a user option on the neighbour is re-added under next_/prev_ =="
$TM set -w -t :3 @nb 'three-value'
$TM display-message -p '#{W:#{window_index}:[#{next_@nb}][#{prev_@nb}] ,}'

echo "== the loop's own choose form still picks the active entry =="
$TM display-message -p '#{W:#{window_index}n,#{window_index}A}'
