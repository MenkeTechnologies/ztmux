# Pane target tokens (cmd-find.c:63-74) name a position in the layout:
# {top}/{bottom}/{left}/{right}, the four corners, and the {up-of} family which
# is relative to the current pane.
$TM split-window -d          # pane 1 below pane 0
$TM split-window -d -h -t 0  # pane 2 right of pane 0
$TM select-pane -t 0
$TM list-panes -F '#{pane_index} #{pane_left},#{pane_top} #{pane_width}x#{pane_height}'
for t in '{top}' '{bottom}' '{left}' '{right}' '{top-left}' '{top-right}' '{bottom-left}' '{bottom-right}'; do
  printf '%-16s %s\n' "$t" "$($TM display-message -p -t "$t" '#{pane_index}')"
done
echo "== relative to pane 0 =="
for t in '{down-of}' '{right-of}'; do
  printf '%-12s %s\n' "$t" "$($TM display-message -p -t "$t" '#{pane_index}')"
done
