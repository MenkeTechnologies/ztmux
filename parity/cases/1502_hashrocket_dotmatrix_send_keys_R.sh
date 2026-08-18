# dotmatrix's two "unstick this pane" bindings: C-l is `send-keys -R`, C-k is
# `send-keys -R \; clear-history`. They are what a Hashrocket user reaches for
# after a curses app dies mid-escape-sequence, so -R has to do the whole job:
# clear the pane's colour palette, reset the input parser, and mark the pane for
# redraw -- without sending any key, because -R with no keys returns early.
#
# The pane below is left in four sticky states at once (alternate screen, insert
# mode, origin mode, no-wrap) plus an OSC 4 palette override, so a -R that resets
# only some of them shows up as a flag that survived.
flags() {
  $TM display-message -p -t r \
    "$1 alt=#{alternate_on} ins=#{insert_flag} org=#{origin_flag} wrap=#{wrap_flag} keypad=#{keypad_flag} kcur=#{keypad_cursor_flag}"
}
$TM new-window -d -n r 'printf "\033]4;1;rgb:00/00/ff\033\\\\\033[?1049h\033[4h\033[?6h\033[?7l\033[?1h"; sleep 300'
sleep 1
flags before
$TM send-keys -t r -R
sleep 0.5
flags after-R

# -R alone sends nothing: a pane fed only -R has taken no keystroke, so the
# cursor has not moved off the origin.
d=$(mktemp -d)
$TM new-window -d -n k "cat > $d/keys; sleep 300"
sleep 1
$TM send-keys -t k -R
sleep 0.5
$TM display-message -p -t k 'cursor=#{cursor_x},#{cursor_y}'

# C-k's second half. history-limit is the dotmatrix value, so the scrollback the
# pane accumulates is real and clear-history has something to drop.
$TM set -g history-limit 100000
$TM new-window -d -n h 'for i in $(seq 1 300); do echo "line $i"; done; sleep 300'
sleep 1.5
$TM display-message -p -t h "before size>0=#{?#{>:#{history_size},0},yes,no} limit=#{history_limit}"
$TM send-keys -t h -R
$TM clear-history -t h
sleep 0.5
$TM display-message -p -t h "after size=#{history_size} bytes>0=#{?#{>:#{history_bytes},0},yes,no}"
printf 'keys-received=%s\n' "$(wc -c <"$d/keys" | tr -d ' ')"
rm -rf "$d"
