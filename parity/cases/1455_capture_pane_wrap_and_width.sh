# A line longer than the pane wraps into a second grid line carrying the "this
# line continues" flag, and -J is what re-joins them. Wide (double-width) cells
# make the same boundary interesting: one that does not fit in the last column
# moves whole to the next line rather than splitting, and overwriting the left
# half of a wide cell has to clear its right half too. Both are grid invariants
# a redraw path can only get right if the grid stores them right.
$TM new-window -d -n wrap 'printf "%s\n" "$(i=0; while [ $i -lt 100 ]; do printf x; i=$((i+1)); done)"; printf "ab\0344\0270\0255\0344\0270\0256cd\n"; sleep 300'
sleep 1
echo "== unjoined"
$TM capture-pane -p -S 0 -E 3 -t wrap | perl -pe "s{^(.*)\$}{[\$1]}"
echo "== joined"
$TM capture-pane -p -J -S 0 -E 3 -t wrap | perl -pe "s{^(.*)\$}{[\$1]}"
echo "== widths"
$TM display-message -p -t wrap 'w=#{pane_width} h=#{pane_height} cx=#{cursor_x} cy=#{cursor_y}'
# A wide character straddling the right margin: 79 narrow cells then a wide one.
$TM new-window -d -n wide 'i=0; while [ $i -lt 79 ]; do printf .; i=$((i+1)); done; printf "\0344\0270\0255end\n"; sleep 300'
sleep 1
$TM capture-pane -p -S 0 -E 2 -t wide | perl -pe "s{^(.*)\$}{[\$1]}"
$TM display-message -p -t wide 'cx=#{cursor_x} cy=#{cursor_y}'
