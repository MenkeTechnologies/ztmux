# A grid cell holds up to a fixed number of UTF-8 bytes, and combining marks are
# appended to the cell they follow rather than taking a cell of their own. Past
# that limit the extra marks are dropped, and a zero-width joiner sequence has
# to stay in one cell while a wide emoji takes two. capture-pane is what shows
# which of those the port actually stored.
$TM new-window -d -n u8 'printf "e\0314\0201 combining\n"; printf "a\0314\0201\0314\0200\0314\0202\0314\0203\0314\0204\0314\0205\0314\0206\0314\0207 many\n"; printf "\0342\0202\0254 euro\n"; printf "\0360\0237\0221\0250\0342\0200\0215\0360\0237\0222\0273 zwj\n"; printf "\0344\0270\0255\0346\0226\0207 wide\n"; sleep 300'
sleep 1
$TM capture-pane -p -S 0 -E 5 -t u8 | perl -pe "s{^(.*)\$}{[\$1]}" | od -c | perl -pe 's/\s+$//' | head -40
$TM display-message -p -t u8 'cur=#{cursor_x},#{cursor_y}'
# The same content through the copy-mode word/line formats, which read the grid
# cell by cell rather than by run.
$TM copy-mode -t u8
$TM send-keys -X -t u8 history-top
for _ in 1 2 3 4; do
  $TM display-message -p -t u8 "line=[#{copy_cursor_line}] word=[#{copy_cursor_word}] x=#{copy_cursor_x}"
  $TM send-keys -X -t u8 cursor-down
done
