# Line-level operations on a pane that a floating pane overlaps. Each of these
# normally emits one cheap escape covering the whole line or region, which would
# smear across the float; with a float above they must fall back to a redraw.
# Checked through the model: each op's effect on the tiled pane's own grid must
# be identical whether or not a float sits over it.
$TM new-pane -d -x30 -y8 "cat"
$TM send-keys -t0 'printf "abcdefghijklmnopqrstuvwxyz\\n"' Enter
$TM send-keys -t0 'printf "\\033[3;1H\\033[4@INS\\033[2;1H\\033[3P\\033[5;1H\\033[2X"' Enter
$TM send-keys -t0 'printf "\\033[7;1H\\033[2L\\033[9;1H\\033[1M\\033[12;1H\\033[3T"' Enter
$TM capture-pane -t0 -p | head -14
$TM display-message -p 'float=#{pane_floating_flag} panes=#{window_panes}'
