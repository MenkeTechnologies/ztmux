# Key names are looked up when -l is absent: Space, Tab and a hex-like word are
# distinct from their literal text. Unknown key names are an error.
$TM set -g status off
$TM split-window -d 'cat'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
$TM send-keys -t "$pane" a Space b Tab c
for _ in 1 2 3 4 5 6 7 8 9 10; do
  out=$($TM capture-pane -p -t "$pane" | head -1)
  [ -n "$out" ] && break
  sleep 0.2
done
$TM capture-pane -p -t "$pane" | head -1 | perl -pe 's/\t/<TAB>/g'
echo "== an unknown key name =="
$TM send-keys -t "$pane" NoSuchKey 2>&1; echo "rc=$?"
