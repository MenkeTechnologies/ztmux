# Pane {last}, {next} and {previous} walk the pane list; {next} wraps.
$TM split-window -d
$TM split-window -d
$TM select-pane -t 2
$TM select-pane -t 0
for t in '{last}' '{next}' '{previous}'; do
  printf '%-12s %s\n' "$t" "$($TM display-message -p -t "$t" '#{pane_index}')"
done
echo "== from the last pane {next} wraps =="
$TM select-pane -t 2
$TM display-message -p -t '{next}' '#{pane_index}'
