# #{C:...} searches the pane's contents for a string and gives the line it is
# on, and #{R:...} repeats its argument. Both are single-case surfaces today.
$TM set -g status off
$TM split-window -d "printf 'alpha\nbravo\ncharlie\n'; sleep 300"
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 40); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c charlie)" = 1 ] && break
  sleep 0.2
done
echo "search hit:  [$($TM display-message -p -t "$pane" '#{C:bravo}')]"
echo "search miss: [$($TM display-message -p -t "$pane" '#{C:nowhere}')]"
echo "search in a conditional: $($TM display-message -p -t "$pane" '#{?#{C:alpha},found,missing}')"
echo "repeat:      [$($TM display-message -p '#{R:3:ab}')]"
echo "repeat zero: [$($TM display-message -p '#{R:0:ab}')]"
echo "repeat of a format: [$($TM display-message -p '#{R:2:#{session_windows}}')]"
