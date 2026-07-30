# select-word and select-line set BOTH ends of a selection in one step, from the
# word-separator scan and from the line's extent respectively. They are the pair
# most likely to disagree with the manual motions: the same boundary scan run
# forwards and backwards from one position, where an off-by-one in either
# direction shifts the copied text by a character without changing its shape.
$TM new-window -d -n sw 'printf "alpha bravo  charlie\n  delta_echo foxtrot\n"; sleep 300'
sleep 1
$TM copy-mode -t sw
$TM send-keys -X -t sw history-top
$TM send-keys -X -t sw start-of-line
report() {
  $TM display-message -p -t sw \
    "$1 s=#{selection_start_y},#{selection_start_x} e=#{selection_end_y},#{selection_end_x} cur=#{copy_cursor_y},#{copy_cursor_x} word=[#{copy_cursor_word}]"
}
$TM send-keys -X -t sw select-word; report word-at-start
$TM send-keys -X -t sw copy-selection-no-clear
$TM show-buffer | perl -pe "s{^(.*)\$}{[\$1]}"
# Inside the second word, then over the run of spaces between words.
$TM send-keys -X -t sw start-of-line
for _ in 1 2 3 4 5 6 7 8; do $TM send-keys -X -t sw cursor-right; done
$TM send-keys -X -t sw select-word; report word-mid
$TM send-keys -X -t sw copy-selection-no-clear
$TM show-buffer | perl -pe "s{^(.*)\$}{[\$1]}"
# select-line takes the whole line including its leading blanks.
$TM send-keys -X -t sw cursor-down
$TM send-keys -X -t sw select-line; report line
$TM send-keys -X -t sw copy-selection-no-clear
$TM show-buffer | perl -pe "s{^(.*)\$}{[\$1]}"
