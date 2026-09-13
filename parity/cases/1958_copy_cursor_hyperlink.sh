# copy-mode's three cursor callbacks are read straight off the grid under the
# cursor, and `copy_cursor_hyperlink` (window-copy.c:1052-1062, via
# format_grid_hyperlink) is the only one no case had ever expanded. It resolves
# the cell's hyperlink id through the screen's hyperlink table, so it is the one
# format that proves OSC 8 state SURVIVED into the grid rather than merely being
# parsed and dropped: a port that consumed the sequence without storing the id
# prints the text identically in every capture-pane case and empty here.
#
# The row is written by the pane's own command, so the case fences on the text
# appearing instead of sleeping — under suite load a blind sleep races the pty.
$TM new-window -d -n link "printf '\033]8;;http://example.com/a\033\\\\ALPHA\033]8;;\033\\\\ plain \033]8;;http://example.com/b\033\\\\BETA\033]8;;\033\\\\\n'; sleep 300"

i=0
while [ $i -lt 80 ]; do
  case "$($TM capture-pane -p -t link 2>/dev/null)" in *ALPHA*plain*BETA*) break ;; esac
  i=$((i+1)); sleep 0.1
done

echo "== the text itself carries no trace of the link =="
$TM capture-pane -p -t link | head -1

$TM copy-mode -t link
echo "== the cursor starts on the empty row below, so all three are empty =="
$TM display-message -p -t link 'word=[#{copy_cursor_word}] line=[#{copy_cursor_line}] link=[#{copy_cursor_hyperlink}]'

$TM send-keys -X -t link top-line
echo "== on the first link =="
$TM display-message -p -t link 'x=#{copy_cursor_x} word=[#{copy_cursor_word}] link=[#{copy_cursor_hyperlink}]'

echo "== still the same link three columns in =="
$TM send-keys -X -t link cursor-right
$TM send-keys -X -t link cursor-right
$TM send-keys -X -t link cursor-right
$TM display-message -p -t link 'x=#{copy_cursor_x} word=[#{copy_cursor_word}] link=[#{copy_cursor_hyperlink}]'

echo "== the unlinked run between them has no hyperlink =="
$TM send-keys -X -t link next-word
$TM display-message -p -t link 'x=#{copy_cursor_x} word=[#{copy_cursor_word}] link=[#{copy_cursor_hyperlink}]'

echo "== the second link is a DIFFERENT id, not the first one again =="
$TM send-keys -X -t link next-word
$TM display-message -p -t link 'x=#{copy_cursor_x} word=[#{copy_cursor_word}] link=[#{copy_cursor_hyperlink}]'

echo "== the whole line, which is what copy_cursor_line reads =="
$TM display-message -p -t link 'line=[#{copy_cursor_line}]'

echo "== outside copy mode the callbacks are not installed at all =="
$TM send-keys -X -t link cancel
$TM display-message -p -t link 'word=[#{copy_cursor_word}] link=[#{copy_cursor_hyperlink}]'
