# Pane scrollbars as GEOMETRY: the space a scrollbar takes out of its pane.
#
# pane-scrollbars is off by default (options-table.c:1592), so nothing in the
# suite had it turned on and the whole layout consequence of a visible scrollbar
# was unmeasured. It is not a decoration drawn over the pane: screen-redraw.c:767
# reserves scrollbar_style.width + scrollbar_style.pad columns and the pane is
# laid out narrower, on the left or the right depending on pane-scrollbars-position
# (screen-redraw.c:780). Only "on" reserves space -- "modal" and "auto-hide" are
# overlay modes (window.c:2223 window_pane_scrollbar_overlay) and leave the pane
# full width.
#
# So a port can be wrong in several independent ways that all show up as a pane
# rectangle: reserving space for an overlay mode, forgetting the pad, adding pad
# on the wrong side, shifting pane_left for "right", or applying the reservation
# to a horizontal split's inner pane as well as the outer. Every assertion below
# is a pane rectangle read back through formats, which makes this case entirely
# server-side and free of timing.
echo "--- defaults ---"
$TM show -gw pane-scrollbars
$TM show -gw pane-scrollbars-position
$TM show -gw pane-scrollbars-style
$TM show -gw pane-scrollbars-timeout

echo "--- state: only 'on' reserves columns ---"
for m in off on modal auto-hide off; do
  $TM set -gw pane-scrollbars $m
  printf '%-9s %s\n' "$m" "$($TM display -p '#{pane_width}x#{pane_height} #{pane_left}-#{pane_right}')"
done

echo "--- width/pad x position ---"
$TM set -gw pane-scrollbars on
for st in width=1,pad=0 width=2,pad=0 width=1,pad=1 width=3,pad=2 width=0,pad=0 width=1,pad=9; do
  $TM set -gw pane-scrollbars-style "$st"
  for pos in right left; do
    $TM set -gw pane-scrollbars-position $pos
    printf '%-14s %-5s %s\n' "$st" "$pos" "$($TM display -p '#{pane_left}-#{pane_right}:#{pane_width}')"
  done
done

echo "--- reservation is per pane, not per window ---"
# Each pane in a horizontal split gets its own scrollbar, so a 80-wide window
# split once loses TWO columns to scrollbars plus the one border column.
$TM set -gw pane-scrollbars-style 'width=1,pad=0'
$TM set -gw pane-scrollbars-position right
$TM split-window -h -d
$TM list-panes -F 'h #{pane_index} #{pane_left}-#{pane_right} w=#{pane_width}'
$TM set -gw pane-scrollbars-position left
$TM list-panes -F 'hL #{pane_index} #{pane_left}-#{pane_right} w=#{pane_width}'
$TM set -gw pane-scrollbars-position right

echo "--- style is pane-scoped: one pane wide, one pane default ---"
# pane-scrollbars-style carries OPTIONS_TABLE_PANE scope (options-table.c:1610),
# so a per-pane style must narrow only that pane and leave its sibling alone.
$TM set -p -t 0 pane-scrollbars-style 'width=4,pad=2'
$TM list-panes -F 'p #{pane_index} #{pane_left}-#{pane_right} w=#{pane_width}'
# A width larger than the pane must clamp rather than produce a zero or negative
# width pane.
$TM set -p -t 0 pane-scrollbars-style 'width=100,pad=0'
$TM list-panes -F 'clamp #{pane_index} #{pane_left}-#{pane_right} w=#{pane_width}'
$TM set -p -t 0 pane-scrollbars-style 'width=1,pad=0'
$TM kill-pane -t 1

echo "--- vertical split: rows are untouched, both panes narrow ---"
$TM split-window -v -d
$TM list-panes -F 'v #{pane_index} #{pane_left}-#{pane_right} #{pane_top}-#{pane_bottom} #{pane_width}x#{pane_height}'
$TM kill-pane -t 1

echo "--- rejected values ---"
$TM set -gw pane-scrollbars sometimes 2>&1
$TM set -gw pane-scrollbars-position middle 2>&1
$TM set -gw pane-scrollbars-style 'width=-1' 2>&1
$TM set -gw pane-scrollbars-style 'width=abc' 2>&1
$TM set -gw pane-scrollbars-timeout -5 2>&1
# ... and none of them disturbed the pane that the last accepted value produced.
printf 'after rejects %s\n' "$($TM display -p '#{pane_left}-#{pane_right}:#{pane_width}')"
$TM show -gw pane-scrollbars
$TM show -gw pane-scrollbars-position
$TM show -gw pane-scrollbars-style
