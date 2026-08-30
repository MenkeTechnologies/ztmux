# The scrolling *-and-cancel commands leave the mode when they run out of
# history to scroll, and stay in it while there is more.
$TM set -g status off
$TM split-window -d "i=1; while [ \$i -le 60 ]; do echo line \$i; i=\$((i+1)); done; sleep 300"
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 40); do
  [ "$($TM display-message -p -t "$pane" '#{history_size}')" -ge 30 ] && break
  sleep 0.2
done
enter() { $TM send-keys -X -t "$pane" cancel 2>/dev/null; $TM copy-mode -t "$pane"; }
state() { $TM display-message -p -t "$pane" "$1 in_mode=#{pane_in_mode} scroll=#{scroll_position}"; }

enter
$TM send-keys -X -t "$pane" page-up
state 'after page-up:'
$TM send-keys -X -t "$pane" page-down-and-cancel
state 'after page-down-and-cancel:'

enter
$TM send-keys -X -t "$pane" halfpage-up
state 'after halfpage-up:'
$TM send-keys -X -t "$pane" halfpage-down-and-cancel
state 'after halfpage-down-and-cancel:'

enter
$TM send-keys -X -t "$pane" scroll-up
state 'after scroll-up:'
$TM send-keys -X -t "$pane" scroll-down-and-cancel
state 'after scroll-down-and-cancel:'
