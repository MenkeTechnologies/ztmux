# The erase sequences each clear a different region measured from the cursor:
# EL 0/1/2 within the line, ED 0/1/2 within the screen, ECH by a count. Every
# one of them is a start/end pair computed from the cursor position, which is
# exactly where a sentinel or an off-by-one silently eats a cell — the shape of
# the cursor-position bugs the port has already hit twice.
$TM new-window -d -n erase 'printf "AAAAAAAAAA\nBBBBBBBBBB\nCCCCCCCCCC\nDDDDDDDDDD\n"; printf "\0033[2;5H\0033[0K"; printf "\0033[3;5H\0033[1K"; printf "\0033[4;5H\0033[3X"; sleep 300'
sleep 1
$TM capture-pane -p -S 0 -E 5 -t erase | perl -pe "s{^(.*)\$}{[\$1]}"
$TM display-message -p -t erase 'cur=#{cursor_x},#{cursor_y}'
# ED 0 from the middle, then ED 1, then ED 2 on a fresh screenful.
$TM new-window -d -n ed 'printf "1111111111\n2222222222\n3333333333\n4444444444\n"; printf "\0033[2;3H\0033[0J"; sleep 300'
sleep 1
echo "== ED0"; $TM capture-pane -p -S 0 -E 4 -t ed | perl -pe "s{^(.*)\$}{[\$1]}"
$TM new-window -d -n ed1 'printf "1111111111\n2222222222\n3333333333\n4444444444\n"; printf "\0033[3;5H\0033[1J"; sleep 300'
sleep 1
echo "== ED1"; $TM capture-pane -p -S 0 -E 4 -t ed1 | perl -pe "s{^(.*)\$}{[\$1]}"
$TM new-window -d -n ed2 'printf "1111111111\n2222222222\n"; printf "\0033[2J"; printf "after"; sleep 300'
sleep 1
echo "== ED2"; $TM capture-pane -p -S 0 -E 4 -t ed2 | perl -pe "s{^(.*)\$}{[\$1]}"
