# -P sets the pane's style and -g prints it; both are deprecated in favour of
# the window-style options but still work, and the style lands where
# #{pane_bg} / #{pane_fg} read it back.
$TM display-message -p 'default fg=[#{pane_fg}] bg=[#{pane_bg}]'
$TM select-pane -P 'fg=red,bg=blue'; echo "rc=$?"
$TM select-pane -g
$TM display-message -p 'after -P fg=[#{pane_fg}] bg=[#{pane_bg}]'
$TM select-pane -P default
$TM select-pane -g
$TM display-message -p 'after default fg=[#{pane_fg}] bg=[#{pane_bg}]'
echo "== an invalid style =="
$TM select-pane -P 'notastyle' 2>&1; echo "rc=$?"
