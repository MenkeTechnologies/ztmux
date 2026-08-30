# -r stops paste-buffer translating line feeds into carriage returns, and -p
# wraps the paste in the bracketed-paste markers when the pane has asked for
# them. Read what arrived back out of the pane's own echo.
$TM set -g status off
$TM set-buffer -b two 'aa
bb'
$TM split-window -d 'cat'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
$TM paste-buffer -b two -t "$pane"
for _ in $(seq 1 40); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c bb)" -ge 1 ] && break
  sleep 0.2
done
echo "== default translation =="
$TM capture-pane -p -t "$pane" | head -4
$TM set-buffer -b raw 'cc
dd'
$TM paste-buffer -r -b raw -t "$pane"
for _ in $(seq 1 40); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c dd)" -ge 1 ] && break
  sleep 0.2
done
echo "== with -r =="
$TM capture-pane -p -t "$pane" | head -6
