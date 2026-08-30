# respawn-pane -c respawns in another directory and -e sets a variable for the
# new process, the same pair respawn-window takes.
$TM set -g status off
d=$(mktemp -d)
mask() { perl -pe "s{\Q$d\E}{DIR}g; s{^/private}{}"; }
$TM split-window -d 'sleep 300'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
$TM respawn-pane -k -t "$pane" -c "$d" -e ZTPAR_RP=set 'pwd; printf "env=%s\n" "$ZTPAR_RP"; sleep 300'
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c env=)" = 1 ] && break
  sleep 0.2
done
$TM capture-pane -p -t "$pane" | head -2 | mask
$TM display-message -p -t "$pane" 'start_path=[#{pane_start_path}]' | mask
command rm -rf "$d"
