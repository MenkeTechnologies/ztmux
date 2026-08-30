# swap-pane -s and -t can name panes in different windows: the two panes trade
# places, each keeping its own contents.
#
# The contents are checked by COUNTING a marker rather than printing the screen,
# and the case stops with one line if the panes have not printed yet -- a slow
# machine must not turn "nothing captured yet" into a difference, and must not
# leave the comparison vacuously equal either.
$TM set -g automatic-rename off
$TM set -g status off
$TM new-window -d -n left "printf 'i-am-left\n'; sleep 300"
$TM new-window -d -n right "printf 'i-am-right\n'; sleep 300"
ready() {
  for _ in $(seq 1 25); do
    [ "$($TM capture-pane -p -t left | grep -c .)" -ge 1 ] &&
      [ "$($TM capture-pane -p -t right | grep -c .)" -ge 1 ] && return 0
    sleep 0.2
  done
  return 1
}
ready || { echo "panes produced no output in time"; exit 0; }
where() {
  printf 'left has i-am-left=%s i-am-right=%s | right has i-am-left=%s i-am-right=%s\n' \
    "$($TM capture-pane -p -t left  | grep -c i-am-left)" \
    "$($TM capture-pane -p -t left  | grep -c i-am-right)" \
    "$($TM capture-pane -p -t right | grep -c i-am-left)" \
    "$($TM capture-pane -p -t right | grep -c i-am-right)"
}
echo "before: $(where)"
$TM swap-pane -s left.0 -t right.0; echo "rc=$?"
echo "after:  $(where)"
echo "panes still one each: $($TM list-windows -F '#{window_name}:#{window_panes}' | grep -E '^(left|right):' | sort | tr '\n' ' ')"
