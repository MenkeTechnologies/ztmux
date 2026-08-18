# The dotmatrix status bar as a CLIENT actually paints it.
#
# Every other case in this suite asks the server what it stored. This one asks
# what got drawn, because dotmatrix's status-left is a STYLE string --
# `#[fg=colour235,bg=colour76,bold] #S ` -- and a style that parses and prints
# back correctly can still paint nothing.
#
# There is no terminal in the parity harness, so one is built: a second server
# runs inside a pane of the first, and a client attaches to it from there. The
# inner client's output lands in the outer pane's grid, so `capture-pane -e` on
# the OUTER server re-serialises exactly the attributes the inner client emitted.
#
# What that catches, and nothing else here does: tmux's DEFAULT status-style is
# `bg=themegreen,fg=themeblack`, next-3.7's theme colours, which resolve to a
# real colour only at render time (tty.c:2800 tty_map_theme_colour). A port that
# parses `themegreen` and prints it back but never maps it draws the whole bar
# unstyled, while every show-options case stays green. dotmatrix sets only
# status-left, so the rest of its bar is exactly that default.
set -- $TM
BIN="$1"
ISOCK="hrst_$$_inner"

$BIN -L "$ISOCK" -f /dev/null new-session -d -s hr -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" new-window -d -n two 'sleep 300'
# The clock and hostname on the right are not comparable; the left is the test.
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
$BIN -L "$ISOCK" set -g default-terminal screen-256color
$BIN -L "$ISOCK" set -g status-left-length 18
$BIN -L "$ISOCK" set -g status-left '#[fg=colour235,bg=colour76,bold] #S '

$TM set -g default-terminal screen-256color
$TM new-window -d -n client "$BIN -L $ISOCK attach -t hr"
sleep 2

echo "text:"
$TM capture-pane -p -t client | tail -1 | cat -v
echo "attributes:"
$TM capture-pane -p -e -t client | tail -1 | cat -v

# And with the default status-left, so the theme-coloured default is the whole
# line rather than a tail after the styled segment.
$BIN -L "$ISOCK" set -gu status-left
sleep 1.5
echo "default-left:"
$TM capture-pane -p -e -t client | tail -1 | cat -v

$BIN -L "$ISOCK" kill-server 2>/dev/null
