# send-prefix sends the session's prefix key into the pane; -2 sends prefix2,
# which is `None` by default (options-table.c, `KEYC_NONE`) -- that default is
# the interesting case, because the key still travels the whole input path.
# The lock commands need a client and say so.
$TM set -g status off
$TM split-window -d 'cat'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
echo "prefix2 default: [$($TM show -gv prefix2)]"
$TM send-prefix -t "$pane"; echo "send-prefix rc=$?"
$TM send-prefix -2 -t "$pane"; echo "send-prefix -2 with prefix2 unset rc=$?"
$TM set -g prefix2 M-b
$TM send-prefix -2 -t "$pane"; echo "send-prefix -2 with prefix2 set rc=$?"
$TM set -gu prefix2
echo "server still there: $($TM list-panes -F '#{pane_index}' | wc -l | tr -d ' ')"
$TM lock-client 2>&1; echo "lock-client rc=$?"
$TM lock-session -t 0 2>&1; echo "lock-session rc=$?"
