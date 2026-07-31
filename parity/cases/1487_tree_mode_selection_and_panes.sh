# tree-mode-selection-style, and the preview options read from PANE options.
#
# Companion to 1486, same nested-client trick: an inner server whose only client
# is attached inside a pane of the outer server, then `capture-pane -e` on that
# outer pane reads back what the inner server drew.
#
# When choose-tree previews a WINDOW it draws one framed label per pane and
# reads the format and the styles out of each PANE's options, expanded against
# that pane. Reaching that path needs a window with two panes that is NOT the
# one hosting the mode — splitting the host would halve it and leave it too
# short for mode-tree to draw a preview at all.
#
# The selected item line cannot be compared whole — next-3.7 builds it from a
# prefix format ztmux has not ported — but its leading SGR run is exactly what
# tree-mode-selection-style contributes to it, so that is what is compared. It
# is probed under `choose-tree -s`, where the one line on screen is the selected
# one — which is only true on a fresh tree, so that probe runs first.
set -- $TM
BIN=$1
SOCK=$3
IN="${SOCK}_tp"

inner() { $BIN -L "$IN" "$@" >/dev/null 2>&1; }

inner -f /dev/null new-session -d -s S -n w1 -x 60 -y 30 'sleep 300'
inner new-window -d -n w2 'sleep 300'
inner split-window -d -t w2 'sleep 300'
inner set-option -g status off
inner set-option -g mode-style 'fg=yellow,bg=black'
inner set-option -wg tree-mode-border-style 'fg=blue,bg=black'
inner set-option -wg tree-mode-selection-style 'fg=green,bg=black'
inner set-option -wg tree-mode-preview-style 'fg=#{?pane_active,green,red}'
inner set-option -wg tree-mode-preview-format '#{pane_index}P'

$TM new-session -d -s outer -x 60 -y 30 "$BIN -L $IN attach" >/dev/null 2>&1
sleep 1

box() { printf '== %s\n' "$1"; $TM capture-pane -p -e -t outer:0 -S 16 -E 29 | cat -v; }
sel() {
  printf '== %s selected-line style: ' "$1"
  $TM capture-pane -p -e -t outer:0 -S 0 -E 0 \
    | perl -ne 'print $1 if /^((?:\e\[[0-9;]*m)+)/' | cat -v
  printf '\n'
}

# Selection style, on the one item a fresh `-s` puts on screen.
inner choose-tree -s
sleep 1
sel 'fg=green,bg=black'

# It is read fresh on every draw.
inner set-option -wg tree-mode-selection-style 'fg=black,bg=cyan'
sleep 1
sel 'fg=black,bg=cyan'
inner set-option -wg tree-mode-selection-style 'reverse'
sleep 1
sel 'reverse'
# Unset, it falls back to mode-style through its #{E:mode-style} default — and
# follows mode-style when that changes.
inner set-option -u -wg tree-mode-selection-style
sleep 1
sel 'from mode-style'
inner set-option -g mode-style 'fg=blue,bg=black'
sleep 1
sel 'from changed mode-style'

inner send-keys -X cancel

# Pane preview. A pane remembers its mode tree across a cancel, so `M-+`
# (expand every root) makes the walk independent of what `-s` left collapsed:
# from the session, two Downs reach w2. Pane 0 is w2's active pane and takes the
# green arm of the preview style, pane 1 the red one.
inner choose-tree -w
inner send-keys M-+
inner send-keys Down
inner send-keys Down
sleep 1
box 'window preview by pane'

# A per-pane override moves only the pane it was set on.
inner set-option -p -t w2.1 tree-mode-preview-format 'ONLY-#{pane_index}'
sleep 1
box 'per-pane format override'

inner send-keys -X cancel
$TM kill-session -t outer >/dev/null 2>&1
inner kill-server
