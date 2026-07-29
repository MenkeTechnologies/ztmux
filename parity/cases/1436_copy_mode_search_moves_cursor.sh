# copy-mode search must actually find the term and move the cursor to it.
# The search compares the term cell by cell and only reports a match when every
# cell matched, which a port can get subtly wrong in a way that makes EVERY
# search silently fail while every other copy-mode command still works — so the
# cursor position after each search is the thing to compare, not just the fact
# that the command was accepted.
$TM new-window -d -n hay 'printf "alpha\nbravo\ncharlie\ndelta\nneedle-here\necho\nfoxtrot\n"; sleep 300'
sleep 1
$TM copy-mode -t hay
$TM send-keys -X -t hay search-backward needle
$TM display-message -p -t hay 'up   #{copy_cursor_y},#{copy_cursor_x} [#{copy_cursor_line}]'
$TM send-keys -X -t hay search-forward foxtrot
$TM display-message -p -t hay 'down #{copy_cursor_y},#{copy_cursor_x} [#{copy_cursor_line}]'
# A single-character term, then repeat it forwards and backwards.
$TM send-keys -X -t hay search-backward a
$TM display-message -p -t hay 'a    #{copy_cursor_y},#{copy_cursor_x} [#{copy_cursor_line}]'
$TM send-keys -X -t hay search-again
$TM display-message -p -t hay 'again #{copy_cursor_y},#{copy_cursor_x} [#{copy_cursor_line}]'
$TM send-keys -X -t hay search-reverse
$TM display-message -p -t hay 'rev  #{copy_cursor_y},#{copy_cursor_x} [#{copy_cursor_line}]'
# Case-insensitive: an all-lowercase term matches mixed case, an uppercase one
# does not.
$TM send-keys -X -t hay history-top
$TM send-keys -X -t hay search-forward-text charlie
$TM display-message -p -t hay 'ci   #{copy_cursor_y},#{copy_cursor_x} [#{copy_cursor_line}]'
# A term that is nowhere in the pane leaves the cursor where it was.
$TM send-keys -X -t hay search-forward-text zzzznotthere
$TM display-message -p -t hay 'miss #{copy_cursor_y},#{copy_cursor_x} [#{copy_cursor_line}]'
