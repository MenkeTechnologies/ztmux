# The window list entries themselves: which FORMAT and which STYLE each window in
# the status bar gets drawn with.
#
# The stock status-format[0] does not simply concatenate window-status-format for
# every window. It picks per window between window-status-format and
# window-status-current-format, and layers a style chosen by a nested conditional
# -- current vs normal, then last/bell/activity on top, and a `default`
# window-status-current-style falls back to window-status-style rather than to
# nothing. All of that runs only while a client is drawing, so the option values
# being right (which show-options cases already cover) says nothing about the
# entry a user actually sees.
#
# Captured with `capture-pane -e`, so the SGR runs are compared, not just the
# text: a port that resolves the styles but emits them in the wrong order, drops
# the reset between entries, or paints the current entry with the normal style is
# byte-different here and identical everywhere else in the suite.
#
# Built like cases 1504/1507/1508: a second server inside a pane of the first with
# a client attached, so capture-pane on the OUTER server reads back the inner
# client's status row.
set -- $TM
BIN="$1"
ISOCK="wss_$$_inner"

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" new-window -d -t alpha -n two 'sleep 300'
$BIN -L "$ISOCK" new-window -d -t alpha -n three 'sleep 300'
# The default status-right is a clock plus the hostname; neither is comparable.
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the PORTED
# rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" set -g default-terminal screen-256color
$TM set -g default-terminal screen-256color

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"

# Fence instead of sleeping: under suite load a blind sleep races the repaint.
# Formats and styles are set FIRST and a fixed-width status-left marker LAST, so
# the marker can only appear in a redraw that already used them. status-left is
# not under test here, and every marker is the same width, so the window list
# starts at the same column in every step.
step() {   # $1 = 4-column marker
  local i=0 got
  $BIN -L "$ISOCK" set -g status-left "$1"
  while [ $i -lt 80 ]; do
    got=$($TM capture-pane -p -t client 2>/dev/null)
    case "$got" in *"$1"*) sleep 0.3; return 0 ;; esac
    i=$((i+1)); sleep 0.1
  done
  echo "step: timed out waiting for marker [$1]"
  return 1
}
text()  { $TM capture-pane -p -t client    | tail -1 | cat -v | perl -pe 's/[ \t]+$//'; }
attrs() { $TM capture-pane -p -e -t client | tail -1 | cat -v | perl -pe 's/[ \t]+$//'; }

# 1. Stock everything. window-status-current-style is `underscore` out of the box,
#    so the current entry must carry SGR 4 and the others must not.
step 'AA| '
echo "stock text : [$(text)]"
echo "stock attrs: [$(attrs)]"

# 2. Different format strings for the two roles. The point is not the strings but
#    that the CURRENT window -- and only it -- takes the current one.
$BIN -L "$ISOCK" set -g window-status-format 'n<#{window_index}.#{window_name}>'
$BIN -L "$ISOCK" set -g window-status-current-format 'C[#{window_index}.#{window_name}]'
step 'BB| '
echo "formats    : [$(text)]"

# 3. Explicit, clearly different styles on the two roles.
$BIN -L "$ISOCK" set -g window-status-style 'fg=colour33,bg=colour17'
$BIN -L "$ISOCK" set -g window-status-current-style 'fg=colour226,bg=colour88,bold'
step 'CC| '
echo "styles     : [$(attrs)]"

# 4. window-status-current-style `default` is NOT "no style": the stock
#    status-format falls back to window-status-style for the current entry too, so
#    all three entries end up identically styled.
$BIN -L "$ISOCK" set -g window-status-current-style default
step 'DD| '
echo "cur=default: [$(attrs)]"

# 5. window-status-last-style, layered on top of the normal style for the window
#    carrying the `-` last flag after a switch. Restoring a real current style
#    first keeps the current entry distinguishable from the last entry.
$BIN -L "$ISOCK" set -g window-status-current-style 'fg=colour226,bg=colour88,bold'
$BIN -L "$ISOCK" set -g window-status-last-style 'fg=colour46,underscore'
$BIN -L "$ISOCK" select-window -t alpha:1
step 'EE| '
echo "last text  : [$(text)]"
echo "last attrs : [$(attrs)]"

$BIN -L "$ISOCK" kill-server 2>/dev/null
