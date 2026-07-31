# tree-mode-border-style / -preview-style / -preview-format: the session preview
# of choose-tree as it is actually DRAWN.
#
# A mode screen never reaches the pane's own grid, so `capture-pane` on the pane
# in choose-tree cannot see it. Observe it the way 1484 observes scrollbars: run
# a second ("inner") server whose only client is attached inside a pane of the
# outer server, then capture that OUTER pane. `-e` keeps the colours, which is
# where two of these three options live.
#
# Only the preview box is captured (`-S 16`), not the item list above it: the
# next-3.7 item line is built from a prefix format ztmux has not ported, so its
# bytes differ for reasons that have nothing to do with these options. 1487
# covers tree-mode-selection-style and the per-pane preview.
#
# Explicit fg/bg throughout: every shipped default here names theme colours, and
# how those resolve is a separate matter from tree mode.
set -- $TM
BIN=$1
SOCK=$3
IN="${SOCK}_tm"

inner() { $BIN -L "$IN" "$@" >/dev/null 2>&1; }

# Inner server: 60x30 is the smallest size that gives mode-tree a preview at all
# (it wants a list at least 10 rows tall with room left under it).
inner -f /dev/null new-session -d -s S -n w1 -x 60 -y 30 'sleep 300'
inner new-window -d -n w2 'sleep 300'
inner set-option -g status off
inner set-option -wg tree-mode-border-style 'fg=blue,bg=black'
inner set-option -wg tree-mode-selection-style 'fg=green,bg=black'
inner set-option -wg tree-mode-preview-style 'fg=red'
inner set-option -wg tree-mode-preview-format '#{window_index}W'

$TM new-session -d -s outer -x 60 -y 30 "$BIN -L $IN attach" >/dev/null 2>&1
sleep 1

# Row 15 is the top of the box, and carries the item name and sort order; rows
# 16 down are the preview proper.
box() { printf '== %s\n' "$1"; $TM capture-pane -p -e -t outer:0 -S 16 -E 29 | cat -v; }

inner choose-tree -s
sleep 1

# Session preview: one framed label per window, drawn over the window's own
# contents, separated by a vertical rule. The frame and the rule take
# tree-mode-border-style, the label text tree-mode-preview-style, and the label
# itself is tree-mode-preview-format expanded against that window.
box 'session preview'

# The border style moves the frame, the rules and the label's own background.
inner set-option -wg tree-mode-border-style 'fg=magenta,bg=black'
sleep 1
box 'border fg=magenta'

# The preview style only reaches the label text.
inner set-option -wg tree-mode-preview-style 'fg=cyan,bright'
sleep 1
box 'preview style fg=cyan,bright'

# It is a format, so it can choose per item: this one greens the active window.
inner set-option -wg tree-mode-preview-style 'fg=#{?window_active,green,yellow}'
sleep 1
box 'preview style by window_active'

# The format decides the text. A wide one is trimmed to fit the frame.
inner set-option -wg tree-mode-preview-format '[#{window_index}:#{window_name}]'
sleep 1
box 'format with name'
inner set-option -wg tree-mode-preview-format 'window #{window_index} of session #{session_name} running #{window_panes} pane(s)'
sleep 1
box 'format too wide to fit'

# An empty format draws no label at all — not an empty frame.
inner set-option -wg tree-mode-preview-format ''
sleep 1
box 'format empty'

inner send-keys -X cancel
$TM kill-session -t outer >/dev/null 2>&1
inner kill-server
