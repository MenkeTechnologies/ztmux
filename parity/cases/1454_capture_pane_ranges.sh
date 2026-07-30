# capture-pane's -S/-E range arithmetic runs over history lines and the visible
# screen with different signs: negative values count back into scrollback, "-"
# means the extreme end, and a range that runs off either end is clamped rather
# than wrapping. That is the same signed/unsigned boundary arithmetic that made
# pane and layout-cell offsets wrong, but here it is directly observable.
$TM new-window -d -n rng 'i=1; while [ $i -le 60 ]; do echo "row-$i"; i=$((i+1)); done; sleep 300'
sleep 1
$TM display-message -p -t rng "hist=#{history_size} h=#{pane_height}"
for r in "-S 0 -E 2" "-S -5 -E -3" "-S - -E 1" "-S 0 -E -" "-S 58 -E 59" "-S 1000 -E 1001" "-S -1000 -E -999"; do
  echo "== capture $r"
  # shellcheck disable=SC2086
  $TM capture-pane -p -t rng $r 2>&1 | perl -pe "s{^(.*)\$}{[\$1]}"
done
# -J joins wrapped lines, -N preserves trailing spaces, -T strips them.
echo "== flags"
$TM capture-pane -p -J -S 0 -E 1 -t rng | perl -pe "s{^(.*)\$}{[\$1]}"
$TM capture-pane -p -N -S 0 -E 1 -t rng | perl -pe "s{^(.*)\$}{[\$1]}"
$TM capture-pane -p -T -S 0 -E 1 -t rng | perl -pe "s{^(.*)\$}{[\$1]}"
