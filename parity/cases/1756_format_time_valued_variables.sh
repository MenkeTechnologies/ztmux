# The time-valued formats are wall clocks, so only their shape can be compared;
# each is rendered through the t modifier and masked down to that shape.
shape() { perl -pe 's/^\d+$/EPOCH-SHAPE-OK/; s/^\d\d:\d\d$/PRETTY-SHAPE-OK/; s/^\d+[smhd][0-9smhd]*$/RELATIVE-SHAPE-OK/; s/^$/EMPTY/'; }
$TM set-buffer -b timed 'x'
for v in start_time session_created session_last_attached buffer_created; do
  printf '%-24s %s\n' "$v" "$($TM display-message -p "#{$v}" | shape)"
  printf '%-24s %s\n' "  as t/p" "$($TM display-message -p "#{t/p:$v}" | shape)"
done
echo "== a dead pane's time =="
$TM setw -g remain-on-exit on
$TM split-window -d 'true'
for _ in $(seq 1 40); do
  [ "$($TM list-panes -F '#{pane_dead}' | grep -c '^1$')" = 1 ] && break
  sleep 0.2
done
printf '%-24s %s\n' pane_dead_time "$($TM display-message -p -t 1 '#{pane_dead_time}' | shape)"
printf '%-24s %s\n' '  on a live pane' "$($TM display-message -p -t 0 '[#{pane_dead_time}]')"
$TM setw -gu remain-on-exit
