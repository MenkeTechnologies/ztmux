# default-command is what a pane runs when no command is given; it is a shell
# command, so it goes through the shell rather than being exec'd directly.
#
# Each check polls for the pane's own output and then reports what it found by
# NAME rather than printing the screen: if a pane is slow on a loaded machine
# both binaries print the same "not yet" line instead of one of them printing a
# blank and the other the text.
$TM set -g status off
$TM set -g default-command 'printf "spawned by default-command\n"; sleep 300'
$TM split-window -d
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c 'spawned by default-command')" = 1 ] && break
  sleep 0.2
done
echo "default-command ran: $($TM capture-pane -p -t "$pane" | grep -c 'spawned by default-command')"
$TM set -gu default-command
echo "== an explicit command still wins =="
$TM split-window -d 'printf "explicit\n"; sleep 300'
pane2=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t "$pane2" | grep -c explicit)" = 1 ] && break
  sleep 0.2
done
echo "explicit command ran: $($TM capture-pane -p -t "$pane2" | grep -c explicit)"
echo "and the default did not: $($TM capture-pane -p -t "$pane2" | grep -c 'spawned by default-command')"
