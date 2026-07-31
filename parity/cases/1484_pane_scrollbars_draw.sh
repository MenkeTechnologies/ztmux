# pane-scrollbars: the bar as it is actually DRAWN.
#
# A scrollbar is written straight to the client's terminal — it never enters the
# pane's own grid — so `capture-pane` on the pane it belongs to cannot see it.
# Observe it the way regress/am-terminal.sh observes drawn output: run a second
# ("inner") server whose only client is attached inside a pane of the outer
# server, then capture that OUTER pane, whose grid holds exactly what the inner
# server drew. `capture-pane -e` keeps the colours, which is where the bar is:
# the trough is drawn in the scrollbar style, the slider in the same style with
# foreground and background swapped, and the padding column in the pane's own
# default colours.
#
# Explicit fg/bg are used throughout: the shipped default
# `pane-scrollbars-style` names the theme colours themedarkgrey/themelightgrey,
# whose resolution is a separate matter from the scrollbar itself.
set -- $TM
BIN=$1
SOCK=$3
IN="${SOCK}_sb"

inner() { $BIN -L "$IN" "$@" >/dev/null 2>&1; }

# Inner server: 20x6, no status line, and a pane holding a dozen lines so there
# is scrollback for the slider to represent.
inner -f /dev/null new-session -d -x 20 -y 6 \
  "sh -c 'printf \"L%s\n\" 1 2 3 4 5 6 7 8 9 10 11 12; sleep 300'"
inner set-option -g status off
inner set-option -g pane-scrollbars-timeout 60000
# The stock copy-mode position indicator carries a clock, and its style comes
# from mode-style, which names theme colours. Pin both: this case is about the
# bar, and the indicator only has to stay put beside it.
inner set-option -g copy-mode-position-format '#[align=right][#{copy_position}/#{copy_position_limit}]'
inner set-option -g copy-mode-position-style 'fg=green,bg=black'

# The outer client renders the inner server's screen into a pane we can capture.
$TM new-session -d -s outer -x 20 -y 6 "$BIN -L $IN attach" >/dev/null 2>&1
sleep 1

shot() { printf '== %s\n' "$1"; $TM capture-pane -p -e -t outer:0 | cat -v; }

# Off: nothing is drawn, and the pane keeps every column.
shot 'off'
$TM display-message -p -t outer:0 'outer pane #{pane_width}x#{pane_height}'

# On (reserved): the bar sits in the column layout took out of the pane. The
# trough runs from the top, the slider sits at the bottom because the view is at
# the bottom of the history.
inner set-option -g pane-scrollbars on
inner set-option -g pane-scrollbars-style 'fg=blue,bg=red,width=1,pad=0'
sleep 1
shot 'on right width=1 pad=0'

# Width and padding: the pad column is drawn in the pane's default colours, on
# the pane side of the bar.
inner set-option -g pane-scrollbars-style 'fg=blue,bg=red,width=2,pad=1'
sleep 1
shot 'on right width=2 pad=1'

# Left position: the bar moves to the other side and the pane's contents shift
# right with it.
inner set-option -g pane-scrollbars-position left
sleep 1
shot 'on left width=2 pad=1'

inner set-option -g pane-scrollbars-position right
inner set-option -g pane-scrollbars-style 'fg=blue,bg=red,width=1,pad=0'

# Scrolling moves the slider: in copy mode the slider position comes from the
# copy-mode offset rather than from the screen size, so paging up walks it to
# the top of the trough.
inner copy-mode -t 0
sleep 1
shot 'copy-mode at bottom'
inner send-keys -t 0 -X page-up
sleep 1
shot 'copy-mode one page up'
inner send-keys -t 0 -X history-top
sleep 1
shot 'copy-mode at top'
inner send-keys -t 0 -X cancel

# Auto-hide (overlay): nothing is drawn until something scrolls, and then the
# bar is drawn OVER the pane's own last column rather than beside it, so the
# pane keeps its full width.
inner set-option -g pane-scrollbars auto-hide
sleep 1
shot 'auto-hide idle'
$TM display-message -p -t outer:0 'outer pane #{pane_width}x#{pane_height}'
inner copy-mode -t 0
inner send-keys -t 0 -X page-up
sleep 1
shot 'auto-hide after scrolling'
inner send-keys -t 0 -X cancel

# Modal: shown only while the pane is in a mode, also as an overlay.
inner set-option -g pane-scrollbars modal
sleep 1
shot 'modal outside a mode'
inner copy-mode -t 0
sleep 1
shot 'modal in copy mode'
inner send-keys -t 0 -X cancel

$TM kill-session -t outer >/dev/null 2>&1
inner kill-server
