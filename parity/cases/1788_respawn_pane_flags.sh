# respawn-pane replaces what a pane is running: without -k a live pane is
# refused, with -k it is replaced, and the new command's output appears in the
# same pane index.
#
# Respawning a DEAD pane is not exercised: on the vendored next-3.7 reference
# that takes the server down (`remain-on-exit on`, let the command exit, then
# `respawn-pane` -> "server exited unexpectedly"), while this port respawns it
# and carries on. There is no reference behaviour to compare against, so the
# case stops at the boundary rather than pinning one side of a crash.
$TM set -g status off
$TM split-window -d 'sleep 300'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
$TM display-message -p -t "$pane" 'live pane: dead=#{pane_dead}'
$TM respawn-pane -t "$pane" 'sleep 300' 2>&1; echo "without -k rc=$?"
$TM respawn-pane -k -t "$pane" 'printf "respawned\n"; sleep 300'; echo "with -k rc=$?"
for _ in $(seq 1 40); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c respawned)" = 1 ] && break
  sleep 0.2
done
$TM capture-pane -p -t "$pane" | head -1
$TM display-message -p -t "$pane" 'after respawn: dead=#{pane_dead} index=#{pane_index}'
