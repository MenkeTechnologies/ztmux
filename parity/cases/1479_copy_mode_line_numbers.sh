# copy-mode line numbers: the gutter, each numbering mode, and the position
# indicator that shares the top line with it.
#
# A pane is filled with numbered lines, put into copy mode, moved to a known
# cursor line, and the rendered screen is read back with capture-pane. The gutter
# is part of the drawn screen, so the capture shows it directly.
$TM new-window -d -n ln 'sleep 300'
$TM send-keys -t ln 'for i in $(seq 1 40); do echo "line$i"; done' Enter
sleep 0.6

cap() { $TM capture-pane -p -t ln | grep -v '^$' | head -6; }
enter() { $TM copy-mode -t ln; $TM send-keys -t ln -X history-top; $TM send-keys -t ln -X -N 5 cursor-down; }
leave() { $TM send-keys -t ln -X cancel; }

for mode in off default absolute relative hybrid; do
  $TM set-option -w -t ln copy-mode-line-numbers "$mode"
  enter
  printf '== %s\n' "$mode"
  cap
  leave
done

# The width is stable at three digits plus a space until the scrollback needs
# more, so content does not shift as lines accumulate.
$TM set-option -w -t ln copy-mode-line-numbers absolute
enter
printf '== width\n'
$TM display-message -p -t ln '#{copy_position}/#{copy_position_limit}'
leave

# Off keeps the old position-only top line; absolute reports scrollback lines.
$TM set-option -w -t ln copy-mode-line-numbers off
enter
printf '== position off: %s\n' "$($TM display-message -p -t ln '#{copy_position}/#{copy_position_limit}')"
leave

# The position indicator is a format, so it can be replaced wholesale.
$TM set-option -w -t ln copy-mode-position-format '#[align=right]<#{copy_position}>'
enter
printf '== custom position\n'
cap
leave
$TM set-option -w -t ln -u copy-mode-position-format

# -H hides the indicator entirely; the gutter stays.
$TM set-option -w -t ln copy-mode-line-numbers absolute
$TM copy-mode -H -t ln
$TM send-keys -t ln -X history-top
printf '== hidden position\n'
cap
leave
