# move-pane -P names one of the 21 placements a floating pane can take
# (cmd-join-pane.c:81-131, the -centre spellings being synonyms of -center), and
# an unknown name is an error. Every placement is read back as the pane's own
# left/top, so the case pins the arithmetic and not just the names.
$TM set -g status off
$TM new-pane -d -E -x 20 -y 6
pane=%$($TM list-panes -F '#{pane_id}' | tr -d '%' | sort -n | tail -1)
echo "window: $($TM display-message -p '#{window_width}x#{window_height}')"
echo "pane:   $($TM display-message -p -t "$pane" '#{pane_width}x#{pane_height}')"
for p in top-left top-centre top-center top-right centre-left center-left centre center centre-right center-right bottom-left bottom-centre bottom-center bottom-right top-left-centre top-right-centre bottom-left-centre bottom-right-centre; do
  $TM move-pane -t "$pane" -P "$p" || echo "  $p FAILED"
  printf '%-20s %s\n' "$p" "$($TM display-message -p -t "$pane" '#{pane_left},#{pane_top}')"
done
echo "== an unknown position is an error =="
$TM move-pane -t "$pane" -P nowhere 2>&1; echo "rc=$?"
echo "== a tiled pane is not a floating pane =="
$TM move-pane -t 0 -P centre 2>&1; echo "rc=$?"
