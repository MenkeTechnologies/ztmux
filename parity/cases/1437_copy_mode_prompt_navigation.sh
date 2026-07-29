# previous-prompt / next-prompt must be no-ops on a pane with no shell prompt
# marks, not a crash. The C steps its line counter with `line += add` where add
# is -1, an unsigned wrap it never actually reaches because the loop guards
# against it; a port that writes that as an unsigned add takes the whole server
# down instead. The display-message afterwards is the real assertion: if the
# server died, there is nothing left to answer it.
$TM new-window -d -n prompts 'printf "one\ntwo\nthree\n"; sleep 300'
sleep 1
$TM copy-mode -t prompts
$TM send-keys -X -t prompts previous-prompt
$TM display-message -p -t prompts 'prev #{copy_cursor_y} #{pane_mode}'
$TM send-keys -X -t prompts next-prompt
$TM display-message -p -t prompts 'next #{copy_cursor_y} #{pane_mode}'
$TM send-keys -X -t prompts previous-prompt -o
$TM display-message -p -t prompts 'prev-o #{copy_cursor_y} #{pane_mode}'
$TM list-windows -F '#{window_name}'
