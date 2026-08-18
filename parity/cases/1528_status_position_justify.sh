# Where the status bar LANDS on the screen, and where the window list sits in it.
#
# status-position, status-justify and window-status-separator only exist at draw
# time: the server stores three strings and nothing more, so every show-options
# case in this suite passes whether or not status_redraw ever honours them. What
# is pinned here is the geometry status.c computes -- which screen ROW the bar
# occupies (top vs bottom, and that the other row stays application content), and
# the column the window list starts at for each of the four justify modes, which
# is derived from the left/right widths, not from the screen width alone.
#
# A client is required for any of it. There is no terminal in the parity harness,
# so one is built the way cases 1504/1507/1508 do: a second server runs inside a
# pane of the first with a client attached to it, and capture-pane on the OUTER
# server reads back exactly the rows the inner client painted.
set -- $TM
BIN="$1"
ISOCK="spj_$$_inner"

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

# Never sleep blind waiting for a repaint: this case runs alongside the rest of
# the suite and a fixed sleep races the client's draw under load. Each step sets
# status-justify/status-position FIRST, then stamps a fixed-width marker into
# status-left; the marker can only appear in a redraw that already saw the new
# geometry, so polling for it is an exact fence. The markers are all 4 columns
# wide so the window-list offsets stay comparable between steps.
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
scrub()  { perl -pe 's/\s+$//'; }
first()  { $TM capture-pane -p -t client | head -1 | cat -v | scrub; }
last()   { $TM capture-pane -p -t client | tail -1 | cat -v | scrub; }

# 1. Default: bottom, left-justified. The bar is the inner client's last row; the
#    first row must still be the application's blank screen.
step 'AA| '
echo "bottom/left  last : [$(last)]"
echo "bottom/left  first: [$(first)]"
echo "bottom/left  attrs: $($TM capture-pane -p -e -t client | tail -1 | cat -v | scrub)"

# 2..4. The three remaining justify modes. absolute-centre centres on the whole
#    line rather than on the space left over by status-left/right, so it and
#    centre must NOT produce the same column.
$BIN -L "$ISOCK" set -g status-justify centre
step 'BB| '
echo "centre       last : [$(last)]"

$BIN -L "$ISOCK" set -g status-justify right
step 'CC| '
echo "right        last : [$(last)]"

$BIN -L "$ISOCK" set -g status-justify absolute-centre
step 'DD| '
echo "abs-centre   last : [$(last)]"

# 5. status-position top: the bar moves to the first row and the last row becomes
#    pane content again.
$BIN -L "$ISOCK" set -g status-justify left
$BIN -L "$ISOCK" set -g status-position top
step 'EE| '
echo "top/left     first: [$(first)]"
echo "top/left     last : [$(last)]"

# 6. window-status-separator, the string status_redraw puts between list entries
#    (and, per the default status-format, NOT after the last one).
$BIN -L "$ISOCK" set -g status-position bottom
$BIN -L "$ISOCK" set -g window-status-separator ' :: '
step 'FF| '
echo "sep ' :: '   last : [$(last)]"
$BIN -L "$ISOCK" set -g window-status-separator ''
step 'GG| '
echo "sep empty    last : [$(last)]"

$BIN -L "$ISOCK" kill-server 2>/dev/null
