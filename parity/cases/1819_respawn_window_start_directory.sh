# respawn-window -c respawns in a different directory, and -e sets a variable
# for the new process; both are visible from the respawned command's output.
$TM set -g status off
d=$(mktemp -d)
mask() { perl -pe "s{\Q$d\E}{DIR}g; s{^/private}{}"; }
$TM new-window -d -n resp 'sleep 300'
$TM respawn-window -k -t resp -c "$d" -e ZTPAR_RESP=set-here 'pwd; printf "env=%s\n" "$ZTPAR_RESP"; sleep 300'
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t resp | grep -c env=)" = 1 ] && break
  sleep 0.2
done
$TM capture-pane -p -t resp | head -2 | mask
command rm -rf "$d"
