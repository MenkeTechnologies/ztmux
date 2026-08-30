# -O orders the listing: by index, by name or by time, and -r reverses whichever
# was chosen.
$TM split-window -d
$TM split-window -d
$TM select-pane -t 1
$TM select-pane -t 0
echo "default:"; $TM list-panes -F '  #{pane_index}'
for o in index name time; do
  echo "-O $o:"; $TM list-panes -O "$o" -F '  #{pane_index}' 2>&1
done
echo "-O index -r:"; $TM list-panes -O index -r -F '  #{pane_index}'
echo "== an unknown order =="
$TM list-panes -O nonsense -F '#{pane_index}' 2>&1 | head -2; echo "rc=$?"
