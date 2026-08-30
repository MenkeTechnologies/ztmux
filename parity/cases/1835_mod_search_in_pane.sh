# #{C:...} searches the pane's content and #{C/r:...} treats the pattern as a
# regular expression; both give the line number of the first match or nothing.
$TM set -g status off
$TM split-window -d "printf 'alpha\nbaaar\ngamma\n'; sleep 300"
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c gamma)" = 1 ] && break
  sleep 0.2
done
echo "literal hit:  [$($TM display-message -p -t "$pane" '#{C:baaar}')]"
echo "literal miss: [$($TM display-message -p -t "$pane" '#{C:nowhere}')]"
echo "regex hit:    [$($TM display-message -p -t "$pane" '#{C/r:^ba+r$}')]"
echo "regex miss:   [$($TM display-message -p -t "$pane" '#{C/r:^zz+$}')]"
