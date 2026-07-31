# pane-scrollbars: the options, and the column a reserved scrollbar takes out of
# the pane.
#
# Only `on` reserves — `modal` and `auto-hide` draw over the pane instead, since
# a bar that comes and goes cannot keep resizing the pane under the running
# program. The width and padding come from `pane-scrollbars-style`, and
# `pane-scrollbars-position` decides which side, which moves `pane_left` as well
# as narrowing `pane_width`.
$TM show-options -wg pane-scrollbars
$TM show-options -wg pane-scrollbars-position
$TM show-options -wg pane-scrollbars-style
$TM show-options -wg pane-scrollbars-timeout

$TM new-window -d -n sb 'sleep 300'
g() { printf '%-22s %s\n' "$1" "$($TM display-message -p -t sb '#{pane_width}x#{pane_height}+#{pane_left},#{pane_top}')"; }
o() { $TM set-option -w -t sb "$@"; }

g 'default(off)'
for state in on modal auto-hide off; do
  o pane-scrollbars "$state"
  g "$state"
done

# Width and padding both come out of the pane, on either side.
o pane-scrollbars on
for pos in right left; do
  o pane-scrollbars-position "$pos"
  for st in 'bg=red' 'bg=red,width=3' 'bg=red,width=3,pad=2' 'bg=red,width=1,pad=0'; do
    o pane-scrollbars-style "$st"
    g "$pos $st"
  done
done

# A style that sets no width or padding falls back to the defaults rather than
# reserving nothing.
o pane-scrollbars-style 'bg=blue'
g 'no width/pad'
$TM show-options -wv -t sb pane-scrollbars-style

# Splitting with a scrollbar reserved: each pane loses its own column.
o pane-scrollbars-position right
o pane-scrollbars-style 'bg=red,width=1,pad=0'
$TM split-window -h -t sb -d 'sleep 300'
$TM list-panes -t sb -F '#{pane_index} #{pane_width}x#{pane_height}+#{pane_left},#{pane_top}'
o pane-scrollbars off
$TM list-panes -t sb -F '#{pane_index} #{pane_width}x#{pane_height}+#{pane_left},#{pane_top}'

# Bad values are refused rather than silently accepted.
o pane-scrollbars bogus 2>&1 | head -1
o pane-scrollbars-position middle 2>&1 | head -1
o pane-scrollbars-timeout -1 2>&1 | head -1
$TM show-options -wg pane-scrollbars
