# #{copy_cursor_line} and #{copy_cursor_word} read the pane's backing grid at
# the scrolled-back position, NOT copy mode's own screen: the screen carries
# the "[0/0]" position indicator copy mode paints over the top-right of the
# pane, and its line numbering ignores the scroll offset. Reading the wrong one
# shows up as the indicator leaking into the line, and as the wrong line
# entirely once the view is scrolled back into history.
$TM new-window -d -n src 'for i in 1 2 3 4 5 6 7 8 9 10 11 12; do echo line-$i-word; done; sleep 300'
sleep 1
$TM copy-mode -t src
$TM display-message -p -t src 'start  #{copy_cursor_y} [#{copy_cursor_line}] [#{copy_cursor_word}]'
$TM send-keys -X -t src history-top
$TM display-message -p -t src 'top    #{copy_cursor_y} off=#{scroll_position} [#{copy_cursor_line}] [#{copy_cursor_word}]'
$TM send-keys -X -t src cursor-down
$TM send-keys -X -t src cursor-right
$TM send-keys -X -t src cursor-right
$TM display-message -p -t src 'down1  #{copy_cursor_y} off=#{scroll_position} [#{copy_cursor_line}] [#{copy_cursor_word}]'
$TM send-keys -X -t src history-bottom
$TM display-message -p -t src 'bottom #{copy_cursor_y} off=#{scroll_position} [#{copy_cursor_line}]'
