# ICH/DCH shift a line's cells sideways and IL/DL shift the screen's lines
# vertically, both bounded by the scroll region. These are the memmove-shaped
# grid operations: an off-by-one leaves a duplicated or dropped cell that no
# format variable reports, and the redraw path faithfully paints the wrong
# grid. capture-pane is the only assertion that sees it.
$TM new-window -d -n ins 'printf "abcdefghij\nklmnopqrst\nuvwxyz0123\n"; printf "\0033[1;4H\0033[3@"; printf "\0033[2;4H\0033[3P"; sleep 300'
sleep 1
echo "== ICH/DCH"; $TM capture-pane -p -S 0 -E 4 -t ins | perl -pe "s{^(.*)\$}{[\$1]}"
$TM new-window -d -n il 'printf "row1\nrow2\nrow3\nrow4\n"; printf "\0033[2;1H\0033[2L"; sleep 300'
sleep 1
echo "== IL"; $TM capture-pane -p -S 0 -E 7 -t il | perl -pe "s{^(.*)\$}{[\$1]}"
$TM new-window -d -n dl 'printf "row1\nrow2\nrow3\nrow4\n"; printf "\0033[2;1H\0033[2M"; sleep 300'
sleep 1
echo "== DL"; $TM capture-pane -p -S 0 -E 7 -t dl | perl -pe "s{^(.*)\$}{[\$1]}"
# Inside a scroll region the same operations must not touch lines outside it.
$TM new-window -d -n reg 'printf "r1\nr2\nr3\nr4\nr5\nr6\n"; printf "\0033[2;4r\0033[3;1H\0033[1L\0033[6;1Hbottom"; sleep 300'
sleep 1
echo "== region"; $TM capture-pane -p -S 0 -E 8 -t reg | perl -pe "s{^(.*)\$}{[\$1]}"
$TM display-message -p -t reg 'region=#{scroll_region_upper},#{scroll_region_lower} cur=#{cursor_x},#{cursor_y}'
