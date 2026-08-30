# paste-buffer -p wraps the paste in the bracketed-paste markers, but only when
# the pane has asked for bracketed paste (DECSET 2004); without that the markers
# are not sent even with -p.
$TM set -g status off
$TM set-buffer -b bp 'pasted-text'
$TM split-window -d 'cat -v'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 25); do
  [ -n "$($TM display-message -p -t "$pane" '#{pane_id}')" ] && break
  sleep 0.2
done
$TM paste-buffer -p -b bp -t "$pane"
$TM send-keys -t "$pane" Enter
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c pasted-text)" -ge 1 ] && break
  sleep 0.2
done
echo "without the mode set:"
$TM capture-pane -p -t "$pane" | head -2 | sed 's/^/  /'
# Turn bracketed paste on from inside the pane, then paste again.
$TM send-keys -t "$pane" -H 1b 5b 3f 32 30 30 34 68
$TM set-buffer -b bp2 'second-paste'
$TM paste-buffer -p -b bp2 -t "$pane"
$TM send-keys -t "$pane" Enter
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c second-paste)" -ge 1 ] && break
  sleep 0.2
done
echo "with the mode set:"
$TM capture-pane -p -t "$pane" | grep -n '200~\|second-paste' | head -3 | sed 's/^/  /'
