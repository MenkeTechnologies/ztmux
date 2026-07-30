# The alternate screen swaps the whole grid out and back, saving and restoring
# the cursor with it. Nothing written to the alternate screen may reach the
# history, and the primary screen's contents and cursor must come back exactly
# as they were — the invariant that a redraw path relies on when it decides
# what to repaint.
$TM new-window -d -n alt 'printf "primary-1\nprimary-2\n"; printf "\0033[?1049h"; printf "alternate-content\n"; sleep 300'
sleep 1
$TM display-message -p -t alt 'in-alt=#{alternate_on} hist=#{history_size} cur=#{cursor_x},#{cursor_y}'
$TM capture-pane -p -S 0 -E 3 -t alt | perl -pe "s{^(.*)\$}{[\$1]}"
$TM send-keys -t alt -H 1b 5b 3f 31 30 34 39 6c
sleep 1
$TM display-message -p -t alt 'after=#{alternate_on} hist=#{history_size}'
# The plain 47/1047 variants and DECSC/DECRC around them.
$TM new-window -d -n sc 'printf "line-A\nline-B\n"; printf "\0033[3;5H\0338"; printf "\00337"; printf "\0033[10;20H"; printf "\0338moved"; sleep 300'
sleep 1
$TM display-message -p -t sc 'cur=#{cursor_x},#{cursor_y}'
$TM capture-pane -p -S 0 -E 12 -t sc | perl -pe "s{^(.*)\$}{[\$1]}" | grep -n moved
