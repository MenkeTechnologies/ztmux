# `#{history_bytes}` is the only format whose value is a direct function of the
# in-memory grid layout: it sums sizeof(struct grid_cell_entry) and, for cells
# that spilled, sizeof(struct grid_extd_entry) across the history. Both C structs
# are __packed -- 5 and 23 bytes, not the 8 and 24 the compiler would pick on its
# own -- so a port that declares them without the packed attribute reports a
# number that is too large by a few bytes per cell while every other format about
# the same grid still agrees.
#
# The empty-pane figure alone pins the per-line arithmetic against the fixed
# 80x24 geometry the runner provides.
$TM display-message -p 'empty  bytes=[#{history_bytes}] size=[#{history_size}] limit=[#{history_limit}]'
# Then the same reading once real lines have scrolled into the history, which
# brings the extended-entry path in: the wide characters below cannot sit in a
# plain grid_cell_entry and have to spill.
$TM new-window -d -n grid "cat"
$TM send-keys -t grid 'printf "plain %s\n" $(seq 1 40)' Enter
$TM send-keys -t grid 'printf "wide \xe6\x97\xa5\xe6\x9c\xac\xe8\xaa\x9e %s\n" $(seq 1 40)' Enter
$TM send-keys -t grid 'printf done\\n' Enter
# Wait for the scrollback to settle so the reading is not a race with the child.
# Bounded, and not a spin: each probe forks a client, so poll rather than busy-loop.
for _ in $(seq 1 100); do
  [ "$($TM display-message -p -t grid '#{history_size}')" -ge 40 ] && break
  sleep 0.05
done
$TM display-message -p -t grid 'filled size=[#{history_size}] limit=[#{history_limit}]'
$TM display-message -p -t grid 'filled bytes=[#{history_bytes}]'
