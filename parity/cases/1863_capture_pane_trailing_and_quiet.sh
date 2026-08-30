# -T keeps the trailing spaces on a line the way -J does without joining it
# (cmd-capture-pane.c:237), -P captures only what has not been consumed yet, and
# -q keeps quiet about a pane that has nothing to capture.
$TM set -g status off
$TM split-window -d "printf 'trailing   \nplain\n'; sleep 300"
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c plain)" = 1 ] && break
  sleep 0.2
done
echo "default line lengths:"
$TM capture-pane -p -t "$pane" | perl -ne 'print length($_)-1, "\n" if $. <= 2' | sed 's/^/  /'
echo "with -T:"
$TM capture-pane -pT -t "$pane" | perl -ne 'print length($_)-1, "\n" if $. <= 2' | sed 's/^/  /'
echo "== -P on a pane with nothing pending =="
$TM capture-pane -pP -t "$pane" | wc -l | tr -d ' '
echo "== -q on a target that cannot be captured =="
$TM capture-pane -pq -t 99 2>&1; echo "rc=$?"
$TM capture-pane -p -t 99 2>&1; echo "rc=$?"
