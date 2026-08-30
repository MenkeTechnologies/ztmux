# respawn-window needs -k when the window still has a live process; with -k it
# replaces it, and the pane count collapses back to one.
$TM new-window -d -n resp 'sleep 300'
$TM split-window -d -t resp 'sleep 300'
echo "panes before: $($TM list-panes -t resp -F '#{pane_index}' | wc -l | tr -d ' ')"
$TM respawn-window -t resp 'sleep 300' 2>&1; echo "without -k rc=$?"
$TM respawn-window -k -t resp 'sleep 300'; echo "with -k rc=$?"
echo "panes after: $($TM list-panes -t resp -F '#{pane_index}' | wc -l | tr -d ' ')"
