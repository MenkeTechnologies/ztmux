# The mirrored layouts put the main pane on the other side: main-horizontal
# puts it on top and -mirrored at the bottom, main-vertical on the left and
# -mirrored on the right (layout-set.c). The geometry is what tells them apart.
$TM split-window -d
$TM split-window -d
$TM setw -g main-pane-height 8
$TM setw -g main-pane-width 30
for l in main-horizontal main-horizontal-mirrored main-vertical main-vertical-mirrored; do
  $TM select-layout "$l" >/dev/null
  echo "== $l =="
  $TM list-panes -F '  #{pane_index} #{pane_left},#{pane_top} #{pane_width}x#{pane_height}' | sort
  $TM display-message -p "  layout=$($TM display-message -p '#{window_layout}' | perl -pe 's/^[0-9a-f]{4},/CKSUM,/')"
done
echo "== an unknown layout name =="
$TM select-layout notalayout 2>&1; echo "rc=$?"
