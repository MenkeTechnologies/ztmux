# switch-mode as it is actually DRAWN: the item list, the selection, the
# incremental (search) prompt on the bottom row, and switch-mode-match-style on
# the characters the fuzzy filter matched.
#
# A mode screen never reaches the pane's own grid, so `capture-pane` on the pane
# in switch-mode cannot see it. Observe it the way 1484/1486 do: run a second
# ("inner") server whose only client is attached inside a pane of the outer
# server, then capture that OUTER pane. `-e` keeps the colours, which is where
# switch-mode-match-style and mode-style live.
#
# Explicit fg/bg throughout, including message-style: the shipped defaults name
# theme colours, and how those resolve is a separate matter from switch mode.
set -- $TM
BIN=$1
SOCK=$3
IN="${SOCK}_sw"

inner() { $BIN -L "$IN" "$@" >/dev/null 2>&1; }

inner -f /dev/null new-session -d -s alpha -n w1 -x 60 -y 12 'sleep 300'
inner new-window -d -n w2 'sleep 300'
inner new-session -d -s beta -n b1 'sleep 300'
inner set-option -g status off
inner set-option -g message-style 'fg=white,bg=blue'
inner set-option -wg mode-style 'fg=black,bg=white'
inner set-option -wg switch-mode-match-style 'fg=red,bg=black'

$TM new-session -d -s outer -x 60 -y 12 "$BIN -L $IN attach -t alpha" >/dev/null 2>&1
sleep 1

box() { printf '== %s\n' "$1"; $TM capture-pane -p -e -t outer:0 -S 0 -E 11 | cat -v; }

# -s lists sessions, sorted by name, current one selected with mode-style. The
# last row is the prompt, drawn by prompt.c into the mode's own screen.
inner switch-mode -s -F '#{session_name}'
sleep 1
box 'session list'

# Typing filters incrementally: the prompt keeps the text, the list keeps only
# fuzzy matches, and the matched columns take switch-mode-match-style.
inner send-keys -t alpha:w1 b
sleep 1
box 'filter b'

# Down moves the selection rather than editing, because the prompt is
# PROMPT_INCREMENTAL without PROMPT_EDITARROWS on the vertical keys.
inner send-keys -t alpha:w1 BSpace Down
sleep 1
box 'down moves selection'

# -w lists windows instead, with the same prompt.
inner send-keys -t alpha:w1 Escape
sleep 1
inner switch-mode -w -F '#{window_name}'
sleep 1
box 'window list'

inner send-keys -t alpha:w1 Escape
$TM kill-session -t outer >/dev/null 2>&1
inner kill-server
