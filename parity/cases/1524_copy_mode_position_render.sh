# The copy-mode POSITION INDICATOR as a client paints it.
#
# window_copy_write_line (window-copy.c:5209) draws copy-mode-position-format
# through format_draw on row 0 only, inside copy-mode-position-style, clipped to
# content_sx. Nothing about that is visible from the server: #{copy_position}
# and #{copy_position_limit} expand fine with no client attached, but whether the
# indicator is DRAWN, WHERE, in WHICH style, and whether -H suppresses it are all
# client-only. So is the #[align=...] handling, since the default format leads
# with #[align=right] and format_draw is what resolves it.
#
# Built like cases 1504/1507/1508: an inner server with a client attached inside
# a pane of the outer server, so capture-pane on the OUTER server reads back the
# exact cells the inner client drew.
#
# Determinism: the default format leads with #{t/p:top_line_time}, which is a
# wall clock as soon as the top visible line has been scrolled into history, so
# HH:MM is masked. The scrollback content is generated AFTER the client has
# attached and the pane has settled at its final size -- generating it first and
# attaching after would reflow the grid mid-race and make history_size differ
# between runs.
set -- $TM
BIN="$1"
ISOCK="cpr_$$_inner"

scrub() { perl -pe 's{/dev/tty[a-z0-9]+}{/dev/ttyDEV}g; s/\d\d:\d\d:\d\d/HH:MM:SS/g; s/\d\d:\d\d/HH:MM/g; s/[ \t]+$//'; }

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" setw -g mode-keys vi
# Pin the style explicitly: the default is #{E:mode-style} -> themeyellow, which
# resolves through terminal theme detection and would put an RGB triplet in the
# expected bytes.
$BIN -L "$ISOCK" setw -g copy-mode-position-style 'bg=blue,fg=white'

# Poll real state instead of sleeping: under suite load a blind sleep races the
# client's first draw and the post-attach resize. Both loops are bounded at ~3s
# so the whole case stays far inside the runner's 15s per-case budget.
inner() { $BIN -L "$ISOCK" display-message -p -t "$1" "$2" 2>/dev/null; }
wait_for() {  # $1 target, $2 format, $3 expected
  local i=0 got
  while [ $i -lt 60 ]; do
    got=$(inner "$1" "$2")
    [ "$got" = "$3" ] && return 0
    i=$(( i + 1 )); sleep 0.05
  done
  echo "wait_for: [$2] on [$1] wanted [$3] got [$got]"
}
settle() {  # wait until the outer pane stops changing
  local a='' b i=0
  while [ $i -lt 30 ]; do
    b=$($TM capture-pane -p -t client)
    [ -n "$b" ] && [ "$a" = "$b" ] && return 0
    a="$b"; i=$(( i + 1 )); sleep 0.1
  done
}
row1() { $TM capture-pane -p -e -t client | sed -n '1p' | cat -v | scrub; }
enter() {  # cancel any mode, then enter copy mode with the given flags
  $BIN -L "$ISOCK" send-keys -X -t alpha:data cancel 2>/dev/null
  wait_for alpha:data '#{pane_in_mode}' 0
  $BIN -L "$ISOCK" copy-mode "$@" -t alpha:data
  wait_for alpha:data '#{pane_mode}' copy-mode
  settle
}

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"
wait_for alpha '#{session_attached}' 1
wait_for alpha:one '#{pane_height}' 23
settle

# Content is made only now, at the pane's final size, so no reflow can race.
$BIN -L "$ISOCK" new-window -n data 'i=1; while [ $i -le 30 ]; do echo "line $i"; i=$(( i + 1 )); done; sleep 300'
wait_for alpha:data '#{history_size}' 8
settle

echo "bottom:"; enter; row1
echo "position: $(inner alpha:data 'pos=#{copy_position} limit=#{copy_position_limit} scroll=#{scroll_position}')"

# page-up moves oy, so both the numbers and the top_line_time segment change.
echo "after page-up:"
$BIN -L "$ISOCK" send-keys -X -t alpha:data page-up
wait_for alpha:data '#{scroll_position}' 8
settle
row1
echo "position: $(inner alpha:data 'pos=#{copy_position} limit=#{copy_position_limit} scroll=#{scroll_position}')"

# -H sets hide_position: the indicator must not be drawn at all.
echo "copy-mode -H:"; enter -H; row1

# format_draw alignment, exercised through the option rather than the default.
$BIN -L "$ISOCK" setw -g copy-mode-position-format '#[align=left]<#{copy_position}|#{copy_position_limit}>'
echo "align=left:"; enter; row1
$BIN -L "$ISOCK" setw -g copy-mode-position-format '#[align=centre]{#{copy_position}}'
echo "align=centre:"; enter; row1
# An empty expansion draws nothing (window-copy.c:5214 checks *expanded).
$BIN -L "$ISOCK" setw -g copy-mode-position-format '#{?0,never,}'
echo "empty expansion:"; enter; row1
# An empty option value skips format_expand entirely.
$BIN -L "$ISOCK" setw -g copy-mode-position-format ''
echo "empty option:"; enter; row1

$BIN -L "$ISOCK" kill-server 2>/dev/null
